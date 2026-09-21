//! INTEROP: our stack against a FOREIGN one.
//!
//! Every other test in this crate has ours on both ends. That proves the
//! Kairos packages compose with each other and nothing at all about
//! whether they compose with anybody else — two implementations that
//! share an author share its misreadings.
//!
//! This one does not. On one side of a TAP device sits
//! `rusty_rtos_http`'s client over `rusty_rtos_tcp`'s sockets over
//! smoltcp. On the other sits **the Linux kernel's TCP stack**, carrying
//! a stock Python HTTP server. Neither end has ever seen the other's
//! source.
//!
//! Driven by `tools/vertical/interop.sh`, which creates the TAP, brings
//! up the peer address, starts the server and tears it all down again.
//! It needs root and `/dev/net/tun`, which is why it is a binary rather
//! than a `cargo test`: an ordinary `cargo test` must not need either.
//!
//! Exit status is the verdict: zero if the exchange completed and the
//! response parsed, non-zero otherwise, with the reason on stderr.

use std::env;
use std::process::ExitCode;

use rusty_rtos_http_core::client::send as http_send;
use rusty_rtos_http_core::receive::{Response, TransportRecv};
use rusty_rtos_http_core::request::{
    NO_USER_AGENT, RequestHeaders, RequestInfo, Status as HttpStatus,
};
use rusty_rtos_http_core::transport::Transport;

use rusty_rtos_tcp_core::socket::{Host, Socket, Stack, Status as TcpStatus};

use smoltcp::iface::{Config, Interface, SocketSet, SocketStorage};
use smoltcp::phy::{Medium, TunTapInterface};
use smoltcp::time::Instant;
use smoltcp::wire::{HardwareAddress, IpAddress, IpCidr};

/// A real clock, because the peer is a real stack with real timers. The
/// loopback tests could use a counter; this cannot.
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

/// The seam, with NO server to service: the peer is the Linux kernel and
/// it services itself. That absence is the point of this file.
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

impl Transport for Wire<'_, '_> {
    fn send(&mut self, data: &[u8]) -> i32 {
        let Ok(sent) = self.stack.send(self.socket, data) else {
            return -1;
        };
        self.sent = self.sent.saturating_add(sent);
        self.turn();
        i32::try_from(sent).unwrap_or(i32::MAX)
    }
}

impl TransportRecv for Wire<'_, '_> {
    fn recv(&mut self, into: &mut [u8]) -> i32 {
        self.turn();
        match self.stack.recv(self.socket, into) {
            Ok(read) => {
                self.received = self.received.saturating_add(read);
                i32::try_from(read).unwrap_or(i32::MAX)
            }
            // Nothing yet is a zero, which coreHTTP retries on. A negative
            // would end the read loop the first time a packet was in
            // flight, and over a real network that is most of the time.
            Err(_) => {
                self.host.wait(2);
                0
            }
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let tap = args.get(1).cloned().unwrap_or_else(|| "kairos0".to_owned());
    let ours: Vec<u8> = parse_v4(args.get(2).map_or("192.168.69.2", String::as_str));
    let theirs: Vec<u8> = parse_v4(args.get(3).map_or("192.168.69.1", String::as_str));
    let port: u16 = args
        .get(4)
        .and_then(|value| value.parse().ok())
        .unwrap_or(8080);

    let device = match TunTapInterface::new(&tap, Medium::Ethernet) {
        Ok(device) => device,
        Err(error) => {
            eprintln!("cannot open {tap}: {error}");
            return ExitCode::from(2);
        }
    };
    let mut device = device;

    let mut config = Config::new(HardwareAddress::Ethernet(
        smoltcp::wire::EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]),
    ));
    config.random_seed = 0x1234_5678;
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
    let connected = stack.connect_blocking(&mut host, socket, peer, port, 49152, 5000);
    if connected != TcpStatus::Success {
        eprintln!("could not connect to {theirs:?}:{port}: {connected:?}");
        return ExitCode::from(3);
    }
    println!("connected to a FOREIGN stack at {theirs:?}:{port}");

    let host_text = format!("{}.{}.{}.{}", theirs[0], theirs[1], theirs[2], theirs[3]);
    let mut request_buffer = [0u8; 256];
    let mut request = RequestHeaders::new(&mut request_buffer);
    if request.initialize(&RequestInfo {
        method: b"GET",
        path: b"/",
        host: host_text.as_bytes(),
        flags: NO_USER_AGENT,
    }) != HttpStatus::Success
    {
        eprintln!("could not build the request");
        return ExitCode::from(4);
    }

    let mut response_buffer = [0u8; 4096];
    let mut response = Response::new(&mut response_buffer, 0);

    let mut wire = Wire {
        stack: &mut stack,
        socket,
        host: Wall {
            started: std::time::Instant::now(),
        },
        sent: 0,
        received: 0,
    };

    let start = std::time::Instant::now();
    let mut clock = || u32::try_from(start.elapsed().as_millis()).unwrap_or(u32::MAX);

    let status = http_send(
        &mut wire,
        Some(&mut clock),
        &mut request,
        None,
        &mut response,
        0,
    );

    println!("sent={} received={}", wire.sent, wire.received);
    println!(
        "status={status:?} code={} headers={} content-length={} body={} bytes",
        response.status_code(),
        response.header_count(),
        response.content_length(),
        response.body().len()
    );

    if status != HttpStatus::Success {
        eprintln!("the exchange did not complete: {status:?}");
        return ExitCode::from(5);
    }
    if response.status_code() != 200 {
        eprintln!("the server did not answer 200: {}", response.status_code());
        return ExitCode::from(6);
    }
    if response.header_count() == 0 {
        eprintln!("no headers were parsed out of a real server's reply");
        return ExitCode::from(7);
    }

    println!("INTEROP OK");
    ExitCode::SUCCESS
}

fn parse_v4(text: &str) -> Vec<u8> {
    text.split('.')
        .filter_map(|part| part.parse::<u8>().ok())
        .collect()
}
