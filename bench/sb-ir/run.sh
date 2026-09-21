#!/bin/sh
# Per-function instruction counts for the STREAM-BUFFER path, from callgrind.
#
#     wsl -e bash -lc 'cd /mnt/f/coding/rusty_RTOS && sh bench/sb-ir/run.sh baseline'
#     wsl -e bash -lc 'cd /mnt/f/coding/rusty_RTOS && sh bench/sb-ir/run.sh after'
#     wsl -e bash -lc 'cd /mnt/f/coding/rusty_RTOS && sh bench/sb-ir/run.sh diff'
#
# INVOKE IT THE SAME WAY EVERY TIME. A login shell and a non-login shell
# hand the process a different `environ`, libc copies it at startup, and
# the count moves by a fixed pedestal of roughly 950 instructions --
# larger than most of the wins this bench exists to find. Same session,
# same invocation, or the comparison is void.
#
# METHOD LINE, which every number taken here must be quoted with:
#
#   Instrument: callgrind Ir (instructions retired), per function. NOT a
#   clock. Deterministic to the instruction, so there is no noise floor,
#   no interleaving, no null arm and no z-score -- a difference of one is
#   a difference of one. What it cannot see is cache behaviour and
#   stalls; a change that trades instructions for locality has to be
#   judged somewhere else.
#
#   TWO ARMS, different corpus shapes over the same `stream.rs`:
#
#     sbd  StreamBufferDemo          the whole face at once -- blocking
#                                    sends and receives that time out, the
#                                    FromISR pair inside a critical
#                                    section, one-byte and 17-byte writes
#                                    that wrap the ring, a reset, and two
#                                    echo pairs crossing a priority
#                                    boundary in both directions.
#     sbi  StreamBufferInterrupt     one byte per tick from an interrupt
#          MessageBufferAMP          against a trigger level of ten, and
#                                    message buffers -- which is the
#                                    length-prefix path `sbd` never takes,
#                                    plus the replaced send-completed seam.
#
#   Quote BOTH on every probe. Roughly a third of probes move two
#   instruments in opposite directions, and the single-instrument answer
#   is wrong every time that happens. When the signs disagree, look for a
#   per-call-site split before accepting a trade.
#
#   Work parity: every scenario's own `KAIROS_RESULT` line is printed
#   beside its number, and the real gate is `kairos conform --all`, which
#   demands a trace identical to the C kernel's for all twenty-two
#   scenarios. Two runs that produce the same trace did the same work by
#   construction. If an anchor moves, STOP -- the experiment changed, not
#   the code.
#
#   THE VERDICT IS THE PROGRAM TOTAL. The per-function rows are a census
#   -- they are filtered to `rusty_rtos_*`, so `core::str::from_utf8`,
#   `memcpy` and the inlined `BufWriter` are all outside them, and a change
#   that moves work across that boundary reads as a small regression when
#   it is a large one. The first probe taken with this bench did exactly
#   that: the rows said +19.9M and the program total said +42.1M.
#
#   Denominator warning: the sim spends roughly half its instructions
#   FORMATTING THE TRACE, which firmware does not have. A DELTA on the
#   total is the verdict; a SHARE of the total is not a share of a kernel,
#   and must never be quoted as one.
set -u

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
LABEL=${1:-baseline}
OUT=${OUT:-$ROOT/bench/sb-ir/out}
TICKS=${TICKS:-20000}

ARM_SBD=${ARM_SBD:-"StreamBufferDemo"}
ARM_SBI=${ARM_SBI:-"StreamBufferInterrupt MessageBufferAMP"}

mkdir -p "$OUT"

if [ "$LABEL" = "diff" ]; then
    status=0
    for arm in sbd sbi; do
        a="$OUT/baseline.$arm.txt"
        b="$OUT/after.$arm.txt"
        if [ ! -f "$a" ] || [ ! -f "$b" ]; then
            echo "need both $a and $b" >&2
            exit 1
        fi
        echo "== $arm =="
        python3 "$ROOT/bench/kernel-ir/diff.py" "$a" "$b" || status=1
        echo
    done
    echo "== PROGRAM TOTALS -- the verdict =="
    paste "$OUT/baseline.totals" "$OUT/after.totals" 2>/dev/null \
        | awk '{printf "%-26s %16s -> %16s\n", $1, $2, $5}'
    echo
    echo "== work parity =="
    if diff -u "$OUT/baseline.anchors" "$OUT/after.anchors"; then
        echo "anchors unmoved: the two runs did the same work"
    else
        echo "ANCHORS MOVED -- the experiment changed, not the code" >&2
        status=1
    fi
    exit "$status"
fi

cd "$ROOT/rusty_rtos_demo" || exit 1
CARGO=${CARGO:-$(command -v cargo || echo "$HOME/.cargo/bin/cargo")}
CARGO_TARGET_DIR=/tmp/kairos-prof "$CARGO" build --release -q || exit 1
BIN=/tmp/kairos-prof/release/kairos-sim

: > "$OUT/$LABEL.anchors"
: > "$OUT/$LABEL.totals"

for arm in sbd sbi; do
    case "$arm" in
        sbd) scenarios=$ARM_SBD ;;
        *) scenarios=$ARM_SBI ;;
    esac
    : > "$OUT/$LABEL.$arm.raw"
    for s in $scenarios; do
        valgrind --tool=callgrind --callgrind-out-file="$OUT/cg.$s" \
            "$BIN" "$s" "$TICKS" >/dev/null 2>"$OUT/cg.$s.err" || exit 1
        # The scenario's own verdict line is the work-parity anchor.
        grep -h '^KAIROS_RESULT' "$OUT/cg.$s.err" >> "$OUT/$LABEL.anchors"
        # The PROGRAM TOTAL, which is the verdict. The per-crate rows below
        # are a census, not a score: they exclude `core::str::from_utf8`,
        # `memcpy` and everything else outside `rusty_rtos_*`, so a change
        # that moves work ACROSS that boundary reads as a small regression
        # when it is a large one. It happened on the first probe taken with
        # this bench -- the rows said +19.9M and the truth was +42.1M.
        printf '%-26s ' "$s" >> "$OUT/$LABEL.totals"
        callgrind_annotate --threshold=1 "$OUT/cg.$s" 2>/dev/null \
            | grep 'PROGRAM TOTALS' >> "$OUT/$LABEL.totals"
        callgrind_annotate --threshold=100 "$OUT/cg.$s" 2>/dev/null \
            | grep -E 'rusty_rtos_(kernel_core|core|port_core|demo_core)' \
            >> "$OUT/$LABEL.$arm.raw"
    done
    python3 "$ROOT/bench/kernel-ir/names.py" \
        < "$OUT/$LABEL.$arm.raw" > "$OUT/$LABEL.$arm.txt"
    rm -f "$OUT/$LABEL.$arm.raw"
done

for arm in sbd sbi; do
    total=$(awk '{n += $1} END {print n}' "$OUT/$LABEL.$arm.txt")
    echo "== $LABEL/$arm: $total Ir over the kernel rows, at $TICKS ticks =="
    head -12 "$OUT/$LABEL.$arm.txt"
    echo
done

echo "== PROGRAM TOTALS -- the verdict =="
cat "$OUT/$LABEL.totals"
echo
echo "== work-parity anchors =="
cat "$OUT/$LABEL.anchors"
