//! **The K7 packages computing on a Cortex-M3.**
//!
//! Six packages — TCP, HTTP, MQTT, JSON, SNTP and backoff — say they are
//! `no_std` and build for `thumbv7m-none-eabi`. Every one of them was proved
//! correct by a differential against C, on x86-64. This cell asks the
//! question those differentials could not: **are the answers the same at
//! half the pointer width?**
//!
//! # How it is a gate and not a report
//!
//! The cell holds **no expected values**. It runs `k7_battery::run()` and
//! compares against `k7_battery::PINNED`, which is the table the host test
//! asserts against too. So there is exactly one set of numbers, measured on
//! x86-64, and the chip either reproduces it or does not. A cell carrying
//! its own pins could be re-pinned from a chip run and would then agree with
//! itself forever, which is the failure mode this avoids.
//!
//! # What a failure would mean
//!
//! Not "the chip is broken". The battery normalises `usize` to eight
//! big-endian bytes precisely so that a width difference is not, by itself,
//! a difference. A mismatch means a length, an offset or a count came out
//! with a different VALUE on a 32-bit machine — a real defect, and one that
//! no amount of host testing would have found.
//!
//! # What it does not cover
//!
//! The `std`-only surface: the socket layer with its blocking loop, and the
//! smoltcp engine. Those are proved by the interop rigs against foreign
//! stacks instead, on a workstation. And nothing here is a timing or
//! stack-depth measurement.

#![no_std]
#![no_main]

use cortex_m_rt::entry;
use cortex_m_semihosting::{debug, hprintln};
use panic_semihosting as _;

use k7_battery::{PINNED, run};

#[entry]
fn main() -> ! {
    hprintln!();
    hprintln!("=== the K7 packages computing on a Cortex-M3 (mps2-an385, QEMU) ===");
    hprintln!("one battery, two architectures. The numbers below were measured");
    hprintln!("on x86-64; this machine has 32-bit pointers and has to agree.");
    hprintln!();

    let measured = run();

    let mut failed = 0u32;
    for ((name, got), (_, want)) in measured.rows().into_iter().zip(PINNED.rows()) {
        if got == want {
            hprintln!("      ok    {:<8} 0x{:016x}", name, got);
        } else {
            failed = failed.saturating_add(1);
            hprintln!(
                "      FAIL  {:<8} 0x{:016x}   host pinned 0x{:016x}",
                name,
                got,
                want
            );
        }
    }

    hprintln!();
    if failed == 0 {
        hprintln!("RESULT: PASS -- all six K7 packages compute byte-for-byte the");
        hprintln!("        same answers on ARMv7-M as on x86-64.");
        debug::exit(debug::EXIT_SUCCESS);
    } else {
        hprintln!("RESULT: FAIL -- {} package(s) differ on this architecture.", failed);
        hprintln!("        This is a real defect: the battery normalises pointer");
        hprintln!("        width, so a difference here is a difference in VALUE.");
        debug::exit(debug::EXIT_FAILURE);
    }

    loop {
        core::hint::spin_loop();
    }
}
