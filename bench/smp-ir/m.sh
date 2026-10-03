#!/bin/sh
# Price one candidate on every Ir instrument at once, NO gates (probe.sh runs
# those; use it before keeping anything). One invocation, so every arm is
# measured on the same rung of the exactness ladder.
#
#     sh bench/smp-ir/m.sh            # measure HEAD-or-working-tree as `after`
#     sh bench/smp-ir/m.sh keep       # make this measurement the baseline
#
# Prints: the two-core shapes (A rows + B), the one-core kernel rows, and the
# one-core PROGRAM totals per scenario -- the rows exclude `from_utf8`,
# `memcpy` and `core::fmt`, so a change that moves work into them reads as a
# win on the rows and nothing on the total (instruction-counting: the
# filtered subtotal lies).
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
K1=$ROOT/bench/smp-ir/out/k1
if [ "${1:-}" = keep ]; then
    cp "$ROOT/bench/smp-ir/out/after.tot" "$ROOT/bench/smp-ir/out/baseline.tot"
    cp "$ROOT/bench/smp-ir/out/after.txt" "$ROOT/bench/smp-ir/out/baseline.txt"
    cp "$K1/after.txt" "$K1/baseline.txt"
    cp "$K1/after.prog" "$K1/baseline.prog"
    echo "baseline <- after"
    exit 0
fi
wsl -e bash -lc "cd /mnt/f/coding/rusty_RTOS && sh bench/smp-ir/run.sh after >/dev/null 2>&1 && sh bench/smp-ir/run.sh diff | grep -E '^(shape|A |B |anchors)'"
wsl -e bash -lc "cd /mnt/f/coding/rusty_RTOS && OUT=/mnt/f/coding/rusty_RTOS/bench/smp-ir/out/k1 sh bench/kernel-ir/run.sh after >/dev/null 2>&1 && for s in BlockQ GenQTest TimerDemo; do printf '%s ' \$s; callgrind_annotate --threshold=100 bench/smp-ir/out/k1/cg.\$s 2>/dev/null | grep 'PROGRAM TOTALS' | sed 's/^ *//;s/ .*//' | tr -d ,; done > bench/smp-ir/out/k1/after.prog"
python3 - "$K1" <<'PY'
import sys, os
k1 = sys.argv[1]
def tot(p):
    t = 0
    for l in open(p, encoding="utf-8"):
        a = l.split(None, 1)
        if len(a) == 2:
            try: t += int(a[0].replace(",", ""))
            except ValueError: pass
    return t
def rows(p):
    d = {}
    for l in open(p, encoding="utf-8"):
        a = l.split(None, 1)
        if len(a) == 2:
            try: d[a[1].strip()] = d.get(a[1].strip(), 0) + int(a[0].replace(",", ""))
            except ValueError: pass
    return d
a, b = tot(f"{k1}/baseline.txt"), tot(f"{k1}/after.txt")
print(f"one-core kernel rows      {a:>14,} -> {b:>14,}  {b - a:+,}")
def prog(p):
    if not os.path.exists(p): return {}
    w = open(p).read().split()
    return {w[i]: int(w[i + 1]) for i in range(0, len(w) - 1, 2)}
pa, pb = prog(f"{k1}/baseline.prog"), prog(f"{k1}/after.prog")
for s in pb:
    print(f"one-core program {s:9} {pa.get(s, 0):>14,} -> {pb[s]:>14,}  {pb[s] - pa.get(s, 0):+,}")
ra, rb = rows(f"{k1}/baseline.txt"), rows(f"{k1}/after.txt")
moved = sorted(((rb.get(k, 0) - ra.get(k, 0), k) for k in set(ra) | set(rb)), key=lambda x: -abs(x[0]))
for d, k in moved[:8]:
    if d: print(f"   {d:+12,}  {k}")
PY
# The one-core anchors: each scenario's KAIROS_RESULT line, exits included.
for s in BlockQ GenQTest TimerDemo; do
    grep -h KAIROS_RESULT "$K1/cg.$s.err" 2>/dev/null | head -1
done
