//! `rusty_rtos_mqtt`'s client, over `rusty_rtos_tcp`'s socket surface,
//! over a real TCP connection.
//!
//! # Why this exists
//!
//! The mission plan's K7 kill test is *"a Kairos device publishing over
//! our TCP to the Home Computer's `rumqttd`, one hour, zero lost
//! keep-alives"*. That needs a broker, a network and an hour. This is the
//! part of it that does not: **does `rusty_rtos_mqtt` compose with
//! `rusty_rtos_tcp` at all?**
//!
//! Both packages are gated against their own C oracle and neither has
//! heard of the other — coreMQTT names a `Transport` and coreTCP offers
//! sockets — so each agreeing with its own reference says nothing about
//! the seam between them. If a CONNECT cannot get out and a CONNACK
//! cannot get back over a real connection, the hour against `rumqttd` was
//! never going to happen, and finding that out needs neither.
//!
//! # What it does NOT prove
//!
//! The broker here is four bytes of hand-written CONNACK, not `rumqttd`.
//! This shows the packets move and the seam holds; it says nothing about
//! whether a real broker likes what we send. That is the kill test, and
//! it needs the network.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use rusty_rtos_mqtt_core::client::{Clock, MqttContext, Store};
use rusty_rtos_mqtt_core::connect::Connect;
use rusty_rtos_mqtt_core::reader::{Recv, Sent, Transport as MqttTransport};
use rusty_rtos_mqtt_core::writer::ConnectInfo;

use rusty_rtos_tcp_core::socket::{Host, Socket, Stack, Status as TcpStatus};

use smoltcp::iface::{Config, Interface, SocketSet, SocketStorage};
use smoltcp::phy::{Loopback, Medium};
use smoltcp::time::Instant;
use smoltcp::wire::{HardwareAddress, IpAddress, IpCidr};

const PORT: u16 = 1883;
const LOCAL: u16 = 49152;
const HERE: [u8; 4] = [127, 0, 0, 1];

/// An MQTT 5 CONNACK: type 0x20, three more bytes, no session present,
/// reason code Success, and an empty property section.
///
/// The property length byte is what makes this a version 5 packet rather
/// than a 3.1.1 one, and leaving it out is the usual way a hand-written
/// broker desynchronises a v5 client.
const CONNACK: [u8; 5] = [0x20, 0x03, 0x00, 0x00, 0x00];

struct Ticking {
    now: i64,
}

impl Host for Ticking {
    fn now_millis(&mut self) -> i64 {
        self.now
    }

    fn wait(&mut self, millis: u32) {
        self.now = self.now.saturating_add(i64::from(millis));
    }
}

struct Millis {
    now: u32,
}

impl Clock for Millis {
    fn now_ms(&mut self) -> u32 {
        self.now = self.now.saturating_add(1);
        self.now
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

/// The seam: `rusty_rtos_mqtt` asks for a `Transport`, and this supplies
/// one backed by a `rusty_rtos_tcp` socket.
///
/// As in the HTTP vertical, the peer is serviced inside `recv` — the one
/// moment the client is waiting, and where a real network would have been
/// doing its work.
struct Wire<'a, 'buf> {
    stack: &'a mut Stack<'buf, Loopback>,
    client: Socket,
    broker: Socket,
    host: Ticking,
    seen: Vec<u8>,
    answered: bool,
    log: Vec<String>,
}

impl Wire<'_, '_> {
    fn turn(&mut self, times: usize) {
        for _ in 0..times {
            let now = self.host.now_millis();
            self.stack.poll(now);
            self.host.wait(1);
        }
    }

    /// Read what the broker has, and answer a CONNECT with a CONNACK.
    fn service_broker(&mut self) {
        let mut buffer = [0u8; 512];
        while let Ok(read) = self.stack.recv(self.broker, &mut buffer) {
            if read == 0 {
                break;
            }
            self.seen.extend_from_slice(&buffer[..read]);
        }

        // A CONNECT is packet type 1 in the high nibble.
        if !self.answered && self.seen.first().map(|b| b >> 4) == Some(1) {
            self.log.push(format!(
                "  broker got {} bytes, type={} (CONNECT)",
                self.seen.len(),
                self.seen[0] >> 4
            ));
            let sent = self.stack.send(self.broker, &CONNACK).unwrap_or(0);
            self.log
                .push(format!("  broker replied {sent} bytes (CONNACK)"));
            self.answered = true;
            self.turn(4);
        }
    }
}

impl MqttTransport for Wire<'_, '_> {
    fn recv(&mut self, into: &mut [u8]) -> Recv {
        self.turn(2);
        self.service_broker();
        self.turn(2);

        match self.stack.recv(self.client, into) {
            Ok(0) => Recv::Nothing,
            Ok(read) => {
                self.log.push(format!("  client received {read} bytes"));
                Recv::Bytes(read)
            }
            // Nothing yet is not a failure: coreMQTT retries on Nothing
            // and gives up on Failed, so answering Failed here would end
            // the connection the first time a packet was in flight.
            Err(_) => Recv::Nothing,
        }
    }

    fn send(&mut self, bytes: &[u8]) -> Sent {
        let Ok(sent) = self.stack.send(self.client, bytes) else {
            self.log.push("  client send FAILED".to_owned());
            return Sent::Failed;
        };
        self.log
            .push(format!("  client sent {sent} of {}", bytes.len()));
        self.turn(4);
        self.service_broker();
        Sent::Bytes(sent)
    }
}

fn rig(sockets: SocketSet<'_>) -> Stack<'_, Loopback> {
    let mut device = Loopback::new(Medium::Ip);
    let mut config = Config::new(HardwareAddress::Ip);
    config.random_seed = 1;
    let mut iface = Interface::new(config, &mut device, Instant::from_millis(0));
    iface.update_ip_addrs(|addrs| {
        addrs
            .push(IpCidr::new(IpAddress::v4(127, 0, 0, 1), 8))
            .unwrap();
    });
    Stack::new(iface, sockets, device)
}

/// A CONNECT out and a CONNACK back, over a real TCP connection.
#[test]
fn an_mqtt_connect_completes_over_our_own_tcp() {
    let mut storage = [SocketStorage::EMPTY; 4];
    let mut brx = [0u8; 512];
    let mut btx = [0u8; 512];
    let mut crx = [0u8; 512];
    let mut ctx = [0u8; 512];

    let sockets = SocketSet::new(&mut storage[..]);
    let mut stack = rig(sockets);
    let mut host = Ticking { now: 0 };

    let broker = stack.tcp_socket(&mut brx, &mut btx);
    let client = stack.tcp_socket(&mut crx, &mut ctx);

    assert_eq!(stack.listen(broker, PORT), TcpStatus::Success);
    assert_eq!(
        stack.connect_blocking(&mut host, client, HERE, PORT, LOCAL, 200),
        TcpStatus::Success,
        "TCP must be up before MQTT says a word"
    );

    let connect = Connect {
        info: ConnectInfo {
            clean_session: true,
            keep_alive_seconds: 60,
            username: None,
            password: None,
        },
        client_identifier: b"kairos-vertical",
        properties: &[],
        will: None,
    };

    let mut network = [0u8; 256];
    let mut mqtt = MqttContext::new(&mut network);

    let mut wire = Wire {
        stack: &mut stack,
        client,
        broker,
        host: Ticking { now: 1000 },
        seen: Vec::new(),
        answered: false,
        log: Vec::new(),
    };
    let mut clock = Millis { now: 0 };
    let mut present = false;

    let answer = mqtt.connect(
        &mut wire,
        &mut clock,
        None::<&mut NoStore>,
        &connect,
        500,
        &mut present,
    );

    for line in &wire.log {
        println!("{line}");
    }

    assert!(
        answer.is_ok(),
        "the CONNECT/CONNACK exchange must complete: {answer:?}"
    );
    assert!(!present, "a clean session must not report one present");

    // The packet the broker received is the one coreMQTT built, and it is
    // a real CONNECT: type 1, then the protocol name.
    assert_eq!(wire.seen[0] >> 4, 1, "packet type 1 is CONNECT");
    assert!(
        wire.seen.windows(4).any(|w| w == b"MQTT"),
        "and it carries the protocol name: {:?}",
        &wire.seen[..wire.seen.len().min(16)]
    );
    assert!(
        wire.seen.windows(15).any(|w| w == b"kairos-vertical"),
        "and the client identifier this test asked for"
    );
}

/// The keep-alive the application asked for is the one on the wire.
///
/// It is two bytes, big-endian, in the variable header — and it is what
/// the one-hour kill test will be counting. A client that sent a
/// different one would keep the connection alive on a schedule nobody
/// agreed to.
#[test]
fn the_keep_alive_on_the_wire_is_the_one_asked_for() {
    let mut storage = [SocketStorage::EMPTY; 4];
    let mut brx = [0u8; 512];
    let mut btx = [0u8; 512];
    let mut crx = [0u8; 512];
    let mut ctx = [0u8; 512];

    let sockets = SocketSet::new(&mut storage[..]);
    let mut stack = rig(sockets);
    let mut host = Ticking { now: 0 };

    let broker = stack.tcp_socket(&mut brx, &mut btx);
    let client = stack.tcp_socket(&mut crx, &mut ctx);
    let _ = stack.listen(broker, PORT);
    let _ = stack.connect_blocking(&mut host, client, HERE, PORT, LOCAL, 200);

    let connect = Connect {
        info: ConnectInfo {
            clean_session: true,
            keep_alive_seconds: 0x0102,
            username: None,
            password: None,
        },
        client_identifier: b"kairos",
        properties: &[],
        will: None,
    };

    let mut network = [0u8; 256];
    let mut mqtt = MqttContext::new(&mut network);
    let mut wire = Wire {
        stack: &mut stack,
        client,
        broker,
        host: Ticking { now: 1000 },
        seen: Vec::new(),
        answered: false,
        log: Vec::new(),
    };
    let mut clock = Millis { now: 0 };
    let mut present = false;
    let _ = mqtt.connect(
        &mut wire,
        &mut clock,
        None::<&mut NoStore>,
        &connect,
        500,
        &mut present,
    );

    assert!(
        wire.seen.windows(2).any(|w| w == [0x01, 0x02]),
        "the keep-alive must be on the wire big-endian: {:?}",
        &wire.seen[..wire.seen.len().min(24)]
    );
}
