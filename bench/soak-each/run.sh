#!/bin/sh
# K3's one-hour clause, one scenario at a time.
#
#     sh bench/soak-each/run.sh riscv32-qemu-corpus
#     sh bench/soak-each/run.sh mps2-an385-qemu-corpus
#
# # Why not just run the cell's own eighteen-scenario soak
#
# Because it takes about eight hours per architecture and no session holds a
# process open that long -- three attempts died partway. Splitting it is not
# a weaker test, though, and that is the point worth making: the cell builds
# a FRESH kernel per scenario (`Runner::kernel_for` inside the loop), so
# eighteen runs of one scenario is the same computation as one run of
# eighteen, in the same order, with the same lengths. Nothing is shared
# across the loop iterations for splitting them to lose.
#
# What it costs is a rebuild per scenario, because `KAIROS_SOAK_ONLY` is a
# compile-time `option_env!`. That is about twenty seconds against the tens
# of minutes `semtest` alone takes, so it is not the term that matters.
#
# # What the hour claims
#
# Liveness, not conformance: there are no C pins at 3,600,000 ticks, so this
# asks exactly what the C demo asks -- is every scenario's own check task
# still reporting that it is running. `ticks >= 3,600,000` is asserted by the
# cell, so a scenario that stopped early cannot pass quietly.
set -e

CELL=${1:-riscv32-qemu-corpus}
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
DIR="$ROOT/rusty_rtos_demo/firmware/$CELL"
[ -d "$DIR" ] || { echo "no such cell: $DIR"; exit 1; }

# The pin table's order, which is the order the cell would run them in.
SCENARIOS="dynamic PollQ BlockQ semtest countsem recmutex blocktim QPeek
GenQTest QueueOverwrite QueueSetPolling IntSemTest StreamBufferInterrupt
TimerDemo EventGroupsDemo MessageBufferAMP PollQ-typed death"

cd "$DIR"
echo "=== the corpus for one hour of simulated time, one scenario per run ==="
echo "cell    $CELL"
echo "length  3,600,000 ticks (an hour at the oracle's own TICK_RATE_HZ of 1000)"
echo "claim   liveness -- every check task still reports running"
echo

pass=0
fail=0
began_all=$(date +%s)

for s in $SCENARIOS; do
    began=$(date +%s)
    out=$(KAIROS_SOAK_ONLY="$s" cargo run --release --features soak 2>/dev/null \
          | grep -E "^$s " || true)
    took=$(( $(date +%s) - began ))
    if [ -n "$out" ]; then
        printf '  %-22s %s  [%ds]\n' "" "$out" "$took"
        pass=$((pass + 1))
    else
        printf '  %-22s NO VERDICT  [%ds]\n' "$s" "$took"
        fail=$((fail + 1))
    fi
done

echo
echo "ran in $(( ($(date +%s) - began_all) / 60 )) minutes"
if [ "$fail" -eq 0 ] && [ "$pass" -eq 18 ]; then
    echo "RESULT: PASS -- all 18 scenarios still running after an hour on $CELL"
else
    echo "RESULT: FAIL -- $pass passed, $fail without a verdict (of 18)"
    exit 1
fi
