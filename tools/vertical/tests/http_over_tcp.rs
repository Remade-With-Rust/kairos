//! `rusty_rtos_http`'s client, over `rusty_rtos_tcp`'s socket surface,
//! over a real TCP connection.
//!
//! # Why this exists
//!
//! Both packages are gated against their own C oracle, and that says
//! nothing about whether they fit together. `rusty_rtos_http` is written
//! against a `Transport` trait and has never heard of a socket;
//! `rusty_rtos_tcp` offers sockets and has never heard of HTTP. Each arm
//! agreeing with its own reference is compatible with the two of them
//! disagreeing at the seam.
//!
//! So this drives `HTTPClient_Send` — the coreHTTP function that sends a
//! request and parses the reply — through a `Transport` implemented on
//! top of a `Stack`, across a real TCP connection with a real three-way
//! handshake. The server is a second socket on the same interface,
//! serviced inside the transport's `recv`, which is where a real network
//! would have been doing its work anyway.
//!
//! # What it does NOT prove
//!
//! Both ends are ours. This is a COMPOSITION test, not interop: it shows
//! the seams line up, not that either package agrees with anyone else's
//! stack. Interop against a foreign peer needs a network and a peer, and
//! is the one thing in K7 that cannot be done on a workstation.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use rusty_rtos_http_core::client::send as http_send;
use rusty_rtos_http_core::receive::{Response, TransportRecv};
use rusty_rtos_http_core::request::{
    NO_USER_AGENT, RequestHeaders, RequestInfo, Status as HttpStatus,
};
use rusty_rtos_http_core::transport::Transport;

use rusty_rtos_tcp_core::socket::{ConnState, Host, Socket, Stack};

use smoltcp::iface::{Config, Interface, SocketSet, SocketStorage};
use smoltcp::phy::{Loopback, Medium};
use smoltcp::time::Instant;
use smoltcp::wire::{HardwareAddress, IpAddress, IpCidr};

const PORT: u16 = 8080;
const LOCAL: u16 = 49152;
const HERE: [u8; 4] = [127, 0, 0, 1];

const REPLY: &[u8] =
    b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\nContent-Type: text/plain\r\n\r\nhello, Kairos";

/// A host whose clock advances by what it is asked to wait.
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

/// The seam. `rusty_rtos_http` asks for a `Transport`; this supplies one
/// backed by a `rusty_rtos_tcp` socket.
///
/// It also SERVICES THE SERVER, inside `recv`. That is not a shortcut: on
/// a real network the peer is doing its work while the client waits, and
/// `recv` is precisely the moment the client is waiting. Doing it anywhere
/// else would mean interleaving by hand, which `HTTPClient_Send` — one
/// call that sends and then receives — gives no opportunity to do.
struct Wire<'a, 'buf> {
    stack: &'a mut Stack<'buf, Loopback>,
    client: Socket,
    server: Socket,
    host: Ticking,
    request: Vec<u8>,
    answered: bool,
    log: Vec<String>,
}

impl Wire<'_, '_> {
    /// Let the stack move frames, a bounded number of times.
    fn turn(&mut self, times: usize) {
        for _ in 0..times {
            let now = self.host.now_millis();
            self.stack.poll(now);
            self.host.wait(1);
        }
    }

    /// Read whatever the server has, and answer once the request is whole.
    fn service_server(&mut self) {
        let mut buffer = [0u8; 512];
        while let Ok(read) = self.stack.recv(self.server, &mut buffer) {
            if read == 0 {
                break;
            }
            self.request.extend_from_slice(&buffer[..read]);
        }

        // A request ends at the blank line. This one has no body, which is
        // why the terminator alone is enough to know it is whole.
        if !self.answered && self.request.windows(4).any(|w| w == b"\r\n\r\n") {
            self.log.push(format!(
                "  server got {} bytes, request line: {:?}",
                self.request.len(),
                String::from_utf8_lossy(self.request.split(|b| *b == b'\r').next().unwrap_or(&[]))
            ));
            let sent = self.stack.send(self.server, REPLY).unwrap_or(0);
            self.log.push(format!("  server replied {sent} bytes"));
            self.answered = true;
            self.turn(4);
        }
    }
}

impl Transport for Wire<'_, '_> {
    fn send(&mut self, data: &[u8]) -> i32 {
        // A negative is an error to coreHTTP and a short count is a
        // partial send it will loop on -- which is exactly what a socket
        // gives back, so the seam needs no cleverness here.
        let Ok(sent) = self.stack.send(self.client, data) else {
            self.log.push("  client send FAILED".to_owned());
            return -1;
        };
        self.log
            .push(format!("  client sent {sent} of {}", data.len()));
        self.turn(4);
        self.service_server();
        i32::try_from(sent).unwrap_or(i32::MAX)
    }
}

impl TransportRecv for Wire<'_, '_> {
    fn recv(&mut self, into: &mut [u8]) -> i32 {
        // The peer gets a turn before the client reads, which is what a
        // network would have been doing while the client was blocked.
        self.turn(2);
        self.service_server();
        self.turn(2);

        match self.stack.recv(self.client, into) {
            Ok(read) => {
                if read > 0 {
                    self.log.push(format!("  client received {read} bytes"));
                }
                i32::try_from(read).unwrap_or(i32::MAX)
            }
            // Nothing yet is not an error: coreHTTP's read loop retries
            // on a zero and gives up on a negative.
            Err(_) => 0,
        }
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

/// The whole vertical: an HTTP GET, over TCP, both halves ours.
#[test]
fn an_http_request_completes_over_our_own_tcp() {
    let mut storage = [SocketStorage::EMPTY; 4];
    let mut srx = [0u8; 512];
    let mut stx = [0u8; 512];
    let mut crx = [0u8; 512];
    let mut ctx = [0u8; 512];

    let sockets = SocketSet::new(&mut storage[..]);
    let mut stack = rig(sockets);
    let mut host = Ticking { now: 0 };

    let server = stack.tcp_socket(&mut srx, &mut stx);
    let client = stack.tcp_socket(&mut crx, &mut ctx);

    assert_eq!(
        stack.listen(server, PORT),
        rusty_rtos_tcp_core::socket::Status::Success
    );
    assert_eq!(
        stack.connect_blocking(&mut host, client, HERE, PORT, LOCAL, 200),
        rusty_rtos_tcp_core::socket::Status::Success,
        "the TCP handshake must complete before HTTP says a word"
    );
    // `connect_blocking` answers when the CLIENT is established, which is
    // one poll before the server sees the final ACK. That is not a defect
    // in either side -- it is what a three-way handshake looks like from
    // the initiator -- but a caller that assumes both ends move together
    // gets this wrong, so it is asserted rather than assumed.
    assert_eq!(stack.conn_state(server), ConnState::Connecting);
    stack.poll(host.now_millis());
    assert_eq!(
        stack.conn_state(server),
        ConnState::Established,
        "and one poll later the server has it too"
    );

    // Build the request with the HTTP package's own writer.
    let mut request_buffer = [0u8; 256];
    let mut request = RequestHeaders::new(&mut request_buffer);
    assert_eq!(
        request.initialize(&RequestInfo {
            method: b"GET",
            path: b"/status",
            host: b"127.0.0.1",
            flags: NO_USER_AGENT,
        }),
        HttpStatus::Success
    );

    let mut response_buffer = [0u8; 512];
    let mut response = Response::new(&mut response_buffer, 0);

    let mut wire = Wire {
        stack: &mut stack,
        client,
        server,
        host: Ticking { now: 1000 },
        request: Vec::new(),
        answered: false,
        log: Vec::new(),
    };

    let mut clock_ticks = 0u32;
    let mut clock = || {
        clock_ticks = clock_ticks.saturating_add(1);
        clock_ticks
    };

    let status = http_send(
        &mut wire,
        Some(&mut clock),
        &mut request,
        None,
        &mut response,
        0,
    );

    for line in &wire.log {
        println!("{line}");
    }

    assert_eq!(status, HttpStatus::Success, "the exchange must succeed");
    assert_eq!(response.status_code(), 200);
    assert_eq!(response.content_length(), 13);
    assert_eq!(response.header_count(), 2);
    assert_eq!(response.body(), b"hello, Kairos");
}

/// The request that actually went over the wire is the one the HTTP
/// package built — not one this test wrote out by hand. If the seam
/// mangled it, the server would have seen something else.
#[test]
fn the_bytes_on_the_wire_are_the_ones_http_wrote() {
    let mut storage = [SocketStorage::EMPTY; 4];
    let mut srx = [0u8; 512];
    let mut stx = [0u8; 512];
    let mut crx = [0u8; 512];
    let mut ctx = [0u8; 512];

    let sockets = SocketSet::new(&mut storage[..]);
    let mut stack = rig(sockets);
    let mut host = Ticking { now: 0 };

    let server = stack.tcp_socket(&mut srx, &mut stx);
    let client = stack.tcp_socket(&mut crx, &mut ctx);
    let _ = stack.listen(server, PORT);
    let _ = stack.connect_blocking(&mut host, client, HERE, PORT, LOCAL, 200);

    let mut request_buffer = [0u8; 256];
    let mut request = RequestHeaders::new(&mut request_buffer);
    let _ = request.initialize(&RequestInfo {
        method: b"GET",
        path: b"/status",
        host: b"127.0.0.1",
        flags: NO_USER_AGENT,
    });
    let expected = request.as_bytes().to_vec();

    let mut response_buffer = [0u8; 512];
    let mut response = Response::new(&mut response_buffer, 0);

    let mut wire = Wire {
        stack: &mut stack,
        client,
        server,
        host: Ticking { now: 1000 },
        request: Vec::new(),
        answered: false,
        log: Vec::new(),
    };
    let mut ticks = 0u32;
    let mut clock = || {
        ticks = ticks.saturating_add(1);
        ticks
    };
    let _ = http_send(
        &mut wire,
        Some(&mut clock),
        &mut request,
        None,
        &mut response,
        0,
    );

    assert_eq!(
        wire.request, expected,
        "the server must receive EXACTLY what the HTTP package wrote"
    );
    assert!(
        wire.request.starts_with(b"GET /status HTTP/1.1\r\n"),
        "and it must be a real request line: {:?}",
        String::from_utf8_lossy(&wire.request)
    );
}
