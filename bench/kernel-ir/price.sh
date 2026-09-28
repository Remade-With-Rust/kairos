#!/bin/sh
# Price one candidate against the FIXED baseline, on BOTH axes this bench can
# see: the per-function kernel rows AND each scenario's program total.
#
#     sh bench/kernel-ir/price.sh <label>
#
# Why the program total is here and `run.sh` does not print it: a change that
# moves work OUT of a kernel function into a callee (or into `core`) shows as a
# win on the rows and nothing on the total. `codec-measurement`'s rule that one
# probe must measure the level ABOVE the change, applied to this instrument --
# the row is the function, the total is the program.
#
# The kernel rows still exclude `core::fmt` and `from_utf8` by construction
# (run.sh greps for the crate names), so read the total for anything that
# touches a name or an Event, and the rows for everything else.
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
LABEL=${1:-after}
OUT=$ROOT/bench/kernel-ir/out

sh "$ROOT/bench/kernel-ir/run.sh" "$LABEL" >/dev/null 2>&1 || { echo "run failed"; exit 1; }

echo "== $LABEL vs baseline: program totals (all scenarios) =="
tot=0
for s in BlockQ GenQTest TimerDemo; do
    ir=$(callgrind_annotate --threshold=1 "$OUT/cg.$s" 2>/dev/null \
         | grep "PROGRAM TOTALS" | sed 's/^ *//;s/ .*//' | tr -d ,)
    printf "  %-10s %12s\n" "$s" "$ir"
    tot=$((tot + ir))
done
echo "  ---------- ------------"
printf "  %-10s %12s\n" TOTAL "$tot"
echo
echo "== per-function rows that moved =="
python3 "$ROOT/bench/kernel-ir/diff.py" "$OUT/baseline.txt" "$OUT/$LABEL.txt"
