#!/bin/sh
# The API census: which kernel code each ORACLE actually executes, from
# LLVM source-based coverage -- not a grep.
#
#     sh tools/api-census/run.sh          # from the Kairos umbrella (Git Bash)
#     python3 tools/api-census/census.py  # then render docs/API-COVERAGE.md
#
# Three groups, each its own profile:
#
#   C1   compared against the C kernel, ONE core: rusty_rtos_demo's
#        `conformance` pins -- every scenario's C trace digest and counters
#        (and the async arm) -- the two tests that compare, by exact name --
#        and the kernel's `api_differential`, one-core script and pins.
#   C2   compared against the C kernel, TWO cores: `smp_conformance`, the
#        kernel's `smp_differential` (both scripts) and `api_differential`'s
#        two-core script and pins.
#   EQ   the typed face's wrapper-equivalence test (`typed_equivalence`):
#        not compared to the C, but each wrapper proved to make exactly the
#        C-twinned calls it claims. The census credits it to typed.rs only.
#   ANY  every test in the kernel and the demo, default and smp features:
#        "executed by something", which is NOT "compared".
#   C1L  compared against the C kernel, one core, at FULL length: given
#        CONFORM_LOG=<the output of `kairos conform --all --ticks N`>, the
#        instrumented `kairos-sim` re-runs every scenario that log reports
#        identical, at that log's tick count -- and refuses a run whose trace
#        is not the same number of lines the C was compared on. The sim is
#        deterministic, so this is the Rust side of that comparison. The pins
#        (C1) are 2,000 ticks; TimerDemo's Test6 starts well after that.
#
# A group whose tests fail aborts the run: coverage from a run that disagreed
# with the C would count disagreement as evidence.
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${OUT:-$ROOT/tools/api-census/out}
TD=${CENSUS_TARGET:-${TMPDIR:-/tmp}/kc}
HOST=$(rustc -vV | sed -n 's/^host: //p')
LLVM="$(cd "$ROOT/rusty_rtos_kernel" && rustc --print sysroot)/lib/rustlib/$HOST/bin"
export RUSTFLAGS="-C instrument-coverage"
export CARGO_TARGET_DIR="$TD"

rm -rf "$OUT"
mkdir -p "$OUT"
# What this run measures, fingerprinted before anything is built: census.py
# refuses to render from coverage of other source, and --check compares the
# published census against the checkouts with exactly these.
python3 "$ROOT/tools/api-census/census.py" --fingerprint > "$OUT/inputs.json"

# run GROUP DIR "CARGO ARGS" "TEST ARGS"
# CENSUS_GROUPS="C1" runs only those groups (the kill test re-runs C1 alone).
# Not GROUPS: bash reserves that name and silently ignores assignments to it.
CENSUS_GROUPS=${CENSUS_GROUPS:-"C1 C2 EQ ANY"}

run() {
    g=$1 d=$2 cargs=$3 targs=$4
    case " $CENSUS_GROUPS " in *" $g "*) ;; *) return 0;; esac
    mkdir -p "$OUT/$g"
    echo "== $g: $d cargo test --release $cargs -- $targs"
    # shellcheck disable=SC2086
    ( cd "$ROOT/$d" && LLVM_PROFILE_FILE="$OUT/$g/%p-%m.profraw" \
        cargo test -q --release $cargs -- $targs ) || { echo "FAILED: $g in $d"; exit 1; }
    # shellcheck disable=SC2086
    ( cd "$ROOT/$d" && cargo test -q --release $cargs --no-run --message-format=json ) \
        | python3 -c "import sys,json
for l in sys.stdin:
    try: m=json.loads(l)
    except ValueError: continue
    if m.get('reason')=='compiler-artifact' and m.get('executable'): print(m['executable'])" \
        >> "$OUT/$g.objects"
}

run C1 rusty_rtos_demo "-p rusty_rtos_demo-core --test conformance" \
    "--exact every_scenario_reproduces_the_c_kernels_trace_and_counters the_async_arm_reproduces_pollqs_trace_exactly"
# The API differential (docs/finished/api-differential.md, P1): every step of a
# seeded script compared against FreeRTOS, one core and two.
run C1 rusty_rtos_kernel "-p rusty_rtos_kernel-core --test api_differential"     "--exact one_core_answers_every_step_as_the_c_kernel_does"
# The digest pins (seeds that once found a defect) and the authored sweeps
# (plan P3), each build's to its column.
export API_PINS=api1-
run C1 rusty_rtos_kernel "-p rusty_rtos_kernel-core --test api_differential"     "--exact pinned_seeds_answer_every_step_as_the_c_kernel_does authored_sweeps_answer_every_step_as_the_c_kernel_does"
export API_PINS=api2-
run C2 rusty_rtos_kernel "-p rusty_rtos_kernel-core --test api_differential"     "--exact pinned_seeds_answer_every_step_as_the_c_kernel_does authored_sweeps_answer_every_step_as_the_c_kernel_does"
unset API_PINS
run C2 rusty_rtos_demo "-p rusty_rtos_demo-core --features smp --test smp_conformance" ""
run C2 rusty_rtos_kernel "-p rusty_rtos_kernel-core --test api_differential"     "--exact two_cores_answer_every_step_as_the_c_kernel_does"
run C2 rusty_rtos_kernel "-p rusty_rtos_kernel-core --test smp_differential" ""
# EQ: the typed face's wrapper-equivalence test (plan P4). Not compared to the
# C: it proves each wrapper makes exactly the C-twinned calls it claims, and
# those calls are what C1 and C2 compare. The census credits it to typed.rs only.
run EQ rusty_rtos_kernel "-p rusty_rtos_kernel-core --test typed_equivalence" ""
run ANY rusty_rtos_kernel "--workspace" ""
run ANY rusty_rtos_demo "--workspace" ""
# Not `--tests`: the ONE-core `conformance` pins fail by construction when the
# demo is built two-core (the scenarios are single-core), which is why the
# demo's CI runs only `smp_conformance` under `smp`. Same here.
run ANY rusty_rtos_demo "-p rusty_rtos_demo-core --features smp --lib --test smp_conformance" ""

if [ -n "${CONFORM_LOG:-}" ]; then
    mkdir -p "$OUT/C1L"
    ( cd "$ROOT/rusty_rtos_demo" && cargo build -q --release -p rusty_rtos_demo --bin kairos-sim )
    SIM="$TD/release/kairos-sim$(case "$HOST" in *windows*) echo .exe;; esac)"
    echo "$SIM" > "$OUT/C1L.objects"
    : > "$OUT/C1L.meta"
    sed -n 's/^\([A-Za-z-]*\): \([0-9]*\) lines identical, \([0-9]*\) ticks$/\1 \2 \3/p' "$CONFORM_LOG" |
    while read -r name lines ticks; do
        got=$(LLVM_PROFILE_FILE="$OUT/C1L/$name-%p.profraw" "$SIM" "$name" "$ticks" 2>&1 >/dev/null | wc -l)
        # `conform` counts the trace without the closing KAIROS_RESULT line.
        if [ "$got" -ne "$lines" ] && [ "$got" -ne $((lines + 1)) ]; then
            echo "C1L: $name traced $got lines, conform compared $lines -- refusing"; exit 1
        fi
        echo "$name $ticks $lines" >> "$OUT/C1L.meta"
    done
    echo "C1L: $(wc -l < "$OUT/C1L.meta") scenarios at full length"
else
    echo "C1L skipped: set CONFORM_LOG to a \`kairos conform --all --ticks N\` log"
fi

for g in C1 C1L C2 EQ ANY; do
    [ -d "$OUT/$g" ] || continue
    "$LLVM/llvm-profdata" merge -sparse "$OUT/$g"/*.profraw -o "$OUT/$g.profdata"
    objs=$(sort -u "$OUT/$g.objects" | sed 's/^/-object /' | tr '\n' ' ')
    # shellcheck disable=SC2086
    "$LLVM/llvm-cov" export -format=text -skip-expansions \
        -instr-profile="$OUT/$g.profdata" $objs \
        -ignore-filename-regex='(rustc|registry|\.cargo|library)' > "$OUT/$g.json"
    echo "$g: $(wc -c < "$OUT/$g.json") bytes of coverage"
done

# A source edit DURING the run would mix two programs' coverage.
python3 "$ROOT/tools/api-census/census.py" --fingerprint > "$OUT/inputs.end.json"
python3 - "$OUT/inputs.json" "$OUT/inputs.end.json" <<'PYEND' || exit 1
import json, sys
a, b = (json.load(open(p, encoding="utf-8"))["inputs"] for p in sys.argv[1:3])
moved = [k for k in a if a[k] != b.get(k)]
if moved:
    sys.exit("inputs changed during the run: %s -- refusing this coverage" % ", ".join(moved))
PYEND

# The cargo-paths trap: building inside the packages rewrote their lockfiles.
for r in rusty_rtos_kernel rusty_rtos_demo; do
    ( cd "$ROOT/$r" && git checkout -q -- Cargo.lock 2>/dev/null || true )
done
