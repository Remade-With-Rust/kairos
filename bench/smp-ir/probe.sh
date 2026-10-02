#!/bin/sh
# One SMP optimisation probe: the gates, then both instruments against the
# baseline. Run from Git Bash on the Windows side (the gates are fastest
# there); the instruments run under WSL.
#
#     sh bench/smp-ir/probe.sh            # gates + measure + diff
#     sh bench/smp-ir/probe.sh keep       # ... and make this the new baseline
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
# `keep` PROMOTES the measurement already taken; it never measures. Deciding
# after seeing the numbers is the only order -- a keep that re-measured once
# promoted a regression straight into the baseline.
if [ "${1:-}" = keep ]; then
    cp "$ROOT/bench/smp-ir/out/after.tot" "$ROOT/bench/smp-ir/out/baseline.tot"
    cp "$ROOT/bench/smp-ir/out/after.txt" "$ROOT/bench/smp-ir/out/baseline.txt"
    [ -f "$ROOT/bench/smp-ir/out/k1/after.txt" ] && cp "$ROOT/bench/smp-ir/out/k1/after.txt" "$ROOT/bench/smp-ir/out/k1/baseline.txt"
    echo "baseline <- after"
    exit 0
fi
# The lockfiles are NOT reset here: during a campaign that adds a core API,
# the committed (registry-pinned) locks cannot build the kernel. Restore them
# at release, from a clean clone.
fail() { echo "GATE FAIL: $1"; exit 1; }

( cd "$ROOT/rusty_rtos_kernel" && cargo test -q --release -p rusty_rtos_kernel-core --lib --test smp_differential 2>&1 | grep -q "test result: FAILED" ) && fail "kernel lib or smp_differential"
( cd "$ROOT/rusty_rtos_kernel" && cargo test -q --release -p rusty_rtos_kernel-core --lib --test smp_differential 2>&1 | grep -c "test result: ok" | grep -q "^2$" ) || fail "kernel tests did not run"
( cd "$ROOT/rusty_rtos_demo" && cargo test -q --release -p rusty_rtos_demo-core --features smp --test smp_conformance 2>&1 | grep -q "test result: ok. 1 passed" ) || fail "smp_conformance"
( cd "$ROOT/rusty_rtos_demo" && cargo test -q --release -p rusty_rtos_demo-core --test conformance 2>&1 | grep -q "test result: ok. 3 passed" ) || fail "one-core conformance"
( cd "$ROOT/rusty_rtos_core" && cargo clippy -q -p rusty_rtos_core --all-targets -- -D warnings 2>&1 | grep -qE "^(error|warning)" ) && fail "clippy core"
( cd "$ROOT/rusty_rtos_kernel" && cargo clippy -q -p rusty_rtos_kernel-core --all-targets -- -D warnings 2>&1 | grep -qE "^(error|warning)" ) && fail "clippy kernel"
( cd "$ROOT/rusty_rtos_demo" && cargo clippy -q -p rusty_rtos_demo-core --all-targets --features smp -- -D warnings 2>&1 | grep -qE "^(error|warning)" ) && fail "clippy demo smp"
# fmt too: CI runs it, and a probe-clean tree once failed it in the release clone.
for r in rusty_rtos_core rusty_rtos_kernel rusty_rtos_demo; do
    ( cd "$ROOT/$r" && cargo fmt --all --check >/dev/null 2>&1 ) || fail "fmt $r"
done
echo "gates: ok"

wsl -e bash -lc "cd /mnt/f/coding/rusty_RTOS && sh bench/smp-ir/run.sh after >/dev/null 2>&1 && sh bench/smp-ir/run.sh diff"

# The ONE-core instrument too: an SMP change can move the one-core build
# without running there (rustc's MIR inliner weighs generic bodies before
# `C::NUMBER_OF_CORES` folds -- win 8 cost one core +63,896 that way).
# Verdict: the sum of bench/kernel-ir's kernel rows, which is its own.
wsl -e bash -lc "cd /mnt/f/coding/rusty_RTOS && OUT=/mnt/f/coding/rusty_RTOS/bench/smp-ir/out/k1 sh bench/kernel-ir/run.sh after >/dev/null 2>&1"
python3 - "$ROOT/bench/smp-ir/out/k1/baseline.txt" "$ROOT/bench/smp-ir/out/k1/after.txt" <<'PY'
import sys
def tot(p):
    t = 0
    for l in open(p, encoding="utf-8"):
        a = l.split(None, 1)
        if len(a) == 2:
            try: t += int(a[0].replace(",", ""))
            except ValueError: pass
    return t
a, b = tot(sys.argv[1]), tot(sys.argv[2])
print(f"one-core kernel rows (bench/kernel-ir): {a:,} -> {b:,}  {b - a:+,}")
PY
