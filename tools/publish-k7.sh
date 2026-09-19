#!/usr/bin/env bash
# Publish the remaining twelve Kairos packages at v0.1.0 -- the K7 libraries
# (json, sntp, mqtt, backoff), the C ABI and the conformance corpus.
#
# crates.io throttles NEW crate names: a small burst, then roughly one per ten
# minutes. All twelve of these are new names, so expect this to take a couple
# of hours. A 429 is waited out rather than failed on.
#
# Order is dependency order and is not negotiable: each facade depends on its
# `-core` by version, so the core must be on the index first. The siblings
# these depend on (rusty_rtos_core, _kernel, _port) are already published at
# 0.1.0 and were verified byte-identical to the local working copies before
# this ran, so each repo's [patch.crates-io] override is inert.
#
#     bash tools/publish-k7.sh
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

STEPS=(
    "rusty_rtos_json:rusty_rtos_json-core"
    "rusty_rtos_json:rusty_rtos_json"
    "rusty_rtos_sntp:rusty_rtos_sntp-core"
    "rusty_rtos_sntp:rusty_rtos_sntp"
    "rusty_rtos_mqtt:rusty_rtos_mqtt-core"
    "rusty_rtos_mqtt:rusty_rtos_mqtt"
    "rusty_rtos_backoff:rusty_rtos_backoff-core"
    "rusty_rtos_backoff:rusty_rtos_backoff"
    "rusty_rtos-capi:rusty_rtos-capi-core"
    "rusty_rtos-capi:rusty_rtos-capi"
    "rusty_rtos_demo:rusty_rtos_demo-core"
    "rusty_rtos_demo:rusty_rtos_demo"
)

# A 429 names the time it will accept the next one. Ten minutes plus a little
# covers it without parsing dates out of an error message.
WAIT=620
# The index can lag a successful publish by a few seconds; a facade that cannot
# yet see its core is a wait, not a failure.
INDEX_WAIT=45

published=0
skipped=0

for step in "${STEPS[@]}"; do
    repo="${step%%:*}"
    crate="${step##*:}"
    cd "$ROOT/$repo" || exit 1

    for attempt in 1 2 3 4 5 6 7 8; do
        printf '=== %-28s attempt %d  (%s)\n' "$crate" "$attempt" "$(date +%H:%M:%S)"
        out="$(cargo publish -p "$crate" --allow-dirty 2>&1)"

        if printf '%s' "$out" | grep -q "Published $crate\|Uploaded $crate"; then
            echo "    PUBLISHED $crate"
            published=$((published + 1))
            break
        fi
        if printf '%s' "$out" | grep -q "already exists\|already uploaded"; then
            echo "    already on the index, skipping"
            skipped=$((skipped + 1))
            break
        fi
        if printf '%s' "$out" | grep -q "429 Too Many Requests\|rate limit"; then
            echo "    rate limited; waiting ${WAIT}s"
            sleep "$WAIT"
            continue
        fi
        # A facade published seconds after its core can outrun the index.
        if printf '%s' "$out" | grep -q "no matching package\|failed to select a version"; then
            echo "    core not visible on the index yet; waiting ${INDEX_WAIT}s"
            sleep "$INDEX_WAIT"
            continue
        fi

        echo "    FAILED, and not a rate limit:"
        printf '%s\n' "$out" | tail -25
        exit 1
    done
done

echo
echo "done: $published published, $skipped already present"

# Any cargo run inside a package rewrites its Cargo.lock, stripping the
# registry source from sibling entries. Committing that breaks
# `cargo build --locked` in CI. Put them all back.
cd "$ROOT" || exit 0
for repo in rusty_rtos_json rusty_rtos_sntp rusty_rtos_mqtt \
            rusty_rtos_backoff rusty_rtos-capi rusty_rtos_demo; do
    cd "$ROOT/$repo" 2>/dev/null || continue
    if ! git diff --quiet -- Cargo.lock 2>/dev/null; then
        git checkout -- Cargo.lock && echo "reverted $repo/Cargo.lock"
    fi
done
