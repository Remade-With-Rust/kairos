#!/bin/sh
# Mutate an architecture PORT and judge it with its own QEMU cells.
#
#     sh tools/mutants-qemu.sh <crate> <file.rs> [extra cargo-mutants args]
#     sh tools/mutants-qemu.sh rusty_rtos_port-cortex-m lib.rs
#
# Run it under GIT BASH, not WSL: cargo-mutants and QEMU are installed for
# the Windows toolchain, and WSL git lacks core.autocrlf so a clean tree
# reads as modified there. Same trap as tools/mutants-corpus.sh.
#
# ---- why this exists ----------------------------------------------------
#
# `rusty_rtos_port-cortex-m` and `rusty_rtos_port-riscv` have NO tests, and
# mutating them on the host measures nothing: their source is arch-gated,
# so on x86-64 every mutation lands in code that is not compiled, the build
# succeeds, the tests pass, and the mutant is scored MISSED. Measured: 87
# survivors in the cortex-m crate and 47 in the riscv one, all artefact.
#
# The cells under `rusty_rtos_port/firmware/` DO run that code on its own
# architecture and already end in `debug::exit(EXIT_SUCCESS)`, so they are
# gates. `crates/rusty_rtos_port-core/tests/qemu_cells.rs` makes them
# reachable from `cargo test`, which is the only thing cargo-mutants runs.
#
# ---- the two things that make this trustworthy --------------------------
#
#   1. A BROKEN PORT HANGS rather than failing -- PendSV never fires, so
#      nothing reaches the semihosting exit and QEMU never returns. The
#      cell test imposes its own deadline and launches QEMU as a direct
#      child it can kill; `cargo run` would leave it orphaned on Windows.
#
#   2. The bridge is PROVED before anything is measured, with a poison
#      chosen per crate, because a run that silently tests an unmutated
#      port reports the same shape as one that tests nothing.
set -eu

R=$(cd "$(dirname "$0")/.." && pwd)
P=$R/rusty_rtos_port

CRATE=${1:-}
FILE=${2:-}
if [ -z "$CRATE" ] || [ -z "$FILE" ]; then
    echo "usage: sh tools/mutants-qemu.sh <crate> <file.rs> [args...]" >&2
    exit 1
fi
shift 2

SRC=$P/crates/$CRATE/src/$FILE
[ -f "$SRC" ] || { echo "no such file: $SRC" >&2; exit 1; }

# The poison, and the cells that can see it. One per crate, named here
# rather than guessed: a generic poison cannot prove a firmware bridge.
case "$CRATE" in
    rusty_rtos_port-cortex-m)
        # ICSR.PENDSVSET moved off its bit: PendSV is never requested, so
        # no switch ever happens and the cell runs for ever.
        POISON_FROM='const PENDSVSET: u32 = 1 << 28;'
        POISON_TO='const PENDSVSET: u32 = 1 << 27;'
        # Narrowable: `switch` is the only cortex-m cell that does NOT
        # depend on `rusty_rtos_kernel`, so it is the one that can run
        # while that repository is being mutated.
        FILTER=${KAIROS_CELL_FILTER:-cortex_m}
        ;;
    rusty_rtos_port-riscv)
        # NOT YET CHOSEN. The anchor has to be a line that exists and whose
        # corruption a cell can SEE; one guessed from the cortex-m shape
        # did not exist in this crate at all. Refusing is the right answer
        # -- a poison that cannot be planted proves nothing, and a harness
        # that skips its own proof is the thing this script exists to stop.
        echo "no poison chosen for $CRATE yet; pick one that a riscv cell" >&2
        echo "can see, and verify it fails before measuring anything." >&2
        exit 1
        ;;
    *)
        echo "no poison defined for $CRATE -- add one before measuring it" >&2
        exit 1
        ;;
esac

if [ -n "$(cd "$P" && git status --porcelain)" ]; then
    echo "REFUSING: $P is dirty, and this runs --in-place." >&2
    exit 1
fi

restore() { (cd "$P" && git checkout -- crates/ Cargo.lock 2>/dev/null) || true; }
trap restore EXIT

export KAIROS_QEMU_CELLS=1
echo "KAIROS_QEMU_CELLS is set: the cells will actually boot."

cells() {
    (cd "$P" && cargo test -p "$CRATE" --test qemu_cells "$FILTER" \
        >/tmp/mq.txt 2>&1)
}

# NOTE ON THE FILTER. cargo-mutants appends the args after `--` to EVERY
# cargo test it runs, including one scoped to the MUTATED package -- and
# `--test qemu_cells` is a TARGET selector that exists only in
# rusty_rtos_port-core, so that invocation dies with "no test target named
# qemu_cells" and the whole run is refused before a single mutant is
# tested. A bare NAME filter is valid in any package: it selects the cell
# tests where they exist and matches nothing, harmlessly, where they do not.

echo "verifying the bridge with a poison that cannot fail to fire..."
if cells; then :; else
    echo "ABORT: the cells FAIL on a clean tree -- fix that before mutating." >&2
    tail -12 /tmp/mq.txt >&2
    exit 1
fi

grep -qF "$POISON_FROM" "$SRC" || {
    echo "ABORT: the poison anchor is not in $SRC any more:" >&2
    echo "  $POISON_FROM" >&2
    echo "Pick a new one; a poison that cannot be planted proves nothing." >&2
    exit 1
}
# `sed -i` with a literal, so no regex metacharacter can quietly not match.
python -c "import io,sys; p=sys.argv[1]; s=io.open(p,encoding='utf-8',newline='').read(); a=sys.argv[2]; b=sys.argv[3]; assert a in s; io.open(p,'w',encoding='utf-8',newline='').write(s.replace(a,b,1))" \
    "$SRC" "$POISON_FROM" "$POISON_TO"

if cells; then
    echo "ABORT: the cells PASSED with a poisoned port." >&2
    echo "They are not running the mutated code. Any numbers from a run in" >&2
    echo "this state would be fiction." >&2
    exit 1
fi
restore
cells || { echo "ABORT: the cells still fail after removing the poison." >&2; exit 1; }
echo "bridge PROVED: poisoned fails, clean passes."

# PRE-FLIGHT. The poison proof above runs the cells DIRECTLY; it says
# nothing about whether cargo-mutants' OWN invocation runs them. It did
# not: `--test-workspace` defaults to false and OVERRIDES `--test-package`,
# so cargo-mutants tested the mutated package alone -- which has no tests
# at all -- logged "running 0 tests", and scored all 87 mutants against
# nothing. Every one of those numbers was fiction.
#
# So run ONE mutant first and require its baseline to have executed a cell.
echo "pre-flight: one mutant, to prove cargo-mutants itself runs the cells..."
(cd "$P" && cargo mutants --in-place --file "crates/$CRATE/src/$FILE" \
    --timeout 300 --shard 1/1000 -- "$FILTER" \
    >/tmp/mq-pre.txt 2>&1) || true
if grep -q "running 0 tests" "$P/mutants.out/log/baseline.log" 2>/dev/null; then
    echo "ABORT: cargo-mutants ran ZERO tests in its own baseline." >&2
    echo "It would score every mutant against nothing, and the report would" >&2
    echo "look exactly like a run that found a great many holes." >&2
    exit 1
fi
echo "pre-flight OK: cargo-mutants' baseline executed a cell."

echo "running cargo mutants on $CRATE/$FILE, judged by its QEMU cells..."
set +e
(cd "$P" && cargo mutants --in-place \
    --file "crates/$CRATE/src/$FILE" \
    --timeout 300 "$@" \
    -- "$FILTER")
verdict=$?
set -e

restore
trap - EXIT
if [ "$verdict" -ne 0 ]; then
    echo "cargo mutants exited $verdict -- non-zero means survivors" >&2
fi
exit "$verdict"
