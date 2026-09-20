#!/bin/sh
# Price ONE change to rusty_rtos_kernel-core on every instrument that sees it.
#
#     wsl -e sh bench/ksweep.sh <variant>          # from the umbrella root
#
# where <variant> names `bench/variants/<variant>_kernel.rs` holding the
# candidate `kernel.rs`. Measures committed HEAD against it, then restores the
# tree. The sibling `sweep.sh` does the same for `rusty_rtos_core`'s list.
#
# ---- the arms ------------------------------------------------------------
#
#   kdelay-ir        tasks actually BLOCK, so the delayed list has depth and
#                    `wake_due_tasks` runs. Nothing measured this until
#                    2026-09-19; every other kernel instrument passes a zero
#                    timeout and never parks anything.
#   kdelay-deep      the same at depth 17 instead of 4. A change on the
#                    delayed list can only be priced against a stated depth.
#   ksched-ir        suspend/resume/yield/tick: ready-list traffic, which is
#                    most of what a scheduler does.
#   khot-ir          the queues, where the IPC machinery dominates.
#
# ---- and the gate --------------------------------------------------------
#
# A kernel change is not admissible because it is fast. `kairos conform`
# compares every scheduling decision, in order, at the same tick, with the
# same tick/yield/critical-exit counters, against the instrumented C kernel.
# 19 scenarios. THAT is the gate; these numbers are only the reason to want
# the change.
#
# The list work needed this lesson twice. A sorted-insert fast path measured
# -18%, passed all 81 unit tests and the differential, and was WRONG -- it
# assumed an invariant `vListInsertEnd` does not keep. A wrong order is still
# a consistent order. Run `conform` before believing any of this.
set -eu

here=$(cd "$(dirname "$0")/.." && pwd)
V=${1:-}
[ -n "$V" ] || { echo "usage: sh bench/ksweep.sh <variant>" >&2; exit 1; }

VAR="$here/bench/variants/${V}_kernel.rs"
KERN="$here/rusty_rtos_kernel/crates/rusty_rtos_kernel-core/src/kernel.rs"
[ -f "$VAR" ] || { echo "MISSING VARIANT $VAR -- refusing to profile" >&2; exit 1; }

# A variant identical to HEAD is a null arm wearing a candidate's name: it
# prints two columns that agree to the noise floor, which reads as "measured,
# flat, refuted". A wrong refutation is permanent.
if cmp -s "$VAR" "$KERN"; then
    echo "VARIANT $V IS IDENTICAL TO HEAD -- that is a null arm, not a change" >&2
    exit 1
fi

restore() {
    (cd "$here/rusty_rtos_kernel" && git checkout -- crates/rusty_rtos_kernel-core/src/kernel.rs)
}
trap restore EXIT

count() { # <bench-dir> <bin> <label> [build flags...]
    d=$1; b=$2; label=$3
    shift 3
    cd "$here/rusty_rtos_kernel/bench/$d"
    rm -f "target/release/$b"
    cargo build --release "$@" >/dev/null 2>&1 || { echo "$arm $label BUILD FAIL"; return 0; }
    [ -f "target/release/$b" ] || { echo "$arm $label NO BINARY"; return 0; }
    valgrind --tool=callgrind --cache-sim=no --branch-sim=no \
             --callgrind-out-file=/tmp/kw.out "./target/release/$b" >/tmp/kw.txt 2>/dev/null
    printf '%-8s %-12s %12s   %s\n' "$arm" "$label" \
        "$(grep -m1 '^summary:' /tmp/kw.out | awk '{print $2}')" "$(head -1 /tmp/kw.txt)"
}

for arm in head "$V"; do
    if [ "$arm" = head ]; then
        restore
    else
        cp "$VAR" "$KERN" || { echo "COPY FAILED for $arm" >&2; exit 1; }
    fi
    count kdelay-ir kdelay-ir kdelay-ir
    count kdelay-ir kdelay-ir kdelay-deep --features deep
    count ksched-ir ksched-ir ksched-ir
    count khot-ir   khot-ir   khot-ir
done

echo
echo "Checksums must be identical down each column. Then run the gate:"
echo "  wsl -e sh -c 'cd tools/kairos && cargo run --release -q -- conform --all --ticks 20000'"
