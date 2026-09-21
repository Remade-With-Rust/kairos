//! INTEROP: our MQTT client against a FOREIGN broker.
//!
//! The mission plan's K7 kill test is *"a Kairos device publishing over
//! our TCP to the Home Computer's `rumqttd`, one hour, zero lost
//! keep-alives"*. This is that test with two substitutions, both stated
//! rather than hidden:
//!
//! * the broker is **mosquitto**, not `rumqttd`. Both are third-party and
//!   neither has heard of us, which is what the test is actually asking;
//!   mosquitto is a C broker, so it is if anything a stranger stranger.
//! * the duration is seconds, not an hour. The hour is a SOAK and needs a
//!   deployment; what a workstation can settle is whether keep-alives are
//!   exchanged correctly at all, and it settles that by running several
//!   keep-alive periods and counting.
//!
//! The stack underneath is ours all the way down: `rusty_rtos_mqtt` over
//! `rusty_rtos_tcp` over smoltcp over a TAP. The broker is on the Linux
//! kernel's stack on the other side of it.
//!
//! Exit status is the verdict, and the counts are on stdout so a soak can
//! be built on this later without rewriting it.

use std::env;
use std::io::Write as _;
use std::process::ExitCode;

use rusty_rtos_mqtt_core::client::{AckReply, Clock, Event, EventHandler, MqttContext, Store};
use rusty_rtos_mqtt_core::connect::Connect;
use rusty_rtos_mqtt_core::outpublish::OutgoingPublish;
use rusty_rtos_mqtt_core::reader::{Recv, Sent, Transport as MqttTransport};
use rusty_rtos_mqtt_core::writer::ConnectInfo;

use rusty_rtos_tcp_core::socket::{Host, Socket, Stack, Status as TcpStatus};

use smoltcp::iface::{Config, Interface, SocketSet, SocketStorage};
use smoltcp::phy::{Medium, TunTapInterface};
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr};

const KEEP_ALIVE_SECONDS: u16 = 2;

struct Wall {
    started: std::time::Instant,
}

impl Host for Wall {
    fn now_millis(&mut self) -> i64 {
        i64::try_from(self.started.elapsed().as_millis()).unwrap_or(i64::MAX)
    }

    fn wait(&mut self, millis: u32) {
        std::thread::sleep(std::time::Duration::from_millis(u64::from(millis)));
    }
}

struct Millis {
    started: std::time::Instant,
}

impl Clock for Millis {
    fn now_ms(&mut self) -> u32 {
        u32::try_from(self.started.elapsed().as_millis()).unwrap_or(u32::MAX)
    }
}

struct NoStore;

impl Store for NoStore {
    fn store(&mut self, _packet_id: u32, _parts: &[&[u8]]) -> bool {
        true
    }
    fn retrieve(&mut self, _packet_id: u32) -> Option<&[u8]> {
        None
    }
    fn clear(&mut self, _packet_id: u32) {}
}

/// Counts what the broker sends back, so "zero lost keep-alives" can be a
/// number rather than an impression.
#[derive(Default)]
struct Counting {
    events: usize,
    pingresp: usize,
    puback: usize,
    publish: usize,
}

impl EventHandler for Counting {
    fn on_event(&mut self, event: &Event<'_>, _reply: &mut AckReply<'_>) -> bool {
        self.events = self.events.saturating_add(1);
        // The event's own Debug spelling is the only thing this binary can
        // rely on across the enum's shape, and it is enough to count by.
        let name = format!("{event:?}");
        if name.contains("PingResp") || name.contains("Pingresp") {
            self.pingresp = self.pingresp.saturating_add(1);
        }
        if name.contains("PubAck") || name.contains("Puback") {
            self.puback = self.puback.saturating_add(1);
        }
        if name.contains("Publish") {
            self.publish = self.publish.saturating_add(1);
        }
        true
    }
}

struct Wire<'a, 'buf> {
    stack: &'a mut Stack<'buf, TunTapInterface>,
    socket: Socket,
    host: Wall,
    sent: usize,
    received: usize,
}

impl Wire<'_, '_> {
    fn turn(&mut self) {
        let now = self.host.now_millis();
        self.stack.poll(now);
    }
}

impl MqttTransport for Wire<'_, '_> {
    fn recv(&mut self, into: &mut [u8]) -> Recv {
        self.turn();
        match self.stack.recv(self.socket, into) {
            Ok(0) => Recv::Nothing,
            Ok(read) => {
                self.received = self.received.saturating_add(read);
                Recv::Bytes(read)
            }
            Err(_) => Recv::Nothing,
        }
    }

    fn send(&mut self, bytes: &[u8]) -> Sent {
        let Ok(sent) = self.stack.send(self.socket, bytes) else {
            return Sent::Failed;
        };
        self.sent = self.sent.saturating_add(sent);
        self.turn();
        Sent::Bytes(sent)
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let tap = args.get(1).cloned().unwrap_or_else(|| "kairos0".to_owned());
    let ours = parse_v4(args.get(2).map_or("192.168.69.2", String::as_str));
    let theirs = parse_v4(args.get(3).map_or("192.168.69.1", String::as_str));
    let port: u16 = args.get(4).and_then(|v| v.parse().ok()).unwrap_or(1883);
    let seconds: u64 = args.get(5).and_then(|v| v.parse().ok()).unwrap_or(8);
    // Whose broker this is. Printing "mosquitto" while talking to
    // rumqttd is the kind of line that gets quoted.
    let broker = args.get(6).cloned().unwrap_or_else(|| "the broker".to_owned());

    let Ok(mut device) = TunTapInterface::new(&tap, Medium::Ethernet) else {
        eprintln!("cannot open {tap}");
        return ExitCode::from(2);
    };

    let mut config = Config::new(HardwareAddress::Ethernet(EthernetAddress([
        0x02, 0x00, 0x00, 0x00, 0x00, 0x01,
    ])));
    config.random_seed = 0x2468_ace0;
    let mut iface = Interface::new(config, &mut device, Instant::now());
    iface.update_ip_addrs(|addrs| {
        let _ = addrs.push(IpCidr::new(
            IpAddress::v4(ours[0], ours[1], ours[2], ours[3]),
            24,
        ));
    });

    let mut storage = [SocketStorage::EMPTY; 2];
    let mut rx = [0u8; 4096];
    let mut tx = [0u8; 4096];
    let sockets = SocketSet::new(&mut storage[..]);
    let mut stack = Stack::new(iface, sockets, device);
    let socket = stack.tcp_socket(&mut rx, &mut tx);

    let mut host = Wall {
        started: std::time::Instant::now(),
    };
    let peer = [theirs[0], theirs[1], theirs[2], theirs[3]];
    if stack.connect_blocking(&mut host, socket, peer, port, 49153, 5000) != TcpStatus::Success {
        eprintln!("could not reach the broker at {theirs:?}:{port}");
        return ExitCode::from(3);
    }
    println!("TCP up to a FOREIGN broker at {theirs:?}:{port}");

    let mut network = [0u8; 2048];
    let mut mqtt = MqttContext::new(&mut network);
    let mut wire = Wire {
        stack: &mut stack,
        socket,
        host: Wall {
            started: std::time::Instant::now(),
        },
        sent: 0,
        received: 0,
    };
    let mut clock = Millis {
        started: std::time::Instant::now(),
    };

    let connect = Connect {
        info: ConnectInfo {
            clean_session: true,
            keep_alive_seconds: KEEP_ALIVE_SECONDS,
            username: None,
            password: None,
        },
        client_identifier: b"kairos-interop",
        properties: &[],
        will: None,
    };

    let mut present = false;
    if let Err(error) = mqtt.connect(
        &mut wire,
        &mut clock,
        None::<&mut NoStore>,
        &connect,
        5000,
        &mut present,
    ) {
        eprintln!("the broker refused our CONNECT: {error:?}");
        return ExitCode::from(4);
    }
    println!("CONNACK accepted by {broker}; session_present={present}");

    let publish = OutgoingPublish {
        qos: rusty_rtos_mqtt_core::state::QoS::AtMostOnce,
        dup: false,
        retain: false,
        topic_name: b"kairos/interop",
        payload: b"hello from a Kairos stack",
        properties: &[],
    };
    if let Err(error) = mqtt.publish(&mut wire, &mut clock, None::<&mut NoStore>, &publish, 0) {
        eprintln!("the publish failed: {error:?}");
        return ExitCode::from(5);
    }
    println!("published to kairos/interop at QoS 0");

    // Hold the connection open across SEVERAL keep-alive periods. With a
    // two-second keep-alive, eight seconds is four of them: enough for the
    // broker to have dropped us if our PINGREQs were wrong, and enough for
    // us to have given up if its PINGRESPs were not understood.
    let mut counting = Counting::default();
    // Snapshot the byte counts as the SOAK begins. Subtracting an
    // assumed CONNACK length worked for mosquitto and gave 21 bytes for
    // ten two-byte replies against rumqttd, whose CONNACK is a
    // different size. A snapshot assumes nothing about any packet.
    let handshake_bytes = wire.received;
    let until = std::time::Instant::now() + std::time::Duration::from_secs(seconds);
    let mut passes = 0usize;
    let mut failures = 0usize;

    // A minute-by-minute tally, so a long soak is observable while it runs
    // and its SHAPE survives. A connection that pinged for five minutes and
    // went quiet for fifty-five passes on totals alone; per-minute counts
    // are what catch it, and `worst_quiet_minute` is the number that says
    // so in one figure.
    let mut minutes: Vec<(u64, usize, usize, usize)> = Vec::new();
    let started = std::time::Instant::now();
    let mut minute_mark = started;
    let mut minute_base = (0usize, 0usize);

    while std::time::Instant::now() < until {
        match mqtt.process_loop(&mut wire, &mut clock, None::<&mut NoStore>, &mut counting) {
            Ok(()) => passes = passes.saturating_add(1),
            Err(error) => {
                failures = failures.saturating_add(1);
                eprintln!("process_loop: {error:?}");
                if failures > 4 {
                    break;
                }
            }
        }

        if minute_mark.elapsed() >= std::time::Duration::from_secs(60) {
            let elapsed = started.elapsed().as_secs();
            let replies = wire.received.saturating_sub(minute_base.0);
            let loops = passes.saturating_sub(minute_base.1);
            minutes.push((elapsed, replies, loops, failures));
            println!(
                "  [{elapsed:>4}s] +{replies} bytes back, +{loops} loops, \
                 {failures} failures so far"
            );
            let _ = std::io::stdout().flush();
            minute_base = (wire.received, passes);
            minute_mark = std::time::Instant::now();
        }

        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    let held = seconds;
    let periods = held / u64::from(KEEP_ALIVE_SECONDS.max(1));

    // `process_loop` SWALLOWS a PINGRESP -- that is the documented
    // difference from `receive_loop`, which hands it to the application --
    // so the handler cannot count them and a zero there means nothing.
    //
    // The evidence is elsewhere and it is stronger: mosquitto disconnects
    // a client that misses its keep-alive by half again. With a 2s
    // keep-alive that is 3s. Surviving many periods with no failure, while
    // two-byte replies keep arriving, is the BROKER stating that our
    // PINGREQs were on time and well formed.
    let after_connack = wire.received.saturating_sub(handshake_bytes);
    println!(
        "held {held}s across ~{periods} keep-alive periods at {KEEP_ALIVE_SECONDS}s each"
    );
    println!(
        "  loops={passes} failures={failures} app-events={}",
        counting.events
    );
    println!(
        "  sent={} received={} -> {} bytes during the soak = {} two-byte PINGRESPs",
        wire.sent,
        wire.received,
        after_connack,
        after_connack / 2
    );
    println!(
        "  {broker} drops a client {:.1}s late; we held {held}s",
        f64::from(KEEP_ALIVE_SECONDS) * 1.5
    );

    // Being connected is not the same as being kept alive: an idle socket
    // nobody has noticed yet looks identical until the broker reaps it.
    // If we outlived two keep-alive periods, something must have come back.
    if held >= u64::from(KEEP_ALIVE_SECONDS) * 2 && after_connack == 0 {
        eprintln!(
            "nothing arrived during {periods} keep-alive periods: \
             the connection was idle, not alive"
        );
        return ExitCode::from(8);
    }

    // The SHAPE, not just the total. A minute in which nothing came back
    // is a minute the broker was about to reap us in.
    if !minutes.is_empty() {
        let quiet = minutes.iter().filter(|(_, replies, _, _)| *replies == 0).count();
        let worst = minutes
            .iter()
            .map(|(_, replies, _, _)| *replies)
            .min()
            .unwrap_or(0);
        println!(
            "  minutes={} quiet-minutes={quiet} fewest-bytes-in-a-minute={worst}",
            minutes.len()
        );
        if quiet > 0 {
            eprintln!(
                "{quiet} minute(s) with NOTHING back: the totals hide a stall"
            );
            return ExitCode::from(9);
        }
    }

    if failures > 0 {
        eprintln!("the connection did not stay healthy");
        return ExitCode::from(6);
    }
    if wire.received == 0 {
        eprintln!("the broker never said anything back");
        return ExitCode::from(7);
    }

    println!("MQTT INTEROP OK");
    ExitCode::SUCCESS
}

fn parse_v4(text: &str) -> Vec<u8> {
    text.split('.')
        .filter_map(|part| part.parse::<u8>().ok())
        .collect()
}
