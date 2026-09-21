//! The host arm of the cross-architecture differential.
//!
//! This asserts the battery against [`k7_battery::PINNED`]. The chip arm,
//! `firmware/mps2-an385-k7`, asserts against the same constant — so the two
//! arms are compared to one table rather than to each other, and neither can
//! drift on its own.
//!
//! A failure here means a K7 package changed behaviour. That is allowed;
//! re-pin from this run and re-run the chip. A failure on the CHIP with this
//! one passing is the interesting case: it means the answer depends on the
//! width or the byte order.

use k7_battery::{PINNED, run};

#[test]
fn the_host_matches_the_pins() {
    let measured = run();
    for ((name, got), (_, want)) in measured.rows().into_iter().zip(PINNED.rows()) {
        assert_eq!(got, want, "{name}: 0x{got:016x} != pinned 0x{want:016x}");
    }
    assert_eq!(measured, PINNED);
}

/// The battery must be a pure function of nothing. If it were not — if any
/// part of it read a clock, an address or uninitialised memory — the digests
/// could not be pinned at all, and the chip comparison would be meaningless.
#[test]
fn the_battery_is_deterministic() {
    let first = run();
    for _ in 0..16 {
        assert_eq!(run(), first, "the battery is not a pure function");
    }
}

/// Every package must contribute. A section that silently did nothing would
/// still produce a stable digest — the FNV basis — and would sail through
/// the comparison above while testing nothing at all.
#[test]
fn no_section_is_empty() {
    const BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    let measured = run();
    for (name, value) in measured.rows() {
        assert_ne!(
            value, BASIS,
            "{name} digested nothing: it is still the FNV offset basis"
        );
    }
}

/// And the sections must differ from one another. Two identical digests
/// would mean a copy-paste that ran the same battery twice under two names.
#[test]
fn the_sections_are_distinct() {
    let rows = run().rows();
    for (i, (left_name, left)) in rows.iter().enumerate() {
        for (right_name, right) in rows.iter().skip(i + 1) {
            assert_ne!(
                left, right,
                "{left_name} and {right_name} digest the same, so one is a copy of the other"
            );
        }
    }
}
