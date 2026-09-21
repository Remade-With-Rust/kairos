//! **The same K7 work, on two architectures, digested.**
//!
//! Every K7 package says it is `no_std` and builds for `thumbv7m-none-eabi`.
//! A build is not a run. This crate is the battery that turns "it compiles
//! for a chip" into "it computes the same answers on a chip", by being
//! compiled twice from one source:
//!
//! | arm | how |
//! |---|---|
//! | x86-64 | `cargo test` here, against the pins in `tests/pinned.rs` |
//! | ARMv7-M | `firmware/mps2-an385-k7`, booted in QEMU, against the same pins |
//!
//! # Why a digest and not a test suite
//!
//! The packages already have their own suites, and re-running them on the
//! chip would need a test harness that `no_std` does not have. What is in
//! question here is not whether the logic is right — the differentials
//! against C settled that — but whether it is right *at a different width
//! and a different endianness*. A digest answers exactly that, in a form a
//! semihosting `hprintln!` can carry.
//!
//! # The one thing that makes this a real differential
//!
//! [`Fnv::push_usize`] folds a `usize` as **eight big-endian bytes**, not as
//! its native bytes. Folding the native representation would make the two
//! digests differ on width alone — every run would "fail", the failure would
//! mean nothing, and the instrument would be measuring the pointer size
//! rather than the code. Normalising the width is what leaves a difference
//! meaning *the value came out different*, which is the only interesting
//! answer.
//!
//! So a mismatch here is a real finding: an offset, a length or a count that
//! is not the same on a 32-bit machine.
//!
//! # What it does NOT prove
//!
//! Nothing about timing, stack depth or interrupt behaviour, and nothing
//! about the parts of the packages that need `std` — the socket layer's
//! blocking loop and the smoltcp engine are not in here, because they
//! cannot be. It proves the *pure* surface agrees.

#![no_std]
#![forbid(unsafe_code)]

use rusty_rtos_backoff_core::{Backoff, RETRY_FOREVER};
use rusty_rtos_http_core::{request, response};
use rusty_rtos_json_core::search::Kind;
use rusty_rtos_json_core::{Validity, is_valid, search, validate};
use rusty_rtos_mqtt_core::header as mqtt_header;
use rusty_rtos_sntp_core::serializer as sntp_serializer;
use rusty_rtos_sntp_core::types::Timestamp;
use rusty_rtos_tcp_core::helpers::Family;
use rusty_rtos_tcp_core::{address, bitconfig, checksum, helpers, streambuffer};

/// FNV-1a, 64-bit. Chosen because it is four lines and has no endianness of
/// its own: every input is fed as bytes in a stated order, so the digest
/// cannot pick up the host byte order by accident the way a word-at-a-time
/// hash would.
#[derive(Debug, Clone, Copy)]
pub struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Self::new()
    }
}

impl Fnv {
    /// The FNV-1a offset basis.
    #[must_use]
    pub const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    /// The digest so far.
    #[must_use]
    pub const fn get(&self) -> u64 {
        self.0
    }

    /// Fold one byte.
    pub fn push_u8(&mut self, value: u8) {
        self.0 ^= u64::from(value);
        self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
    }

    /// Fold a slice, length first.
    pub fn push_bytes(&mut self, data: &[u8]) {
        // The LENGTH goes in too. Without it, one two-byte answer and two
        // one-byte answers digest the same, and a parser that returned the
        // right bytes split the wrong way would pass.
        self.push_u32(u32::try_from(data.len()).unwrap_or(u32::MAX));
        for &byte in data {
            self.push_u8(byte);
        }
    }

    /// Fold a `u16`, big-endian.
    pub fn push_u16(&mut self, value: u16) {
        for &byte in &value.to_be_bytes() {
            self.push_u8(byte);
        }
    }

    /// Fold a `u32`, big-endian.
    pub fn push_u32(&mut self, value: u32) {
        for &byte in &value.to_be_bytes() {
            self.push_u8(byte);
        }
    }

    /// Fold an `i32` by its two-complement bits.
    pub fn push_i32(&mut self, value: i32) {
        self.push_u32(value.cast_unsigned());
    }

    /// A `usize` as eight big-endian bytes — see the module docs. This
    /// normalisation is the reason the two arms are comparable at all.
    pub fn push_usize(&mut self, value: usize) {
        for &byte in &u64::try_from(value).unwrap_or(u64::MAX).to_be_bytes() {
            self.push_u8(byte);
        }
    }

    /// Fold a boolean.
    pub fn push_bool(&mut self, value: bool) {
        self.push_u8(u8::from(value));
    }
}

/// Make a value opaque to the optimiser.
///
/// Without this the whole battery is a pure function of literals, and LLVM is
/// entitled to fold it to a constant at compile time. It would fold it with
/// the TARGET semantics, so the answer would still be right -- and the chip
/// would be printing a number its compiler had computed rather than one it
/// had computed itself. `black_box` is what makes the Cortex-M3 do the work.
///
/// It is safe and it is in `core`, which matters: these packages are
/// `forbid(unsafe_code)` and so is this crate.
#[inline]
#[must_use]
fn opaque<T>(value: T) -> T {
    core::hint::black_box(value)
}

/// One digest per package, and one over all of them.
///
/// Per-package rather than a single number so that a mismatch NAMES the
/// package instead of merely announcing that something moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Digests {
    /// `rusty_rtos_tcp`.
    pub tcp: u64,
    /// `rusty_rtos_http`.
    pub http: u64,
    /// `rusty_rtos_mqtt`.
    pub mqtt: u64,
    /// `rusty_rtos_json`.
    pub json: u64,
    /// `rusty_rtos_sntp`.
    pub sntp: u64,
    /// `rusty_rtos_backoff`.
    pub backoff: u64,
    /// All six, folded in order.
    pub all: u64,
}

impl Digests {
    /// The digests as a table, for printing on either arm.
    #[must_use]
    pub const fn rows(&self) -> [(&'static str, u64); 7] {
        [
            ("tcp", self.tcp),
            ("http", self.http),
            ("mqtt", self.mqtt),
            ("json", self.json),
            ("sntp", self.sntp),
            ("backoff", self.backoff),
            ("ALL", self.all),
        ]
    }
}

/// The digests the battery produces, measured on x86-64 on 2026-09-21.
///
/// Both arms assert against THIS constant, not against each other. That is
/// what makes the chip run a gate rather than a report: the firmware carries
/// no numbers of its own, so it cannot quietly agree with itself.
///
/// If a K7 package changes behaviour these move, and they are meant to —
/// re-pin them from the host run, then re-run the chip. What must never
/// happen is the two arms disagreeing.
pub const PINNED: Digests = Digests {
    tcp: 0x69b9_3ded_7cdb_3812,
    http: 0xb3f6_5999_39db_a776,
    mqtt: 0x9344_4f0d_9da2_63cb,
    json: 0x3862_c7de_11af_d42b,
    sntp: 0x1747_5012_129a_67d7,
    backoff: 0x47a6_bfbd_a06c_3a57,
    all: 0xf679_7db4_2bd9_5cc0,
};

/// Run the whole battery.
#[must_use]
pub fn run() -> Digests {
    let tcp = tcp();
    let http = http();
    let mqtt = mqtt();
    let json = json();
    let sntp = sntp();
    let backoff = backoff();

    let mut all = Fnv::new();
    for value in [tcp, http, mqtt, json, sntp, backoff] {
        for &byte in &value.to_be_bytes() {
            all.push_u8(byte);
        }
    }

    Digests {
        tcp,
        http,
        mqtt,
        json,
        sntp,
        backoff,
        all: all.get(),
    }
}

/// A real IPv4 header, checksum field zeroed, as the checksum routine
/// expects to be handed one.
const IP_HEADER: [u8; 20] = [
    0x45, 0x00, 0x00, 0x3c, 0x1c, 0x46, 0x40, 0x00, 0x40, 0x06, 0x00, 0x00, 0xac, 0x10, 0x0a, 0x63,
    0xac, 0x10, 0x0a, 0x0c,
];

fn tcp() -> u64 {
    let mut d = Fnv::new();

    // ---- the checksum, at EVERY prefix length --------------------------
    // Including the odd ones. A one-complement sum reads the buffer as
    // 16-bit words and has to special-case a trailing odd byte, which is
    // precisely the place a big-endian assumption would hide.
    let header = opaque(IP_HEADER);
    let mut n = 0usize;
    while n <= header.len() {
        if let Some(prefix) = header.get(..n) {
            d.push_u16(checksum::generate_checksum(0, prefix));
            d.push_u16(checksum::generate_checksum(0x1234, prefix));
            d.push_u16(checksum::generate_checksum(0xffff, prefix));
        }
        n += 1;
    }

    // ---- IPv4 text, including the malformed forms the C accepts --------
    let v4: [&[u8]; 9] = [
        b"192.168.1.1",
        b"0.0.0.0",
        b"255.255.255.255",
        b"1.2.3",
        b"1.2.3.4.5",
        b"01.02.03.04",
        b"256.1.1.1",
        b"1.2.3.4 ",
        b"",
    ];
    for text in v4 {
        match address::parse_ipv4(opaque(text)) {
            Some(octets) => {
                d.push_u8(1);
                d.push_bytes(&octets);
            }
            None => d.push_u8(0),
        }
    }
    for octets in [
        [0u8, 0, 0, 0],
        [192, 168, 1, 1],
        [255, 255, 255, 255],
        [10, 0, 0, 1],
    ] {
        let mut out = [0u8; 32];
        match address::format_ipv4(opaque(octets), &mut out) {
            Some(text) => {
                d.push_u8(1);
                d.push_bytes(text.as_bytes());
            }
            None => d.push_u8(0),
        }
    }

    // ---- IPv6 ----------------------------------------------------------
    let v6: [&[u8]; 8] = [
        b"::",
        b"::1",
        b"2001:db8::1",
        b"fe80::1",
        b"2001:0db8:0000:0000:0000:0000:0000:0001",
        b"1:2:3:4:5:6:7:8",
        b":::1",
        b"2001:db8::1::2",
    ];
    for text in v6 {
        match address::parse_ipv6(opaque(text)) {
            Some(bytes) => {
                d.push_u8(1);
                d.push_bytes(&bytes);
            }
            None => d.push_u8(0),
        }
    }
    let mut loopback = [0u8; 16];
    if let Some(slot) = loopback.get_mut(15) {
        *slot = 1;
    }
    for value in [[0u8; 16], loopback] {
        let mut out = [0u8; 64];
        match address::format_ipv6(opaque(value), &mut out) {
            Some(text) => {
                d.push_u8(1);
                d.push_bytes(text.as_bytes());
            }
            None => d.push_u8(0),
        }
    }

    // ---- MAC -----------------------------------------------------------
    let macs: [&[u8]; 5] = [
        b"00:11:22:33:44:55",
        b"aa-bb-cc-dd-ee-ff",
        b"001122334455",
        b"00:11:22:33:44",
        b"gg:11:22:33:44:55",
    ];
    for text in macs {
        match helpers::parse_mac(opaque(text)) {
            Some(mac) => {
                d.push_u8(1);
                d.push_bytes(&mac);
            }
            None => d.push_u8(0),
        }
    }
    const COLON: u8 = 0x3a;
    const DASH: u8 = 0x2d;
    for upper in [false, true] {
        for separator in [COLON, DASH] {
            let mut out = [0u8; 32];
            let mac = [0x00, 0x11, 0x22, 0xaa, 0xbb, 0xcc];
            match helpers::format_mac(opaque(mac), opaque(upper), opaque(separator), &mut out) {
                Some(text) => {
                    d.push_u8(1);
                    d.push_bytes(text.as_bytes());
                }
                None => d.push_u8(0),
            }
        }
    }

    // ---- the family-dispatching wrappers -------------------------------
    for (family, text) in [
        (Family::Inet4, &b"10.0.0.1"[..]),
        (Family::Inet4, b"::1"),
        (Family::Inet6, b"::1"),
        (Family::Inet6, b"10.0.0.1"),
    ] {
        match helpers::parse_address(opaque(family), opaque(text)) {
            Some(helpers::Address::V4(octets)) => {
                d.push_u8(4);
                d.push_bytes(&octets);
            }
            Some(helpers::Address::V6(bytes)) => {
                d.push_u8(6);
                d.push_bytes(&bytes);
            }
            None => d.push_u8(0),
        }
    }

    // ---- the arithmetic helpers, at the edges --------------------------
    // `round_up(u32::MAX, 2)` is the overflow that rounds the largest value
    // to zero in the C. The Rust refuses; pinning the refusal here means the
    // chip has to refuse it too.
    for (a, b) in [
        (1i32, 2i32),
        (i32::MAX, 1),
        (i32::MIN, -1),
        (0, 0),
        (-1, i32::MIN),
    ] {
        d.push_i32(helpers::add_i32(opaque(a), opaque(b)));
        d.push_i32(helpers::mul_i32(opaque(a), opaque(b)));
    }
    for (a, divisor) in [
        (0u32, 1u32),
        (1, 1),
        (7, 4),
        (8, 4),
        (u32::MAX, 2),
        (u32::MAX, 1),
        (5, 0),
        (0, 0),
    ] {
        match helpers::round_up(opaque(a), opaque(divisor)) {
            Some(value) => {
                d.push_u8(1);
                d.push_u32(value);
            }
            None => d.push_u8(0),
        }
        match helpers::round_down(opaque(a), opaque(divisor)) {
            Some(value) => {
                d.push_u8(1);
                d.push_u32(value);
            }
            None => d.push_u8(0),
        }
    }

    // ---- BitConfig: the endianness of the wire, written and read -------
    {
        let mut store = [0u8; 32];
        let mut config = bitconfig::BitConfig::new(&mut store);
        config.write_8(opaque(0xa5));
        config.write_16(opaque(0x1234));
        config.write_32(opaque(0xdead_beef));
        config.write_uc(opaque(&b"kairos"[..]));
        d.push_usize(config.index());
        d.push_usize(config.size());
        d.push_bool(config.has_error());
        d.push_bytes(config.as_bytes());
    }
    {
        let mut store = [0u8; 24];
        let mut i = 0usize;
        while i < store.len() {
            if let Some(slot) = store.get_mut(i) {
                *slot = u8::try_from(i).unwrap_or(0).wrapping_mul(17);
            }
            i += 1;
        }
        let mut config = bitconfig::BitConfig::new(&mut store);
        d.push_u8(config.read_8());
        d.push_u16(config.read_16());
        d.push_u32(config.read_32());
        let mut into = [0u8; 6];
        d.push_bool(config.read_uc(Some(&mut into), opaque(6)));
        d.push_bytes(&into);
        let mut peeked = [0u8; 4];
        d.push_bool(config.peek_last(Some(&mut peeked), opaque(4)));
        d.push_bytes(&peeked);
        // Read past the end: the error must latch, and everything after must
        // fail. A chip that returned garbage instead would show up as a
        // different digest here.
        d.push_bool(config.read_uc(None, opaque(999)));
        d.push_bool(config.has_error());
        d.push_u8(config.read_8());
        d.push_bool(config.has_error());
    }

    // ---- StreamBuffer: offsets and wraps -------------------------------
    {
        let mut array = [0u8; 32];
        let mut buffer = streambuffer::StreamBuffer::new(&mut array);
        d.push_usize(buffer.get_space());
        d.push_usize(buffer.get_size());
        d.push_usize(buffer.front_space());
        d.push_usize(buffer.mid_space());

        d.push_usize(buffer.add(opaque(0), Some(opaque(&b"hello, kairos"[..])), opaque(13)));
        d.push_usize(buffer.get_size());
        d.push_usize(buffer.get_space());

        let mut into = [0u8; 16];
        d.push_usize(buffer.get(opaque(0), Some(&mut into), opaque(5), opaque(true)));
        d.push_bytes(&into);
        d.push_usize(buffer.get_size());

        d.push_usize(buffer.get(opaque(0), Some(&mut into), opaque(13), opaque(false)));
        d.push_bytes(&into);
        d.push_usize(buffer.get_size());

        let (start, run) = buffer.get_ptr();
        d.push_usize(start);
        d.push_usize(run);

        // Drive it past the end of the array so the wrap is exercised.
        d.push_usize(buffer.add(opaque(0), Some(opaque(&IP_HEADER[..])), opaque(IP_HEADER.len())));
        d.push_usize(buffer.add(opaque(0), Some(opaque(&IP_HEADER[..])), opaque(IP_HEADER.len())));
        d.push_usize(buffer.get_size());
        let (start, run) = buffer.get_ptr();
        d.push_usize(start);
        d.push_usize(run);
        d.push_usize(buffer.space(opaque(0), opaque(7)));
        d.push_usize(buffer.distance(opaque(7), opaque(0)));
        buffer.move_mid(opaque(3));
        d.push_usize(buffer.mid_space());
        d.push_bytes(buffer.as_bytes());
    }

    d.get()
}

/// A response with headers a reader has to walk, including one whose name is
/// a prefix of another — the classic place a matcher goes wrong.
const RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 42\r\nContent-Type: text/plain\r\nConnection: keep-alive\r\n\r\nbody";

fn http() -> u64 {
    let mut d = Fnv::new();

    {
        let mut buffer = [0u8; 512];
        let mut headers = request::RequestHeaders::new(&mut buffer);
        let info = opaque(request::RequestInfo {
            method: b"GET",
            path: b"/index.html",
            host: b"example.com",
            flags: 0,
        });
        d.push_u8(headers.initialize(&info) as u8);
        d.push_u8(headers.add(opaque(&b"X-Kairos"[..]), opaque(&b"1"[..])) as u8);
        d.push_u8(headers.add_range(opaque(0), opaque(1023)) as u8);
        d.push_bytes(headers.as_bytes());
    }
    {
        // An empty path means the C NULL, which defaults to a single slash.
        let mut buffer = [0u8; 256];
        let mut headers = request::RequestHeaders::new(&mut buffer);
        let info = opaque(request::RequestInfo {
            method: b"POST",
            path: b"",
            host: b"h",
            flags: 1,
        });
        d.push_u8(headers.initialize(&info) as u8);
        d.push_bytes(headers.as_bytes());
    }
    {
        // A buffer too small to hold the request line: the status is the
        // interesting part, and it must be the same on both arms.
        let mut buffer = [0u8; 8];
        let mut headers = request::RequestHeaders::new(&mut buffer);
        let info = opaque(request::RequestInfo {
            method: b"GET",
            path: b"/a/very/long/path",
            host: b"example.com",
            flags: 0,
        });
        d.push_u8(headers.initialize(&info) as u8);
        d.push_usize(headers.as_bytes().len());
    }
    {
        // The open-ended and invalid ranges, which the C spells with -1.
        for (start, end) in [(0i32, 1023i32), (100, -1), (-100, -1), (-1, 100), (0, 0)] {
            let mut buffer = [0u8; 256];
            let mut headers = request::RequestHeaders::new(&mut buffer);
            let info = opaque(request::RequestInfo {
                method: b"GET",
                path: b"/",
                host: b"h",
                flags: 0,
            });
            let _ = headers.initialize(&info);
            d.push_u8(headers.add_range(opaque(start), opaque(end)) as u8);
            d.push_bytes(headers.as_bytes());
        }
    }

    for field in [
        &b"Content-Length"[..],
        b"Content-Type",
        b"Content",
        b"content-length",
        b"Connection",
        b"Missing",
        b"",
    ] {
        let (status, value) = response::read_header(opaque(RESPONSE), opaque(field));
        d.push_u8(status as u8);
        match value {
            Some(found) => {
                d.push_u8(1);
                d.push_usize(found.offset);
                d.push_usize(found.len);
                match RESPONSE.get(found.offset..found.offset.saturating_add(found.len)) {
                    Some(text) => d.push_bytes(text),
                    None => d.push_u8(0xff),
                }
            }
            None => d.push_u8(0),
        }
    }

    d.get()
}

fn mqtt() -> u64 {
    let mut d = Fnv::new();

    // The variable-length integer, at every continuation boundary. This is
    // the format own encoding and is exactly the kind of thing that could
    // differ on another machine if it were written with a word cast instead
    // of byte by byte.
    let mut out = [0u8; 8];
    for length in [
        0u32,
        1,
        127,
        128,
        16_383,
        16_384,
        2_097_151,
        2_097_152,
        268_435_455,
    ] {
        let used = mqtt_header::encode_variable_length(&mut out, opaque(length));
        d.push_usize(used);
        match out.get(..used) {
            Some(bytes) => d.push_bytes(bytes),
            None => d.push_u8(0xff),
        }
    }

    let frames: [&[u8]; 8] = [
        &[0x20, 0x02, 0x00, 0x00],
        &[0xd0, 0x00],
        &[0x30, 0x0a, 0x00, 0x04],
        &[0x90, 0x03, 0x00, 0x01, 0x00],
        &[0x40, 0x02, 0x00, 0x01],
        &[0x00, 0x00],
        &[0x20],
        &[0x30, 0xff, 0xff, 0xff, 0x7f],
    ];
    for frame in frames {
        match mqtt_header::process_incoming_packet_type_and_length(opaque(frame), opaque(frame.len())) {
            Ok(header) => {
                d.push_u8(1);
                d.push_u8(header.packet_type);
                d.push_u32(header.remaining_length);
                d.push_usize(header.header_length);
            }
            Err(_) => d.push_u8(0),
        }
        // And with `available` understated, which is the partial-read case.
        match mqtt_header::process_incoming_packet_type_and_length(opaque(frame), opaque(1)) {
            Ok(header) => {
                d.push_u8(1);
                d.push_u32(header.remaining_length);
            }
            Err(_) => d.push_u8(0),
        }
    }

    d.get()
}

/// `{"a":"é"}`, spelled as bytes.
///
/// A JSON `\u` escape cannot be written as a Rust source escape without one
/// tool or another along the way deciding to interpret it, so the two
/// documents that carry one are spelled out. 0x5c is the backslash and 0x75
/// is the `u`.
const JSON_UNICODE_ESCAPE: [u8; 14] = [
    0x7b, 0x22, 0x61, 0x22, 0x3a, 0x22, 0x5c, 0x75, 0x30, 0x30, 0x65, 0x39, 0x22, 0x7d,
];

/// `{"a":"😀"}`, spelled as bytes: a surrogate PAIR, which the
/// validator has to join rather than accept as two lone halves.
const JSON_SURROGATE_PAIR: [u8; 20] = [
    0x7b, 0x22, 0x61, 0x22, 0x3a, 0x22, 0x5c, 0x75, 0x64, 0x38, 0x33, 0x64, 0x5c, 0x75, 0x64, 0x65,
    0x30, 0x30, 0x22, 0x7d,
];

fn json() -> u64 {
    let mut d = Fnv::new();

    let documents: [&[u8]; 12] = [
        br#"{"a":1}"#,
        br#"{"a":[1,2,3],"b":{"c":"d"}}"#,
        &JSON_UNICODE_ESCAPE[..],
        &JSON_SURROGATE_PAIR[..],
        br#"[1,2,"#,
        br#"{"a":01}"#,
        br#"{"a":1.0e+3}"#,
        br#"{"a":-0.5E-2}"#,
        b"",
        b"   ",
        br#"{"nested":{"deep":{"deeper":{"deepest":true}}}}"#,
        br#"[[[[[[[[[[1]]]]]]]]]]"#,
    ];
    for document in documents {
        d.push_u8(match validate(opaque(document)) {
            Validity::Valid => 0,
            Validity::Illegal => 1,
            Validity::MaxDepthExceeded => 2,
            Validity::Partial => 3,
            Validity::BadParameter => 4,
        });
        d.push_bool(is_valid(opaque(document)));
    }

    let document = br#"{"a":{"b":[10,20,{"c":"hi"}]},"d":"hello","e":null}"#;
    let queries: [&[u8]; 9] = [
        b"a",
        b"a.b",
        b"a.b[0]",
        b"a.b[1]",
        b"a.b[2].c",
        b"d",
        b"e",
        b"a.b[9]",
        b"missing",
    ];
    for query in queries {
        match search(opaque(document), opaque(query)) {
            Ok(found) => {
                d.push_u8(1);
                d.push_usize(found.offset);
                d.push_bytes(found.value);
                d.push_bool(matches!(found.kind, Kind::String));
            }
            Err(_) => d.push_u8(0),
        }
    }

    d.get()
}

fn sntp() -> u64 {
    let mut d = Fnv::new();

    {
        let mut buffer = [0u8; 48];
        let mut request_time = opaque(Timestamp::new(0xe7c4_0000, 0x1234_5678));
        match sntp_serializer::serialize_request(&mut request_time, opaque(0xdead_beef), &mut buffer) {
            Ok(()) => {
                d.push_u8(1);
                d.push_bytes(&buffer);
                // The call REWRITES the timestamp fractions with the
                // randomised low bits, so the value afterwards is part of
                // the answer, not just the buffer.
                d.push_u32(request_time.seconds);
                d.push_u32(request_time.fractions);
            }
            Err(_) => d.push_u8(0),
        }
    }
    {
        let mut small = [0u8; 8];
        let mut time = Timestamp::new(1, 2);
        d.push_bool(sntp_serializer::serialize_request(&mut time, opaque(7), &mut small).is_ok());
        let mut zero = Timestamp::new(0, 0);
        let mut buffer = [0u8; 48];
        d.push_bool(sntp_serializer::serialize_request(&mut zero, opaque(7), &mut buffer).is_ok());
    }
    {
        // An all-zero "response" must be refused, not believed.
        let response = [0u8; 48];
        let request_time = Timestamp::new(0xe7c4_0000, 0);
        let received = Timestamp::new(0xe7c4_0001, 0);
        d.push_bool(
            sntp_serializer::deserialize_response(&request_time, &received, opaque(&response[..])).is_ok(),
        );
    }
    for (tolerance, accuracy) in [
        (500u16, 100u16),
        (1, 1),
        (0, 1),
        (1, 0),
        (u16::MAX, u16::MAX),
        (1, u16::MAX),
    ] {
        match sntp_serializer::calculate_poll_interval(opaque(tolerance), opaque(accuracy)) {
            Ok(interval) => {
                d.push_u8(1);
                d.push_u32(interval);
            }
            Err(_) => d.push_u8(0),
        }
    }

    d.get()
}

fn backoff() -> u64 {
    let mut d = Fnv::new();

    // A bounded backoff, driven past its own limit so the exhaustion is
    // pinned as well as the delays.
    {
        let mut policy = opaque(Backoff::new(500, 5000, 10));
        let mut random = 0x1234_5678u32;
        let mut attempt = 0;
        while attempt < 14 {
            random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            match policy.next_backoff(opaque(random)) {
                Ok(delay) => {
                    d.push_u8(1);
                    d.push_u16(delay);
                }
                Err(_) => d.push_u8(0),
            }
            d.push_u32(policy.attempts_done());
            d.push_u16(policy.next_jitter_max());
            d.push_bool(policy.exhausted());
            attempt += 1;
        }
    }
    // And an unbounded one, where the jitter ceiling has to saturate rather
    // than wrap.
    {
        let mut policy = opaque(Backoff::new(1000, 60_000, RETRY_FOREVER));
        d.push_u16(policy.max_backoff_delay());
        d.push_u32(policy.max_retry_attempts());
        let mut attempt = 0;
        while attempt < 20 {
            match policy.next_backoff(opaque(0xffff_ffff)) {
                Ok(delay) => {
                    d.push_u8(1);
                    d.push_u16(delay);
                }
                Err(_) => d.push_u8(0),
            }
            d.push_u16(policy.next_jitter_max());
            attempt += 1;
        }
    }

    d.get()
}
