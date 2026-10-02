#!/bin/sh
# Instruction counts for the TWO-CORE kernel, from callgrind -- the
# instrument for SMP optimisation work.
#
#     wsl -e sh -c 'sh bench/smp-ir/run.sh baseline'
#     wsl -e sh -c 'sh bench/smp-ir/run.sh after'
#     wsl -e sh -c 'sh bench/smp-ir/run.sh diff'
#
# METHOD LINE (as bench/kernel-ir's, which this mirrors):
#
#   Instrument: callgrind Ir, deterministic to the instruction. TWO shapes,
#   quoted together on every probe, because a third of probes move two
#   shapes in opposite directions:
#     A  the two-core demo corpus -- `kairos-sim --features smp` on
#        semtest, BlockQ and recmutex at 20,000 ticks: real task bodies,
#        blocking, mutexes and inheritance across two cores;
#     B  the two-core scheduler differential -- the 20,000-step random
#        scripts of `tests/smp_differential.rs`, with and without blocking
#        waits: the scheduler's cross-core paths with no demo around them.
#   Verdict: the PROGRAM TOTAL of each. Kernel rows are printed to say
#   where, never to decide.
#
#   Work parity: the anchors line (trace lines, ticks and yields per
#   scenario for A; the differential's own pass for B) is printed beside
#   the totals. The correctness gate is separate and stronger -- the nine
#   scenarios of `smp_conformance` and both SMP differentials identical to
#   the C kernel -- and every kept change passes it.
#
#   Width blindness applies as for kernel-ir: this is a 64-bit host.
set -u

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
LABEL=${1:-baseline}
OUT=${OUT:-$ROOT/bench/smp-ir/out}
TICKS=${TICKS:-20000}
SCENARIOS=${SCENARIOS:-"semtest BlockQ recmutex"}
mkdir -p "$OUT"

if [ "$LABEL" = "diff" ]; then
    python3 "$ROOT/bench/smp-ir/diff.py" "$OUT/baseline.tot" "$OUT/after.tot" "$OUT/baseline.txt" "$OUT/after.txt"
    exit 0
fi

CARGO=${CARGO:-$(command -v cargo || echo "$HOME/.cargo/bin/cargo")}
TD=${TARGET_DIR:-$HOME/.cache/kairos-smp-prof}

# ---- A: the two-core demo corpus -------------------------------------------
( cd "$ROOT/rusty_rtos_demo" && CARGO_TARGET_DIR="$TD" "$CARGO" build --release -q -p rusty_rtos_demo --features smp ) || exit 1
BIN=$TD/release/kairos-sim
: > "$OUT/$LABEL.raw"
: > "$OUT/$LABEL.tot"
for s in $SCENARIOS; do
    # The trace is the program's stderr; valgrind's own report goes to a log.
    # A scenario that fails its own check exits non-zero, which is not an error.
    valgrind --tool=callgrind --log-file="$OUT/cg.$s.log" --callgrind-out-file="$OUT/cg.$s" \
        "$BIN" "$s" "$TICKS" >/dev/null 2>"$OUT/trace.$s"
    tot=$(grep -o 'Collected : [0-9]*' "$OUT/cg.$s.log" | grep -o '[0-9]*$')
    anchor=$(grep KAIROS_RESULT "$OUT/trace.$s" | sed 's/ exits=[0-9]*//')
    echo "A $s $tot $anchor" >> "$OUT/$LABEL.tot"
    callgrind_annotate --threshold=100 "$OUT/cg.$s" 2>/dev/null \
        | grep -E 'rusty_rtos_(kernel_core|core|port_core|demo_core)' >> "$OUT/$LABEL.raw"
done

# ---- B: the two-core scheduler differential --------------------------------
( cd "$ROOT/rusty_rtos_kernel" && CARGO_TARGET_DIR="$TD" "$CARGO" test --release -q -p rusty_rtos_kernel-core --test smp_differential --no-run ) >/dev/null 2>&1 || exit 1
TB=$(ls -t "$TD"/release/deps/smp_differential-* 2>/dev/null | grep -v '\.d$' | head -1)
valgrind --tool=callgrind --log-file="$OUT/cg.diff.log" --callgrind-out-file="$OUT/cg.diff" "$TB" --test-threads=1 >"$OUT/diff.out" 2>&1
tot=$(grep -o 'Collected : [0-9]*' "$OUT/cg.diff.log" | grep -o '[0-9]*$')
pass=$(grep -o 'test result: [a-z]*\. [0-9]* passed' "$OUT/diff.out")
echo "B differential $tot $pass" >> "$OUT/$LABEL.tot"
callgrind_annotate --threshold=100 "$OUT/cg.diff" 2>/dev/null \
    | grep -E 'rusty_rtos_(kernel_core|core)' >> "$OUT/$LABEL.raw"

python3 "$ROOT/bench/kernel-ir/names.py" < "$OUT/$LABEL.raw" > "$OUT/$LABEL.txt"
rm -f "$OUT/$LABEL.raw"
echo "== $LABEL =="
cat "$OUT/$LABEL.tot"
head -25 "$OUT/$LABEL.txt"
