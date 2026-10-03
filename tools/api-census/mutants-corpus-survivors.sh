#!/bin/sh
# P5, run B: judge run A's survivors with the CORPUS.
#
#     sh tools/api-census/mutants-corpus-survivors.sh <run-A mutants.out dir>... > B.tsv
#
# Run A is `cargo mutants` over one kernel-core file with the kernel's own
# tests as the oracle (unit suite, the API differential's scripts, pins and
# sweeps on one core and two, the typed face's equivalence test, the SMP
# differential). What survives it is applied here, one mutant at a time,
# IN PLACE, and judged by the demo's corpus: the one-core `conformance` pins
# (every scenario's C trace digest and counters, the async and typed arms)
# and the two-core `smp_conformance`. Output: one line per mutant,
# `<name>\t<caught|missed|unviable|timeout>`.
#
# Git Bash on this box, for the reasons tools/mutants-corpus.sh gives. The
# gate runs FROM THE DEMO, whose own .cargo/config.toml patches the kernel to
# the local checkout -- run from the kernel, cargo would read the kernel's
# config and build the demo against the PUBLISHED kernel, and every mutant
# would survive. The bridge is proved before anything is measured: a poison
# that cannot fail to fire must fail the gate, and the clean tree must pass.
set -eu
R=$(cd "$(dirname "$0")/../.." && pwd)
K=$R/rusty_rtos_kernel
D=$R/rusty_rtos_demo
SRC=crates/rusty_rtos_kernel-core/src
LOG=${TMPDIR:-/tmp}/p5-gate.txt
# Same semantics as the release profile (overflow checks stay on), half the
# build: LTO and one codegen unit change speed, never behaviour.
FAST="--config profile.release.lto=false --config profile.release.codegen-units=16"

[ "$#" -ge 1 ] || { echo "usage: $0 <mutants.out>..." >&2; exit 1; }
if [ -n "$(cd "$K" && git status --porcelain -- crates)" ]; then
    echo "REFUSING: $K/crates is dirty, and this mutates it in place." >&2
    exit 1
fi
restore() {
    (cd "$K" && git checkout -q -- crates Cargo.lock 2>/dev/null) || true
    (cd "$D" && git checkout -q -- Cargo.lock 2>/dev/null) || true
}
trap restore EXIT

gate() {
    ( cd "$D" &&
      timeout 900 cargo test -q --release $FAST -p rusty_rtos_demo-core --test conformance &&
      timeout 900 cargo test -q --release $FAST -p rusty_rtos_demo-core --features smp --test smp_conformance
    ) > "$LOG" 2>&1
}

gate || { echo "ABORT: the corpus fails on a clean tree" >&2; tail -5 "$LOG" >&2; exit 1; }
sed -i 's/q\.waiting = q\.waiting\.wrapping_add(1);/q.waiting = q.waiting.wrapping_add(2);/' "$K/$SRC/queue.rs"
if gate; then
    echo "ABORT: the corpus PASSED a poisoned kernel -- it is not building the local tree" >&2
    exit 1
fi
(cd "$K" && git checkout -q -- "$SRC/queue.rs")
gate || { echo "ABORT: the corpus still fails after the poison is removed" >&2; exit 1; }
echo "bridge proved: poisoned fails, clean passes" >&2

for out in "$@"; do
    python3 - "$out" <<'EOF' > "${TMPDIR:-/tmp}/p5-missed.txt"
import json, sys
# LF only: on Windows a CRLF leaves a CR on the diff path and git apply fails.
sys.stdout.reconfigure(newline="\n")
d = json.load(open(sys.argv[1] + "/outcomes.json", encoding="utf-8"))
for o in d["outcomes"]:
    if o["summary"] == "MissedMutant":
        # The diff path first: it has no spaces, so plain `read` splits it off.
        print(sys.argv[1] + "/" + o["diff_path"] + " " + o["scenario"]["Mutant"]["name"])
EOF
    while read -r diff name; do
        # cargo-mutants' diff names the MUTATION on its `+++` line and has no
        # a/ b/ prefixes: point `+++` at the `---` file and apply with -p0.
        target=$(sed -n '1s/^--- //p' "$diff")
        sed "2s|^+++ .*|+++ $target|" "$diff" > "${TMPDIR:-/tmp}/p5-mutant.diff"
        if ! (cd "$K" && git apply -p0 "${TMPDIR:-/tmp}/p5-mutant.diff"); then
            printf '%s\t%s\n' "$name" "apply-failed"
            continue
        fi
        # A compile failure in the demo's build is unviable, not caught.
        if ( cd "$D" && cargo build -q --release $FAST -p rusty_rtos_demo-core --tests ) > "$LOG" 2>&1; then
            if gate; then
                verdict=missed
            else
                code=$?
                if [ "$code" -eq 124 ]; then verdict=timeout; else verdict=caught; fi
            fi
        else
            verdict=unviable
        fi
        (cd "$K" && git checkout -q -- "$SRC")
        printf '%s\t%s\n' "$name" "$verdict"
    done < "${TMPDIR:-/tmp}/p5-missed.txt"
done
