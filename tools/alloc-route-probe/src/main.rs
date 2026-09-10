//! Which constant routes the 16% step in `rusty_alloc`'s small path?
//!
//! On an ESP32-S3 (`xtensa`, 32-bit) the cost of one `alloc` + one `free`
//! steps by 43 cycles — 314 to 271 cycles/op — between a **512-byte**
//! request and a **513-byte** one, and by 662 cycles between 2,048 and
//! 2,049. Measured on silicon with `CCOUNT`, byte-identical totals within
//! each range, nothing stepping anywhere else. So the size selects a route
//! and the route sets the cost.
//!
//! The upper step is unambiguous: 2,048 is `MEDIUM_OBJ_SIZE_MAX`, above
//! which every allocation gets its own large span.
//!
//! **The lower one is ambiguous on 32-bit, and that is what this exists to
//! resolve.** Two different constants both equal 512 there:
//!
//! * `SMALL_SIZE_MAX` = `SMALL_WSIZE_MAX * INTPTR_SIZE` = 128 x 4 — the top
//!   of the `direct[]` lookup table, and
//! * `SMALL_OBJ_SIZE_MAX` = `SEGMENT_SLICE_SIZE / 8` = 4096 / 8 — the top of
//!   the small-page range.
//!
//! On a **64-bit host they come apart**: `INTPTR_SIZE` is 8, so
//! `SMALL_SIZE_MAX` is 1,024, while `ra_small_profile` keeps the slice at
//! 4 KiB and `SMALL_OBJ_SIZE_MAX` at 512. Running the same sweep here puts
//! the step at exactly one of them, and that names the router.
//!
//! It matters because it decides whether the fix is free. If the page kind
//! routes it, moving the boundary trades speed for page granularity — a
//! medium page is four slices, so a class costs 16 KiB of region instead of
//! 4 KiB, which on a 64 KiB microcontroller region is a bad trade. If the
//! `direct[]` table routes it, then the *fast* lookup is losing to the
//! fallback and the fix costs no memory at all.
//!
//! The probe prints the constants it actually compiled against rather than
//! restating them, so a reader can check the premise before the result.
//!
//! ```sh
//! cargo run --release
//! ```

use std::alloc::{GlobalAlloc, Layout};
use std::hint::black_box;

use rusty_alloc::types::{
    INTPTR_SIZE, MEDIUM_OBJ_SIZE_MAX, SEGMENT_SLICE_SIZE, SMALL_OBJ_SIZE_MAX, SMALL_SIZE_MAX,
    SMALL_WSIZE_MAX,
};

#[global_allocator]
static A: rusty_alloc_api::RustyAlloc = rusty_alloc_api::RustyAlloc;

/// Operations per measured round.
const OPS: usize = 256;
/// Rounds; the answer is the best of them, because the floor is what
/// survives the scheduler landing mid-round. A mean would measure whatever
/// else the box was doing.
const ROUNDS: usize = 400;
/// Rounds run before any is counted.
const WARMUP: usize = 64;
/// Both arms request this, matching the firmware's `portBYTE_ALIGNMENT`.
const ALIGN: usize = 8;

#[inline(always)]
fn cycles() -> u64 {
    // SAFETY: `rdtsc` is a read of a counter register; it cannot fault on
    // any x86-64 that runs this binary.
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::x86_64::_rdtsc()
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64)
    }
}

/// One round of `OPS` alloc/free pairs at `size`, and a checksum so a
/// compiler that removed the work would be visible as a changed number
/// rather than as a suspiciously good one.
#[inline(never)]
fn round(size: usize) -> (u64, u64) {
    let layout = Layout::from_size_align(size, ALIGN).expect("valid layout");
    let mut sum = 0u64;
    let start = cycles();
    for i in 0..OPS {
        // SAFETY: `layout` has non-zero size; `p` is checked before use and
        // freed with the same layout it was allocated with.
        unsafe {
            let p = A.alloc(layout);
            if p.is_null() {
                continue;
            }
            p.write((i & 0xff) as u8);
            sum = sum.wrapping_add(u64::from(p.read()));
            black_box(p);
            A.dealloc(p, layout);
        }
    }
    (cycles().wrapping_sub(start), sum)
}

/// The null arm: the same loop with the allocation removed.
#[inline(never)]
fn round_empty() -> (u64, u64) {
    let mut sum = 0u64;
    let start = cycles();
    for i in 0..OPS {
        let x = black_box((i & 0xff) as u8);
        sum = sum.wrapping_add(u64::from(x));
    }
    (cycles().wrapping_sub(start), sum)
}

fn best<F: FnMut() -> (u64, u64)>(mut f: F) -> (u64, u64) {
    for _ in 0..WARMUP {
        black_box(f());
    }
    let mut lo = u64::MAX;
    let mut sum = 0;
    for _ in 0..ROUNDS {
        let (c, s) = f();
        sum = s;
        lo = lo.min(c);
    }
    (lo, sum)
}

fn main() {
    println!();
    println!("=== rusty_alloc: which constant routes the small-path step? ===");
    println!();
    println!("the constants THIS build compiled against:");
    println!("  INTPTR_SIZE          {INTPTR_SIZE}");
    println!("  SMALL_WSIZE_MAX      {SMALL_WSIZE_MAX}");
    println!("  SMALL_SIZE_MAX       {SMALL_SIZE_MAX:>6}   <- top of the direct[] table");
    println!("  SEGMENT_SLICE_SIZE   {SEGMENT_SLICE_SIZE}");
    println!("  SMALL_OBJ_SIZE_MAX   {SMALL_OBJ_SIZE_MAX:>6}   <- top of the small-PAGE range");
    println!("  MEDIUM_OBJ_SIZE_MAX  {MEDIUM_OBJ_SIZE_MAX:>6}");
    println!();

    if SMALL_SIZE_MAX == SMALL_OBJ_SIZE_MAX {
        println!("  !! the two candidates are EQUAL in this build, so it cannot");
        println!("     separate them. That is the 32-bit case; run on 64-bit.");
        println!();
    }

    let (floor, _) = best(round_empty);
    println!(
        "null arm  {floor} cycles for {OPS} empty ops = {} cycles/op, subtracted",
        floor / OPS as u64
    );
    println!("method    best of {ROUNDS} rounds of {OPS} ops, {WARMUP} warm-up");
    println!();

    // Sizes chosen to bracket BOTH candidates by one byte, plus the medium
    // ceiling. A step at 512/513 acquits `direct[]`; a step at 1024/1025
    // acquits the page kind.
    println!("     size   cycles/op   note");
    let mut rows: Vec<(usize, u64)> = Vec::new();
    for size in [
        64usize, 128, 256, 384, 448, 511, 512, 513, 576, 640, 768, 896, 1023, 1024, 1025, 1152,
        1280, 1536, 2047, 2048, 2049, 2560,
    ] {
        let (c, sum) = best(|| round(size));
        let per = c.saturating_sub(floor) / OPS as u64;
        let note = if size == SMALL_SIZE_MAX {
            "== SMALL_SIZE_MAX (direct[] top)"
        } else if size == SMALL_OBJ_SIZE_MAX {
            "== SMALL_OBJ_SIZE_MAX (small-page top)"
        } else if size == MEDIUM_OBJ_SIZE_MAX {
            "== MEDIUM_OBJ_SIZE_MAX"
        } else if sum == 0 {
            "FAIL: checksum zero, work optimised away"
        } else {
            ""
        };
        println!("  {size:>7}   {per:>9}   {note}");
        rows.push((size, per));
    }

    // Name the verdict from the data rather than leaving it to the reader.
    println!();
    let step_at = |lo: usize, hi: usize| -> Option<i64> {
        let a = rows.iter().find(|r| r.0 == lo)?.1 as i64;
        let b = rows.iter().find(|r| r.0 == hi)?.1 as i64;
        Some(b - a)
    };
    let small_step = step_at(512, 513).unwrap_or(0);
    let direct_step = step_at(1024, 1025).unwrap_or(0);
    println!("step 512 -> 513   {small_step:>6} cycles/op  (SMALL_OBJ_SIZE_MAX boundary)");
    println!("step 1024 -> 1025 {direct_step:>6} cycles/op  (SMALL_SIZE_MAX boundary)");
    println!();
    // A verdict needs the step to be big enough to be a step. 8 cycles/op
    // is roughly 3% here and comfortably above the run-to-run spread of a
    // best-of-400 on a pinned box; anything smaller is called "no step"
    // rather than being read as a small one.
    const MIN: i64 = 8;
    match (small_step.abs() >= MIN, direct_step.abs() >= MIN) {
        (true, false) => println!("VERDICT: the PAGE KIND routes it (SMALL_OBJ_SIZE_MAX)."),
        (false, true) => println!("VERDICT: the direct[] TABLE routes it (SMALL_SIZE_MAX)."),
        (true, true) => println!("VERDICT: BOTH boundaries step. Two effects, not one."),
        (false, false) => println!("VERDICT: neither steps here — the host does not reproduce it."),
    }
}
