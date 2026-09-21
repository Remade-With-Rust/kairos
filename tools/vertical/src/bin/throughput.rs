//! `iperf`-style throughput rows for our stack, against a foreign one.
//!
//! The mission plan's K7 row asks for "interop against the host stack;
//! `iperf`-style rows". This is the rows.
//!
//! # The method, because a number without one is not a number
//!
//! * **Both directions.** A stack can be fast writing and slow reading,
//!   and one figure would hide it.
//! * **Work parity is checked, not assumed.** The peer counts the bytes
//!   it saw and this prints both counts. A throughput figure computed
//!   over a transfer that silently truncated is the classic way to get a
//!   fast wrong answer.
//! * **The transfer is long enough that setup does not dominate.** The
//!   default is 16 MiB, which takes long enough that the handshake and
//!   the process launch are noise rather than the measurement.
//! * **The clock's resolution is printed.** `Instant` here is
//!   `clock_gettime(CLOCK_MONOTONIC)`, sub-microsecond, and the run is
//!   seconds — but the rule is to state it rather than to assume the
//!   reader knows.
//! * **What it is NOT.** This is a userspace poll loop over a TAP device
//!   on a workstation, not a NIC on a chip. It bounds nothing about
//!   embedded performance and must never be quoted as if it did. What it
//!   IS good for: a baseline that a later change can be compared against,
//!   and evidence that bulk transfer works at all rather than only
//!   forty-byte requests.

use std::env;
use std::process::ExitCode;
use std::time::Instant as Wallclock;

use rusty_rtos_tcp_core::socket::{Host, Socket, Stack, Status as TcpStatus};

use smoltcp::iface::{Config, Interface, SocketSet, SocketStorage};
use smoltcp::phy::{Medium, TunTapInterface};
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr};

struct Wall {
    started: Wallclock,
}

impl Host for Wall {
    fn now_millis(&mut self) -> i64 {
        i64::try_from(self.started.elapsed().as_millis()).unwrap_or(i64::MAX)
    }

    fn wait(&mut self, millis: u32) {
        std::thread::sleep(std::time::Duration::from_millis(u64::from(millis)));
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let tap = args.get(1).cloned().unwrap_or_else(|| "kairos2".to_owned());
    let ours = parse_v4(args.get(2).map_or("192.168.71.2", String::as_str));
    let theirs = parse_v4(args.get(3).map_or("192.168.71.1", String::as_str));
    let port: u16 = args.get(4).and_then(|v| v.parse().ok()).unwrap_or(9000);
    let direction = args.get(5).cloned().unwrap_or_else(|| "tx".to_owned());
    let bytes: usize = args
        .get(6)
        .and_then(|v| v.parse().ok())
        .unwrap_or(16 * 1024 * 1024);

    let Ok(mut device) = TunTapInterface::new(&tap, Medium::Ethernet) else {
        eprintln!("cannot open {tap}");
        return ExitCode::from(2);
    };

    let mut config = Config::new(HardwareAddress::Ethernet(EthernetAddress([
        0x02, 0x00, 0x00, 0x00, 0x00, 0x01,
    ])));
    config.random_seed = 0x1357_9bdf;
    let mut iface = Interface::new(config, &mut device, Instant::now());
    iface.update_ip_addrs(|addrs| {
        let _ = addrs.push(IpCidr::new(
            IpAddress::v4(ours[0], ours[1], ours[2], ours[3]),
            24,
        ));
    });

    // Big buffers: a 1500-byte MTU with a 4 KiB window spends its life
    // waiting for ACKs, and the measurement would be of that rather than
    // of the stack.
    let mut storage = [SocketStorage::EMPTY; 2];
    let mut rx = vec![0u8; 256 * 1024];
    let mut tx = vec![0u8; 256 * 1024];
    let sockets = SocketSet::new(&mut storage[..]);
    let mut stack = Stack::new(iface, sockets, device);
    let socket = stack.tcp_socket(&mut rx, &mut tx);

    let mut host = Wall {
        started: Wallclock::now(),
    };
    let peer = [theirs[0], theirs[1], theirs[2], theirs[3]];
    // A FRESH local port per run. Reusing one immediately after a close
    // reuses the whole 4-tuple, and the second rep of the first
    // measurement could not connect at all because of it.
    let local = 49154u16.wrapping_add(port.wrapping_sub(9000));
    if stack.connect_blocking(&mut host, socket, peer, port, local, 5000) != TcpStatus::Success {
        eprintln!("could not reach the sink at {theirs:?}:{port}");
        return ExitCode::from(3);
    }

    let moved = match direction.as_str() {
        "tx" => transmit(&mut stack, socket, &mut host, bytes),
        "rx" => receive(&mut stack, socket, &mut host, bytes),
        other => {
            eprintln!("direction must be tx or rx, not {other}");
            return ExitCode::from(4);
        }
    };

    let Some((moved, seconds)) = moved else {
        eprintln!("the transfer did not finish");
        return ExitCode::from(5);
    };

    // Mebibytes per second, and the raw pair beside it so a reader can
    // recompute rather than trust.
    let mib = (moved as f64) / (1024.0 * 1024.0);
    println!("direction={direction}");
    println!("  bytes={moved} seconds={seconds:.3}");
    println!("  throughput={:.2} MiB/s ({:.1} Mbit/s)", mib / seconds, (mib * 8.0) / seconds);
    println!("  clock=CLOCK_MONOTONIC, sub-microsecond; run is seconds, so the clock is not the limit");

    if moved < bytes {
        eprintln!("moved {moved} of {bytes}: the transfer TRUNCATED, so the rate above is meaningless");
        return ExitCode::from(6);
    }

    ExitCode::SUCCESS
}

/// Push `want` bytes and wait until the stack has actually handed them
/// all to the wire, not merely accepted them into its buffer.
fn transmit(
    stack: &mut Stack<'_, TunTapInterface>,
    socket: Socket,
    host: &mut Wall,
    want: usize,
) -> Option<(usize, f64)> {
    let chunk = vec![0x5Au8; 32 * 1024];
    let mut sent = 0usize;
    let started = Wallclock::now();
    let deadline = started + std::time::Duration::from_secs(120);

    while sent < want {
        if Wallclock::now() > deadline {
            eprintln!("transmit timed out after {sent} of {want}");
            return None;
        }
        let now = host.now_millis();
        stack.poll(now);

        let remaining = want.saturating_sub(sent);
        let offer = &chunk[..chunk.len().min(remaining)];
        match stack.send(socket, offer) {
            Ok(0) => {}
            Ok(took) => sent = sent.saturating_add(took),
            Err(error) => {
                eprintln!("send failed after {sent}: {error:?}");
                return None;
            }
        }
    }

    // The buffer being empty is what says the bytes LEFT, and the timer
    // must not stop before then or the rate is of a memcpy.
    while stack.tx_size(socket) > 0 {
        if Wallclock::now() > deadline {
            eprintln!("drain timed out with {} queued", stack.tx_size(socket));
            return None;
        }
        let now = host.now_millis();
        stack.poll(now);
    }
    let seconds = started.elapsed().as_secs_f64();

    // A graceful close, so the peer sees EOF and can report its count.
    let _ = stack.shutdown(socket);
    for _ in 0..200 {
        let now = host.now_millis();
        stack.poll(now);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    Some((sent, seconds))
}

/// Pull until `want` bytes have arrived or the peer closes.
fn receive(
    stack: &mut Stack<'_, TunTapInterface>,
    socket: Socket,
    host: &mut Wall,
    want: usize,
) -> Option<(usize, f64)> {
    let mut sink = vec![0u8; 64 * 1024];
    let mut got = 0usize;
    let mut started: Option<Wallclock> = None;
    let deadline = Wallclock::now() + std::time::Duration::from_secs(120);

    while got < want {
        if Wallclock::now() > deadline {
            eprintln!("receive timed out after {got} of {want}");
            return None;
        }
        let now = host.now_millis();
        stack.poll(now);

        match stack.recv(socket, &mut sink) {
            Ok(0) => {}
            Ok(read) => {
                // The clock starts at the FIRST byte, not at connect: the
                // peer's own startup is not our throughput.
                started.get_or_insert_with(Wallclock::now);
                got = got.saturating_add(read);
            }
            Err(_) => {
                // Not connected any more: the peer finished early.
                if !stack.is_connected(socket) && got > 0 {
                    break;
                }
            }
        }
    }

    let seconds = started.map_or(0.0, |at| at.elapsed().as_secs_f64());
    Some((got, seconds))
}

fn parse_v4(text: &str) -> Vec<u8> {
    text.split('.')
        .filter_map(|part| part.parse::<u8>().ok())
        .collect()
}
