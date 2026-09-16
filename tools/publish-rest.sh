#!/usr/bin/env bash
# Publish the rest of the v0.1.0 set, respecting crates.io's rate limit.
#
# crates.io throttles NEW crate names: a small burst, then roughly one per ten
# minutes. Four of the twelve went through before it fired
# (`rusty_rtos_core`, `rusty_rtos_alloc`, `rusty_rtos_kernel-core`,
# `rusty_rtos_kernel`); this publishes the remaining eight, waiting out a 429
# rather than failing on it.
#
# Order is dependency order and is not negotiable: a crate cannot be published
# until everything it depends on is on the index.
#
#     bash tools/publish-rest.sh
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

STEPS=(
    "rusty_rtos_port:rusty_rtos_port-core"
    "rusty_rtos_port:rusty_rtos_port-cortex-m"
    "rusty_rtos_port:rusty_rtos_port-riscv"
    "rusty_rtos_port:rusty_rtos_port-xtensa"
    "rusty_rtos_port:rusty_rtos_port-host"
    "rusty_rtos_port:rusty_rtos_port"
    "rusty_rtos_heap:rusty_rtos_heap-core"
    "rusty_rtos_heap:rusty_rtos_heap"
)

# A 429 names the time it will accept the next one. Ten minutes plus a little
# covers it without parsing dates out of an error message.
WAIT=620

for step in "${STEPS[@]}"; do
    repo="${step%%:*}"
    crate="${step##*:}"
    cd "$ROOT/$repo" || exit 1

    for attempt in 1 2 3 4 5 6; do
        printf '=== %-28s attempt %d\n' "$crate" "$attempt"
        out="$(cargo publish -p "$crate" --allow-dirty 2>&1)"
        if printf '%s' "$out" | grep -q "Published $crate"; then
            echo "    PUBLISHED $crate"
            break
        fi
        if printf '%s' "$out" | grep -q "already exists"; then
            echo "    already on the index, skipping"
            break
        fi
        if printf '%s' "$out" | grep -q "429 Too Many Requests"; then
            echo "    rate limited; waiting ${WAIT}s"
            sleep "$WAIT"
            continue
        fi
        echo "    FAILED, and not a rate limit:"
        printf '%s\n' "$out" | tail -12
        exit 1
    done
done

echo
echo "All eight published."
