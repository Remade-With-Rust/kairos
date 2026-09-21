#!/bin/sh
# THE CHIP: the K7 packages computing on a Cortex-M3, against the host pins.
#
#     sh tools/vertical/chip.sh
#
# Run it under GIT BASH, not WSL. QEMU and the thumbv7m target are installed
# for the WINDOWS toolchain; WSL has neither. Same trap as
# tools/mutants-qemu.sh, and it is worth repeating because the failure looks
# like a missing target rather than a wrong shell.
#
# ---- what this settles -------------------------------------------------
#
# Six K7 packages were proved against C by differentials, and every one of
# those differentials ran on x86-64. This runs the same work at half the
# pointer width and requires byte-identical answers. See
# firmware/mps2-an385-k7/README.md for why the digests are comparable across
# architectures at all, and for the two poisons that prove the gate can fail.
set -eu

HERE=$(cd "$(dirname "$0")" && pwd)
CELL=$HERE/firmware/mps2-an385-k7

command -v qemu-system-arm >/dev/null || {
    echo "no qemu-system-arm on PATH." >&2
    echo "This needs the Windows QEMU and git bash, not WSL." >&2
    exit 2
}
rustup target list --installed | grep -qx thumbv7m-none-eabi || {
    echo "thumbv7m-none-eabi is not installed:" >&2
    echo "    rustup target add thumbv7m-none-eabi" >&2
    exit 2
}

echo "== the host arm =="
cd "$HERE/k7-battery"
cargo test --quiet
echo "  host digests match the pins"
echo

echo "== the chip arm =="
cd "$CELL"
# A broken cell EXITS rather than hanging -- it has no scheduler to wedge --
# but the deadline is cheap insurance against a QEMU that never returns.
cargo run --release --quiet
