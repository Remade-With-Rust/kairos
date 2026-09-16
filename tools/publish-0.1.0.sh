#!/usr/bin/env bash
# The remaining v0.1.0 publishes, in the only order that resolves.
#
# `rusty_rtos_core` 0.1.0 is already on crates.io (2026-09-16). Everything here
# depends on it directly or transitively, and a crate cannot be published until
# every dependency is on the index -- so this is a sequence, not a list, and
# each step waits for the previous one to be available.
#
# Run from the umbrella root with a crates.io token already in
# ~/.cargo/credentials.toml:
#
#     bash tools/publish-0.1.0.sh            # dry run, the default
#     bash tools/publish-0.1.0.sh --publish  # for real, and it is irreversible
#
# A crates.io publish CANNOT be undone. A version can be yanked, which stops new
# dependents resolving it, but it can never be deleted and the version number can
# never be reused. That is why this defaults to a dry run.
set -euo pipefail

MODE="--dry-run"
EXTRA=""
if [ "${1:-}" = "--publish" ]; then
    MODE=""
    echo "PUBLISHING FOR REAL. This cannot be undone."
    echo
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# repo:crate, in dependency order.
STEPS=(
    "rusty_rtos_core:rusty_rtos_alloc"

    "rusty_rtos_kernel:rusty_rtos_kernel-core"
    "rusty_rtos_kernel:rusty_rtos_kernel"

    "rusty_rtos_port:rusty_rtos_port-core"
    "rusty_rtos_port:rusty_rtos_port-cortex-m"
    "rusty_rtos_port:rusty_rtos_port-riscv"
    "rusty_rtos_port:rusty_rtos_port-xtensa"
    "rusty_rtos_port:rusty_rtos_port-host"
    "rusty_rtos_port:rusty_rtos_port"

    "rusty_rtos_heap:rusty_rtos_heap-core"
    "rusty_rtos_heap:rusty_rtos_heap"
)

# Deliberately NOT here:
#
#   rusty_rtos-capi    -- the publishable crate is a 15-line `lib.rs`; the 87
#                         ABI symbols live in `seam/abi.rs`, which is pulled in
#                         by `#[path]` from the cells and is not a library
#                         crate. Publishing it would ship the ABI's rules
#                         without the ABI.
#   rusty_rtos_json    -- 27-line scaffolds. Publishing them claims a name with
#   rusty_rtos_backoff    nothing behind it.
#   rusty_rtos_demo    -- depends on kernel and port; publish after those land
#                         if the corpus is wanted on crates.io at all.

fail=0
for step in "${STEPS[@]}"; do
    repo="${step%%:*}"
    crate="${step##*:}"
    printf '=== %-28s (%s)\n' "$crate" "$repo"
    cd "$ROOT/$repo"
    if cargo publish $MODE $EXTRA -p "$crate" 2>&1 | tail -3; then
        :
    else
        echo "  FAILED: $crate"
        fail=1
        break
    fi
    cd "$ROOT"
done

if [ "$fail" = "0" ]; then
    echo
    if [ -z "$MODE" ]; then
        echo "All published. Tag each repo and record the release in its ledger."
    else
        echo "Dry run clean. Re-run with --publish to make it real."
    fi
fi
exit "$fail"
