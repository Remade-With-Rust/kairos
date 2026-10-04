#!/bin/sh
# P5, run C: judge chosen survivors by `kairos conform` at FULL length.
#
#     sh tools/api-census/mutants-long-conform.sh <ticks> <scenario,...> <diff>... > C.tsv
#
# Runs A and B judge with the 2,000-tick corpus pins and the generative
# differential. Some machinery -- the stream buffers' resume after a
# preempted exit, say -- is reached only deep into a long run, so its
# survivors are applied here one at a time, IN PLACE, and judged by the
# live C oracle at full length on the scenarios named. Output: one line per
# diff, `<diff>\t<caught|missed|unviable>`.
#
# Git Bash, the kernel tree clean (it is restored to HEAD on exit), and the
# same `+++` repair `mutants-corpus-survivors.sh` makes to cargo-mutants'
# diffs.
set -eu
R=$(cd "$(dirname "$0")/../.." && pwd)
K=$R/rusty_rtos_kernel
D=$R/rusty_rtos_demo
TICKS=$1
SCENARIOS=$(printf '%s' "$2" | tr ',' ' ')
shift 2
LOG=${TMPDIR:-/tmp}/p5-long.txt

if [ -n "$(cd "$K" && git status --porcelain -- crates)" ]; then
    echo "REFUSING: $K/crates is dirty, and this mutates it in place." >&2
    exit 1
fi
restore() {
    (cd "$K" && git checkout -q -- crates Cargo.lock 2>/dev/null) || true
    (cd "$D" && git checkout -q -- Cargo.lock 2>/dev/null) || true
}
trap restore EXIT

conform() {
    for s in $SCENARIOS; do
        ( cd "$R" && tools/kairos/target/release/kairos.exe conform "$s" --ticks "$TICKS" ) >> "$LOG" 2>&1 || return 1
    done
}

: > "$LOG"
conform || { echo "ABORT: the clean tree is not identical to the C" >&2; tail -5 "$LOG" >&2; exit 1; }
echo "clean tree identical on: $SCENARIOS" >&2

for diff in "$@"; do
    target=$(sed -n '1s/^--- //p' "$diff")
    sed "2s|^+++ .*|+++ $target|" "$diff" > "${TMPDIR:-/tmp}/p5-long.diff"
    if ! (cd "$K" && git apply -p0 "${TMPDIR:-/tmp}/p5-long.diff"); then
        printf '%s\t%s\n' "$diff" "apply-failed"
        continue
    fi
    : > "$LOG"
    # `caught` needs EVIDENCE: a trace that diverges from the C's, or a
    # scenario failing its own check. Anything else -- the oracle not running,
    # WSL's transient E_UNEXPECTED -- is `oracle-error`, which mutants.py does
    # not count as a kill. Until 2026-10-03 every failure was `caught`, and
    # P5 recorded take_stream_waited -> false as killed here although it
    # behaves exactly as set_stream_waited -> (), which survived: a failed
    # oracle run read as a kill.
    if conform; then
        verdict=missed
    elif grep -q "error\[E" "$LOG"; then
        verdict=unviable
    elif grep -q "traces diverge at line\|failed its own check" "$LOG"; then
        verdict=caught
    else
        verdict=oracle-error
        tail -3 "$LOG" >&2
    fi
    (cd "$K" && git checkout -q -- crates)
    printf '%s\t%s\n' "$diff" "$verdict"
done
