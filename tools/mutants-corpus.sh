#!/bin/sh
# Mutate the kernel and judge it with the CONFORMANCE CORPUS, self-verified.
#
#     sh tools/mutants-corpus.sh <file.rs> [extra cargo-mutants args]
#
# Run it under GIT BASH on this box, not WSL. Two reasons, and both are
# silent:
#
#   * cargo-mutants is installed for the Windows toolchain, not inside the
#     WSL distro;
#   * Windows git has core.autocrlf=true and WSL git does not, so a tree
#     that is CLEAN to one shows twelve modified files to the other. The
#     dirty-tree guard below reads whichever git it is given, and under
#     WSL it refuses to run on a perfectly clean checkout.
#
# ---- why this script exists rather than a command in the ledger ----------
#
# The kernel's own `cargo test` catches almost nothing in `kernel.rs` and
# `queue.rs` -- 13 of 342, measured -- because the corpus is their oracle by
# design. Judging those files therefore means building the DEMO against the
# mutated kernel, and that is where the trap is:
#
#   * the demo declares `rusty_rtos_kernel = { version = "0.1.0" }`, a
#     REGISTRY dependency, and reaches the local checkout only through the
#     [patch] table in its own `.cargo/config.toml`;
#   * cargo reads config from the INVOCATION directory, which for a mutants
#     run is the KERNEL repo, whose table (written by `kairos patches`, which
#     only lists siblings a repo depends on) names `rusty_rtos_core` alone;
#   * v0.1.0 IS published, so the registry resolution SUCCEEDS.
#
# So the recorded 2026-09-09 invocation, run today, builds the demo against
# the PUBLISHED kernel. A mutation of the local tree changes nothing, every
# mutant survives, and the report looks exactly like a run that found a lot
# of holes. The ledger warned about precisely this shape and the warning was
# not enough on its own, which is why the check is now IN the harness.
#
# ---- what it does -------------------------------------------------------
#
#   1. refuses to run on a dirty tree, because it edits the tree in place;
#   2. adds the demo's patch rows to the kernel's config -- they are
#      sibling-relative, so they are correct verbatim from here;
#   3. PROVES the bridge: plants a poison that cannot fail to fire (every
#      queue send counts twice), requires the gate to FAIL, removes it, and
#      requires the gate to PASS again. Either surprise aborts the run;
#   4. only then runs cargo-mutants;
#   5. restores the config and the tree on any exit.
set -eu

R=$(cd "$(dirname "$0")/.." && pwd)
K=$R/rusty_rtos_kernel
D=$R/rusty_rtos_demo
CFG=$K/.cargo/config.toml
Q=$K/crates/rusty_rtos_kernel-core/src/queue.rs
TEST=every_scenario_reproduces_the_c_kernels_trace_and_counters

FILE=${1:-}
[ -n "$FILE" ] || { echo "usage: sh tools/mutants-corpus.sh <file.rs> [args...]" >&2; exit 1; }
shift

[ -f "$K/crates/rusty_rtos_kernel-core/src/$FILE" ] || {
    echo "no such kernel-core file: $FILE" >&2; exit 1; }

# 1. A dirty tree and an in-place mutation run cannot be told apart afterwards.
if [ -n "$(cd "$K" && git status --porcelain)" ]; then
    echo "REFUSING: $K is dirty, and this runs --in-place." >&2
    exit 1
fi

restore() {
    [ -f "$CFG.h4bak" ] && mv -f "$CFG.h4bak" "$CFG"
    (cd "$K" && git checkout -- crates/ Cargo.lock 2>/dev/null) || true
    (cd "$D" && git checkout -- Cargo.lock 2>/dev/null) || true
}
trap restore EXIT

# 2. Borrow the demo's own rows, so this stays right if they change.
cp "$CFG" "$CFG.h4bak"
sed -n '/^\[patch.crates-io\]/,$p' "$D/.cargo/config.toml" \
    | grep -E "^rusty_rtos_(kernel|port)" >> "$CFG"

cd "$K"
gate() {
    cargo test --manifest-path ../rusty_rtos_demo/Cargo.toml --release \
        -p rusty_rtos_demo-core --test conformance -- "$TEST" >/tmp/mc.txt 2>&1
}

resolved=$(cargo tree --manifest-path ../rusty_rtos_demo/Cargo.toml \
    -p rusty_rtos_demo-core -i rusty_rtos_kernel-core --depth 0 2>/dev/null \
    | grep -c "rusty_rtos_kernel" || true)
echo "bridge: kernel-core resolves to $resolved local path entry/entries"

# 3. The poison. A run that cannot see THIS cannot see anything.
echo "verifying the bridge with a poison that cannot fail to fire..."
if gate; then :; else
    echo "ABORT: the gate FAILS on a clean tree -- fix that before mutating." >&2
    tail -5 /tmp/mc.txt >&2
    exit 1
fi
sed -i 's/q\.waiting = q\.waiting\.wrapping_add(1);/q.waiting = q.waiting.wrapping_add(2);/' "$Q"
if gate; then
    echo "ABORT: the gate PASSED with a poisoned kernel." >&2
    echo "The corpus is not testing the mutated tree -- almost certainly the" >&2
    echo "registry resolution described at the top of this script. Any numbers" >&2
    echo "from a run in this state would be pure fiction." >&2
    exit 1
fi
(cd "$K" && git checkout -- crates/rusty_rtos_kernel-core/src/queue.rs)
gate || { echo "ABORT: the gate still fails after removing the poison." >&2; exit 1; }
echo "bridge PROVED: poisoned fails, clean passes."

# 4. The run itself.
echo "running cargo mutants on $FILE, judged by the corpus..."
cargo mutants --in-place \
    --file "crates/rusty_rtos_kernel-core/src/$FILE" \
    --test-package rusty_rtos_demo-core \
    --timeout 300 "$@" \
    -- --manifest-path ../rusty_rtos_demo/Cargo.toml --release
