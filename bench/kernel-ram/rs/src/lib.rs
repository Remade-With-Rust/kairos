#![no_std]
//! What a Kairos kernel occupies in RAM, on the target, before any task
//! exists — and how that total moves when each arena grows.
//!
//! # Why a `static` and not a `println!`
//!
//! The number wanted here is `size_of` on **rv32**, and `size_of` on the
//! host is a different number: a host `usize` is eight bytes where the
//! target's is four. Printing it from a host test would be measuring the
//! wrong machine.
//!
//! So each probe is a zero-filled array whose LENGTH is the size being
//! measured. The linker then records that length as the symbol's size, and
//! `llvm-nm -S` reads it straight back out of the object built for the real
//! target. No value decoding, no emulator, and nothing to run.
//!
//! # Why several of them
//!
//! One total is not a decomposition. FreeRTOS takes its TCBs, queues and
//! timers from the heap on demand, so its static cost does not move when a
//! task is added; Kairos declares arenas, so its static cost is the whole
//! budget and moves with every dimension. Those are different floor
//! FUNCTIONS, and a function needs more than one point. Each pair below
//! differs in exactly one dimension, so the difference between them is that
//! dimension's per-unit cost — a slope, not a guess.
//!
//! Note that `ITEMS` and `LISTS` are derived from the other dimensions by
//! `items_for`/`lists_for`, exactly as a real configuration derives them.
//! A task costs its list items too, and charging it for them is the point.

use core::mem::size_of;

use rusty_rtos_core::config::PosixDemoConfig;
use rusty_rtos_core::hooks::NoTickHook;
use rusty_rtos_core::trace::NoTrace;
use rusty_rtos_kernel_core::kernel::Kernel;
use rusty_rtos_kernel_core::{items_for, lists_for};
use rusty_rtos_port_core::sim::SimPort;

/// `PosixDemoConfig::MAX_PRIORITIES`, which sizes the ready lists. Written
/// out rather than read through the trait so it reads beside the C arm's
/// `configMAX_PRIORITIES 5`, which is the same number for the same reason.
const PRIOS: u8 = 5;

/// One geometry's size, in bytes, on the target.
///
/// A macro rather than a generic type alias because stable Rust will not let
/// a generic const parameter feed a const expression: `items_for(TASKS, ..)`
/// is rejected where `items_for(8, 16)` is fine. Substituting literals first
/// sidesteps that, and costs only that each geometry is written out.
///
/// The trace and the tick hook are the no-op ones, so what is measured is
/// the kernel rather than a demo's instrumentation.
macro_rules! ksize {
    ($tasks:literal, $queues:literal, $slots:literal,
     $buffers:literal, $bytes:literal, $timers:literal, $groups:literal) => {
        size_of::<
            Kernel<
                PosixDemoConfig,
                SimPort,
                NoTrace,
                NoTickHook,
                $tasks,
                { items_for($tasks, $timers) },
                { lists_for(PRIOS, $queues, $groups) },
                $queues,
                $slots,
                $buffers,
                $bytes,
                $timers,
                $groups,
            >,
        >()
    };
}

// The baseline geometry, and one neighbour per dimension. Each differs from
// BASE in exactly one place, so each difference is that dimension's slope.
const BASE: usize = ksize!(8, 8, 64, 4, 1024, 16, 2);
const TASKS_16: usize = ksize!(16, 8, 64, 4, 1024, 16, 2);
const QUEUES_16: usize = ksize!(8, 16, 64, 4, 1024, 16, 2);
const SLOTS_128: usize = ksize!(8, 8, 128, 4, 1024, 16, 2);
const BUFFERS_8: usize = ksize!(8, 8, 64, 8, 1024, 16, 2);
const BYTES_2048: usize = ksize!(8, 8, 64, 4, 2048, 16, 2);
const TIMERS_32: usize = ksize!(8, 8, 64, 4, 1024, 32, 2);
const GROUPS_4: usize = ksize!(8, 8, 64, 4, 1024, 16, 4);

/// The geometry the conformance corpus actually runs, on every architecture
/// it runs on. Read off the mangled names in the built rv32 firmware:
/// TASKS 24, QUEUES 12, SLOTS 128, BUFFERS 8, BYTES 2048, TIMERS 32,
/// GROUPS 4.
const CORPUS: usize = ksize!(24, 12, 128, 8, 2048, 32, 4);

macro_rules! probe {
    ($sym:ident, $size:expr) => {
        #[no_mangle]
        pub static $sym: [u8; $size] = [0; $size];
    };
}

probe!(KAIROS_RAM_BASE, BASE);
probe!(KAIROS_RAM_TASKS_16, TASKS_16);
probe!(KAIROS_RAM_QUEUES_16, QUEUES_16);
probe!(KAIROS_RAM_SLOTS_128, SLOTS_128);
probe!(KAIROS_RAM_BUFFERS_8, BUFFERS_8);
probe!(KAIROS_RAM_BYTES_2048, BYTES_2048);
probe!(KAIROS_RAM_TIMERS_32, TIMERS_32);
probe!(KAIROS_RAM_GROUPS_4, GROUPS_4);
probe!(KAIROS_RAM_CORPUS, CORPUS);

/// A staticlib needs one even though nothing here can panic.
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
