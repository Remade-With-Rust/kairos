#![no_std]
#![no_main]
#![cfg_attr(target_arch = "xtensa", feature(asm_experimental_arch))]
//! K7 on silicon: an MQTT session from a XIAO ESP32-S3, entirely on Kairos.
//!
//! ```text
//!   rusty_rtos_mqtt   (coreMQTT, remade)        -- the session
//!   rusty_rtos_tcp    (FreeRTOS-Plus-TCP API)   -- the socket
//!   smoltcp + DHCP                               -- the IP engine
//!   esp-radio Wi-Fi   (the blob)                 -- the link
//!   rusty_rtos_kernel + rusty_rtos_port-xtensa   -- every task, queue, timer
//!   heap_4            (rusty_rtos_heap-core)     -- every allocation
//! ```
//!
//! The K7 kill test held MQTT for an hour over our TCP on a TAP device on a
//! workstation. This is the same client, the same TCP and the same broker
//! on the far side, with a radio and a real kernel in between.
//!
//! # What it needs at build time
//!
//! * `KAIROS_WIFI_SSID`, `KAIROS_WIFI_PASSWORD` -- a 2.4 GHz WPA2 network.
//! * `KAIROS_MQTT_BROKER` -- `a.b.c.d:port` (default: the DHCP gateway, 1883).
//! * `KAIROS_MQTT_SECONDS` -- how long to hold the session (default 60).
//!
//! None is printed, and none is committed: `option_env!` puts them in the
//! local build only.
//!
//! # What it checks
//!
//! The session must survive the whole hold with NO failed `process_loop`,
//! and the broker must answer keep-alives throughout. As in K7, the PINGRESP
//! count is a counter, not the evidence: the broker reaps a client that
//! misses 1.5 keep-alive periods, so a session still open at the end IS the
//! evidence that none was missed.

extern crate alloc;

#[path = "../../../../../rusty_rtos_port/firmware/xiao-s3-wifi/src/kernel.rs"]
mod kernel;
#[path = "../../../../../rusty_rtos_port/firmware/xiao-s3-wifi/src/libc_heap.rs"]
mod libc_heap;

use rusty_rtos_port_esp_radio as adapter;

use alloc::format;
use core::ffi::c_void;
use core::future::Future;
use core::pin::pin;
use core::task::{Context, Poll, Waker};

use esp_backtrace as _;
use esp_println::println;

use esp_radio::wifi::sta::StationConfig;
use esp_radio::wifi::{AuthenticationMethodConfig, Config as WifiConfig, WifiController};

use smoltcp::iface::{Config, Interface, SocketSet, SocketStorage};
use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::socket::dhcpv4;
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, HardwareAddress, IpCidr, Ipv4Address};

use rusty_rtos_mqtt_core::client::{AckReply, Clock, Event, EventHandler, MqttContext, Store};
use rusty_rtos_mqtt_core::connect::Connect;
use rusty_rtos_mqtt_core::outpublish::OutgoingPublish;
use rusty_rtos_mqtt_core::reader::{Recv, Sent, Transport as MqttTransport};
use rusty_rtos_mqtt_core::writer::ConnectInfo;
use rusty_rtos_tcp_core::socket::{Host, Socket, Stack, Status as TcpStatus};

esp_bootloader_esp_idf::esp_app_desc!();

#[global_allocator]
static ALLOC: libc_heap::Heap4Alloc = libc_heap::Heap4Alloc;

esp_radio_rtos_driver::register_scheduler_implementation!(
    static SCHEDULER: adapter::Scheduler = adapter::Scheduler
);
esp_radio_rtos_driver::register_semaphore_implementation!(adapter::Semaphore);
esp_radio_rtos_driver::register_queue_implementation!(adapter::Queue);
esp_radio_rtos_driver::register_timer_implementation!(adapter::Timer);
esp_radio_rtos_driver::register_wait_queue_implementation!(adapter::WaitQueue);

const KEEP_ALIVE_SECONDS: u16 = 5;

fn block_on<F: Future>(fut: F) -> F::Output {
    let mut fut = pin!(fut);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return v;
        }
        kernel::sleep_ticks(1);
    }
}

fn now_ms() -> i64 {
    (kernel::now_us() / 1000) as i64
}

// ------------------------------------------------------- the link device --

/// smoltcp's `Device` over the radio's station interface.
struct WifiDevice(esp_radio::wifi::Interface);

struct Rx(esp_radio::wifi::WifiRxToken);
struct Tx(esp_radio::wifi::WifiTxToken);

impl RxToken for Rx {
    fn consume<R, F: FnOnce(&[u8]) -> R>(self, f: F) -> R {
        self.0.consume_token(|buf| f(buf))
    }
}

impl TxToken for Tx {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, len: usize, f: F) -> R {
        self.0.consume_token(len, f)
    }
}

impl Device for WifiDevice {
    type RxToken<'a> = Rx;
    type TxToken<'a> = Tx;

    fn receive(&mut self, _t: Instant) -> Option<(Rx, Tx)> {
        self.0.receive().map(|(r, t)| (Rx(r), Tx(t)))
    }

    fn transmit(&mut self, _t: Instant) -> Option<Tx> {
        self.0.transmit().map(Tx)
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ethernet;
        caps.max_transmission_unit = 1514;
        caps
    }
}

// -------------------------------------------------- the seams, on Kairos --

/// `rusty_rtos_tcp`'s blocking seam: a wait is a kernel sleep, which is what
/// hands the CPU to the radio's own tasks.
struct KairosHost;
impl Host for KairosHost {
    fn now_millis(&mut self) -> i64 {
        now_ms()
    }
    fn wait(&mut self, millis: u32) {
        kernel::sleep_ticks(u64::from(millis.max(1)));
    }
}

struct Millis;
impl Clock for Millis {
    fn now_ms(&mut self) -> u32 {
        now_ms() as u32
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

#[derive(Default)]
struct Counting {
    events: u32,
    pingresp: u32,
}
impl EventHandler for Counting {
    fn on_event(&mut self, event: &Event<'_>, _reply: &mut AckReply<'_>) -> bool {
        self.events = self.events.saturating_add(1);
        let name = format!("{event:?}");
        if name.contains("PingResp") || name.contains("Pingresp") {
            self.pingresp = self.pingresp.saturating_add(1);
        }
        true
    }
}

struct Wire<'a, 'buf> {
    stack: &'a mut Stack<'buf, WifiDevice>,
    socket: Socket,
    sent: usize,
    received: usize,
}

impl MqttTransport for Wire<'_, '_> {
    fn recv(&mut self, into: &mut [u8]) -> Recv {
        self.stack.poll(now_ms());
        match self.stack.recv(self.socket, into) {
            Ok(0) | Err(_) => Recv::Nothing,
            Ok(read) => {
                self.received = self.received.saturating_add(read);
                Recv::Bytes(read)
            }
        }
    }
    fn send(&mut self, bytes: &[u8]) -> Sent {
        let Ok(sent) = self.stack.send(self.socket, bytes) else {
            return Sent::Failed;
        };
        self.sent = self.sent.saturating_add(sent);
        self.stack.poll(now_ms());
        Sent::Bytes(sent)
    }
}

// ------------------------------------------------------------------ main --

#[esp_hal::main]
fn main() -> ! {
    let p =
        esp_hal::init(esp_hal::Config::default().with_cpu_clock(esp_hal::clock::CpuClock::max()));

    println!();
    println!("=== K7 on silicon: MQTT / our TCP / Wi-Fi, all on Kairos (XIAO ESP32-S3) ===");

    let (Some(ssid), Some(password)) = (
        option_env!("KAIROS_WIFI_SSID"),
        option_env!("KAIROS_WIFI_PASSWORD"),
    ) else {
        fail("built without KAIROS_WIFI_SSID / KAIROS_WIFI_PASSWORD -- see the crate docs");
    };
    let hold_s: u64 = option_env!("KAIROS_MQTT_SECONDS")
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);

    if !kernel::boot() || !kernel::start_tick(p.SYSTIMER) {
        fail("the kernel would not start");
    }

    let mut controller = match WifiController::new(p.WIFI, Default::default()) {
        Ok(c) => c,
        Err(_) => fail("WifiController::new"),
    };
    let (Ok(s), Ok(pw)) = (ssid.try_into(), password.try_into()) else {
        fail("the SSID or password does not fit the driver's types");
    };
    let conf = WifiConfig::Station(
        StationConfig::default()
            .with_ssid(s)
            .with_authentication(AuthenticationMethodConfig::Wpa2Personal(pw)),
    );
    if controller.set_config(&conf).is_err() {
        fail("set_config");
    }
    let t = kernel::now_us();
    match block_on(controller.connect_async()) {
        Ok(info) => println!(
            "LINK associated in {} ms, channel={}",
            (kernel::now_us() - t) / 1000,
            info.channel
        ),
        Err(_) => fail("association refused (wrong password, or not a 2.4 GHz WPA2 network)"),
    }

    // ------------------------------------------------------------- DHCP --
    let wifi = esp_radio::wifi::Interface::station();
    let mac = wifi.mac_address();
    let mut device = WifiDevice(wifi);
    let mut config = Config::new(HardwareAddress::Ethernet(EthernetAddress(mac)));
    config.random_seed = kernel::now_us();
    let mut iface = Interface::new(config, &mut device, Instant::from_millis(now_ms()));

    let mut dhcp_storage = [SocketStorage::EMPTY; 1];
    let mut dhcp_set = SocketSet::new(&mut dhcp_storage[..]);
    let dhcp = dhcp_set.add(dhcpv4::Socket::new());
    let t = now_ms();
    let (addr, gateway) = loop {
        iface.poll(Instant::from_millis(now_ms()), &mut device, &mut dhcp_set);
        if let Some(dhcpv4::Event::Configured(c)) = dhcp_set.get_mut::<dhcpv4::Socket>(dhcp).poll()
        {
            let addr = c.address;
            iface.update_ip_addrs(|a| {
                a.clear();
                let _ = a.push(IpCidr::Ipv4(addr));
            });
            if let Some(gw) = c.router {
                let _ = iface.routes_mut().add_default_ipv4_route(gw);
            }
            break (addr, c.router);
        }
        if now_ms() - t > 20_000 {
            fail("no DHCP lease in 20 s");
        }
        kernel::sleep_ticks(10);
    };
    println!("IP    {addr} gateway={gateway:?} after {} ms", now_ms() - t);

    // ----------------------------------------------------- TCP + MQTT --
    let (broker, port) = match option_env!("KAIROS_MQTT_BROKER").and_then(parse_endpoint) {
        Some(e) => e,
        None => match gateway {
            Some(g) => (g.octets(), 1883),
            None => fail("no KAIROS_MQTT_BROKER and no gateway to default to"),
        },
    };

    let mut storage = [SocketStorage::EMPTY; 2];
    let mut rx = [0u8; 4096];
    let mut tx = [0u8; 4096];
    let mut stack = Stack::new(iface, SocketSet::new(&mut storage[..]), device);
    let socket = stack.tcp_socket(&mut rx, &mut tx);
    let mut host = KairosHost;
    if stack.connect_blocking(&mut host, socket, broker, port, 49153, 5000) != TcpStatus::Success {
        fail("could not open TCP to the broker");
    }
    println!(
        "TCP   up to {}.{}.{}.{}:{port}",
        broker[0], broker[1], broker[2], broker[3]
    );

    let mut network = [0u8; 2048];
    let mut mqtt = MqttContext::new(&mut network);
    let mut wire = Wire {
        stack: &mut stack,
        socket,
        sent: 0,
        received: 0,
    };
    let mut clock = Millis;
    let connect = Connect {
        info: ConnectInfo {
            clean_session: true,
            keep_alive_seconds: KEEP_ALIVE_SECONDS,
            username: None,
            password: None,
        },
        client_identifier: b"kairos-xiao-s3",
        properties: &[],
        will: None,
    };
    let mut present = false;
    if mqtt
        .connect(
            &mut wire,
            &mut clock,
            None::<&mut NoStore>,
            &connect,
            5000,
            &mut present,
        )
        .is_err()
    {
        fail("the broker refused our CONNECT");
    }
    println!("MQTT  CONNACK accepted");
    let publish = OutgoingPublish {
        qos: rusty_rtos_mqtt_core::state::QoS::AtMostOnce,
        dup: false,
        retain: false,
        topic_name: b"kairos/xiao-s3",
        payload: b"hello from a Kairos stack on an ESP32-S3",
        properties: &[],
    };
    if mqtt
        .publish(&mut wire, &mut clock, None::<&mut NoStore>, &publish, 0)
        .is_err()
    {
        fail("the publish failed");
    }
    println!("MQTT  published to kairos/xiao-s3 at QoS 0");

    let mut counting = Counting::default();
    let start = now_ms();
    let mut passes = 0u32;
    let mut failures = 0u32;
    let mut next_report = start + 60_000;
    while now_ms() - start < (hold_s * 1000) as i64 {
        match mqtt.process_loop(&mut wire, &mut clock, None::<&mut NoStore>, &mut counting) {
            Ok(()) => passes += 1,
            Err(_) => {
                failures += 1;
                if failures > 4 {
                    break;
                }
            }
        }
        if now_ms() >= next_report {
            println!(
                "HOLD  {}s loops={passes} pingresp={} failures={failures} heap_min_free={}",
                (now_ms() - start) / 1000,
                counting.pingresp,
                libc_heap::minimum_ever_free()
            );
            next_report += 60_000;
        }
        kernel::sleep_ticks(20);
    }
    let held = (now_ms() - start) / 1000;
    let open = wire.stack.is_connected(socket);
    println!(
        "HOLD  done {held}s loops={passes} pingresp={} events={} failures={failures} bytes_out={} bytes_in={} still_connected={open}",
        counting.pingresp, counting.events, wire.sent, wire.received
    );
    if failures == 0 && open && held as u64 >= hold_s {
        println!("RESULT: PASS -- MQTT held {held}s over our TCP over Wi-Fi, on Kairos");
    } else {
        println!("RESULT: FAIL");
    }
    park();
}

fn parse_endpoint(s: &str) -> Option<([u8; 4], u16)> {
    let (ip, port) = s.split_once(':')?;
    let ip: Ipv4Address = ip.parse().ok()?;
    Some((ip.octets(), port.parse().ok()?))
}

fn fail(why: &str) -> ! {
    println!("RESULT: FAIL -- {why}");
    park();
}

fn park() -> ! {
    loop {
        core::hint::spin_loop();
    }
}
