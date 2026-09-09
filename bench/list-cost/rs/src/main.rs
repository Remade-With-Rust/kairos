//! The Rust arm of the K1 arena-list cost row.
//!
//! It runs `rusty_rtos_core`'s index-linked remake of `list.c` through the
//! same four operations, in the same order, on the same values as the C arm
//! (`../../c/list_bench.c`), and prints the same checksum. The checksum is
//! the correctness gate: two instruction counts of two different programs
//! would not be a comparison.
//!
//! Nothing here is timed. See the C arm's header for how the counts are
//! taken and why the run happens twice.

use rusty_rtos_core::list::Lists;

/// How many items are in a list at once: a plausible ready-list depth.
const ITEMS: u16 = 8;
/// Two lists: a ready list and a delayed list.
const LISTS: usize = 2;
const READY: u8 = 0;
const DELAYED: u8 = 1;

/// A 32-bit xorshift, spelled as the C arm spells it so both lists sort the
/// same sequence of values.
fn next(state: &mut u32) -> u32 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    x
}

fn mix(sum: u64, value: u64) -> u64 {
    (sum ^ value).wrapping_mul(0x100_0000_01b3)
}

fn main() {
    let rounds: u64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(100_000);

    let mut lists: Lists<{ ITEMS as usize }, LISTS> = Lists::new();
    let mut sum: u64 = 0xcbf2_9ce4_8422_2325;
    let mut state: u32 = 0x1234_5678;

    for _ in 0..rounds {
        // prvAddTaskToReadyList: append to the end of a ready list.
        for item in 0..ITEMS {
            lists.insert_end(READY, item).expect("a ready-list append");
        }
        // taskSELECT_HIGHEST_PRIORITY_TASK: walk the round robin.
        for _ in 0..ITEMS {
            let owner = lists
                .next_round_robin(READY)
                .expect("the list exists")
                .expect("the list is not empty");
            sum = mix(sum, u64::from(owner));
        }
        // Leaving the ready list.
        for item in 0..ITEMS {
            sum = mix(sum, lists.remove(item).expect("the item is in a list") as u64);
        }
        // prvAddCurrentTaskToDelayedList: an ordered insert by wake time.
        for item in 0..ITEMS {
            let value = u64::from(next(&mut state));
            lists
                .insert(DELAYED, item, value)
                .expect("a delayed-list insert");
        }
        // And out again, as the tick unblocks them.
        for item in 0..ITEMS {
            sum = mix(sum, lists.remove(item).expect("the item is in a list") as u64);
        }
    }

    println!("rounds={rounds} items={ITEMS} checksum={sum:016x}");
}
