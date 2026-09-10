#!/bin/sh
# The whole house-stack gate, in one command.
#
#     sh tools/house-gate/run.sh            # everything
#     sh tools/house-gate/run.sh --quick    # skip the two host workspaces
#
# This exists because the README used to describe the gate as a shell loop
# and a couple of extra commands, and on 2026-09-10 two of those commands
# turned out never to have run at all: `host/` and `hostgit/` are packages
# inside this workspace, whose `members = ["gate-*"]` does not match them,
# so cargo refused both while the README recorded passing times. A gate
# spread across a README is a gate with rungs nobody runs.
#
# Two rules it encodes:
#
#   * **The rungs are discovered, not listed.** `gate-*` comes off the
#     filesystem, so adding a member cannot leave it un-run — the old loop
#     named seven of the eight that exist.
#   * **An expected failure is declared, not tolerated.** `gate-xml` is
#     supposed to fail: no Kairos package will ever parse XML, and the rung
#     exists so the crates.io `no-std` category is not mistaken for a fact.
#     A gate that exits non-zero every day is a gate nobody reads, and one
#     that ignores failures is not a gate. So an EXPECTED failure is green
#     and an expected failure that starts PASSING is red, because that means
#     the world moved and this file did not.
set -u

ROOT=$(cd "$(dirname "$0")" && pwd)
QUICK=0
[ "${1:-}" = "--quick" ] && QUICK=1

TARGETS="thumbv7em-none-eabihf riscv32imac-unknown-none-elf"
# Rungs that are supposed to fail, with why. Anything else failing is a bug.
EXPECTED_FAIL="gate-xml"
# Rungs needing rusty_alloc's two opt-ins.
NEEDS_CFGS="gate-alloc"

cd "$ROOT" || exit 1
fails=0
unexpected=0

note() { printf '%-14s %-30s %s\n' "$1" "$2" "$3"; }

echo "== bare-metal rungs =="
for dir in gate-*/; do
    g=${dir%/}
    for t in $TARGETS; do
        if [ "$g" = "$NEEDS_CFGS" ]; then
            RUSTFLAGS="--cfg ra_single_threaded --cfg ra_small_profile" \
                cargo check -p "$g" --target "$t" -q >/dev/null 2>&1
        else
            cargo check -p "$g" --target "$t" -q >/dev/null 2>&1
        fi
        rc=$?
        expected=0
        for e in $EXPECTED_FAIL; do
            [ "$g" = "$e" ] && expected=1
        done
        if [ $rc -eq 0 ] && [ $expected -eq 0 ]; then
            note "$g" "$t" "OK"
        elif [ $rc -ne 0 ] && [ $expected -eq 1 ]; then
            note "$g" "$t" "FAIL (expected -- not a Kairos want)"
        elif [ $rc -eq 0 ] && [ $expected -eq 1 ]; then
            note "$g" "$t" "PASSES BUT WAS EXPECTED TO FAIL -- update this script"
            unexpected=$((unexpected + 1))
        else
            note "$g" "$t" "FAIL"
            fails=$((fails + 1))
        fi
    done
done

echo
echo "== dependency policy =="
if cargo deny check >/dev/null 2>&1; then
    note "deny" "gate workspace" "OK"
else
    note "deny" "gate workspace" "FAIL"
    fails=$((fails + 1))
fi

if [ $QUICK -eq 1 ]; then
    echo
    echo "(--quick: host/ and hostgit/ skipped)"
else
    echo
    echo "== host-only workspaces =="
    # Each has its own [workspace] table; without it cargo walks up to this
    # one and refuses, which is how both went a day without running.
    if (cd host && cargo check -q >/dev/null 2>&1); then
        note "host" "cargo check" "OK"
    else
        note "host" "cargo check" "FAIL"
        fails=$((fails + 1))
    fi
    # `host` carries three `unmaintained` advisories from spacedb-sdk and
    # ffai-core. None is a vulnerability and none is in a bare-metal graph,
    # so it is reported and does not fail the gate -- see docs/HOUSE-STACK.md.
    if (cd host && cargo deny check advisories >/dev/null 2>&1); then
        note "host" "deny advisories" "OK"
    else
        note "host" "deny advisories" "advisories present (known, host-only)"
    fi
    if (cd hostgit && CARGO_NET_GIT_FETCH_WITH_CLI=true cargo check -q >/dev/null 2>&1); then
        note "hostgit" "cargo check" "OK"
    else
        note "hostgit" "cargo check" "FAIL"
        fails=$((fails + 1))
    fi
fi

echo
if [ $fails -eq 0 ] && [ $unexpected -eq 0 ]; then
    echo "gate: everything that should build, builds"
    exit 0
fi
echo "gate: $fails unexpected failure(s), $unexpected rung(s) that should have failed and did not"
exit 1
