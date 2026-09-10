#!/bin/sh
# Per-function instruction counts for the kernel, from callgrind.
#
#     wsl -e sh -c 'sh bench/kernel-ir/run.sh baseline'   # record
#     wsl -e sh -c 'sh bench/kernel-ir/run.sh after'      # record again
#     wsl -e sh -c 'sh bench/kernel-ir/run.sh diff'       # what moved
#
# METHOD LINE, which every number taken here must be quoted with:
#
#   Instrument: callgrind Ir (instructions retired), per function, on the
#   demo's own scenarios. NOT a clock. Deterministic to the instruction, so
#   there is no noise floor, no interleaving, no null arm and no z-score --
#   a difference of one is a difference of one. What it cannot see is cache
#   behaviour and stalls; a change that trades instructions for locality
#   has to be judged somewhere else.
#
#   Work parity: the correctness gate is `kairos conform --all`, which
#   demands a trace identical to the C kernel's for all eighteen
#   scenarios. Two runs that produce the same trace did the same work by
#   construction -- a far stronger work-count check than comparing a frame
#   count, and it is what makes an Ir delta here attributable.
#
#   Content: three scenarios, chosen to exercise different kernel paths
#   rather than to be quick. BlockQ is blocking queue send/receive with
#   task switches; GenQTest is the non-blocking and peek surface plus
#   mutexes; TimerDemo is the software timers and their daemon. A change
#   that helps one and hurts another shows up here and would not in a
#   single-scenario reading.
#
#   Denominator warning: the sim spends roughly half its instructions
#   FORMATTING THE TRACE (`core::fmt`), which firmware does not have. Read
#   the per-function kernel rows, never the program total, and never quote
#   a share of the total as though it were a share of a kernel.
set -u

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
LABEL=${1:-baseline}
# NOT /tmp: under WSL that is wiped whenever the distro shuts down, which
# it does as soon as no process holds it -- so a baseline taken in one
# invocation is gone by the next one. Keep the recordings beside the bench.
OUT=${OUT:-$ROOT/bench/kernel-ir/out}
TICKS=${TICKS:-20000}
SCENARIOS=${SCENARIOS:-"BlockQ GenQTest TimerDemo"}

mkdir -p "$OUT"

if [ "$LABEL" = "diff" ]; then
    a="$OUT/baseline.txt"
    b="$OUT/after.txt"
    if [ ! -f "$a" ] || [ ! -f "$b" ]; then
        echo "need both $a and $b"
        exit 1
    fi
    python3 "$ROOT/bench/kernel-ir/diff.py" "$a" "$b"
    exit 0
fi

cd "$ROOT/rusty_rtos_demo" || exit 1
CARGO=${CARGO:-$(command -v cargo || echo "$HOME/.cargo/bin/cargo")}
CARGO_TARGET_DIR=/tmp/kairos-prof "$CARGO" build --release -q || exit 1
BIN=/tmp/kairos-prof/release/kairos-sim

: > "$OUT/$LABEL.raw"
for s in $SCENARIOS; do
    valgrind --tool=callgrind --callgrind-out-file="$OUT/cg.$s" \
        "$BIN" "$s" "$TICKS" >/dev/null 2>"$OUT/cg.$s.err" || exit 1
    callgrind_annotate --threshold=100 "$OUT/cg.$s" 2>/dev/null \
        | grep -E 'rusty_rtos_(kernel_core|core|port_core|demo_core)' \
        >> "$OUT/$LABEL.raw"
done

# One row per function, summed over the scenarios. Python, not sed: the
# generic argument lists carry their own `::`, `<` and `>` (see names.py).
python3 "$ROOT/bench/kernel-ir/names.py" < "$OUT/$LABEL.raw" > "$OUT/$LABEL.txt"
rm -f "$OUT/$LABEL.raw"

echo "== $LABEL: Ir per function, $SCENARIOS at $TICKS ticks =="
head -22 "$OUT/$LABEL.txt"
