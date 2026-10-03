#!/bin/sh
# Every rv32 row of the tick-work cell, on BOTH ports, against a baseline.
#
#     sh bench/tick-work/rows.sh          # measure, diff against the baseline
#     sh bench/tick-work/rows.sh keep     # promote the last measurement
#
# run.sh prints the three rows it compares with FreeRTOS; the cell measures
# seventeen. Each is a median of 512 brackets under `-icount shift=0`, exact
# run to run. `RiscvPort` (`--features real-port`) is the shipped port and
# the product number; `SimPort` is what the host benches run.
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
CELL=$ROOT/rusty_rtos_kernel/firmware/riscv32-qemu-tick-work
OUT=$ROOT/bench/tick-work/out
mkdir -p "$OUT"
if [ "${1:-}" = keep ]; then
    cp "$OUT/after.rows" "$OUT/baseline.rows"
    echo "baseline <- after"
    exit 0
fi
{
    ( cd "$CELL" && cargo run --release --features real-port 2>/dev/null ) \
        | grep '^ROW ' | sed 's/^ROW \([a-z_]*\) median=\([0-9]*\).*/real \1 \2/'
    ( cd "$CELL" && cargo run --release 2>/dev/null ) \
        | grep '^ROW ' | sed 's/^ROW \([a-z_]*\) median=\([0-9]*\).*/sim \1 \2/'
} > "$OUT/after.rows"
[ -s "$OUT/after.rows" ] || { echo "no rows: the cell did not run"; exit 1; }
[ -f "$OUT/baseline.rows" ] || cp "$OUT/after.rows" "$OUT/baseline.rows"
python3 - "$OUT/baseline.rows" "$OUT/after.rows" <<'PY'
import sys
def rows(p):
    return {(a, b): int(c) for a, b, c in (l.split() for l in open(p) if l.strip())}
base, after = rows(sys.argv[1]), rows(sys.argv[2])
moved = [(k, base.get(k), v) for k, v in after.items() if base.get(k) != v]
for (port, row), b, a in sorted(moved):
    print(f"  rv32 {port:4} {row:18} {b} -> {a}  {a - (b or 0):+}")
print(f"rv32 rows: {len(after)} measured, {len(moved)} moved")
PY
