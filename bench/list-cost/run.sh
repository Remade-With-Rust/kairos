#!/bin/sh
# The K1 arena-list cost row: how many instructions the index-linked list in
# `rusty_rtos_core` costs against the pointer-linked `list.c` it remakes.
#
#     wsl -e sh bench/list-cost/run.sh          # from the umbrella root
#
# Method, and why each part of it is there:
#
#   * Both arms do identical work in an identical order and print a checksum
#     of every value their list operations returned. The checksums must
#     match; two instruction counts of two different programs would not be a
#     comparison. The script stops if they differ.
#   * Nothing is timed. Instructions are counted with callgrind, which is
#     deterministic, so there is no noise floor to argue about and no need
#     to interleave arms.
#   * Each arm is counted at three run lengths. The cost of one round is the
#     slope, not a single count: process start-up, libc, the dynamic loader
#     and the final printf are constant and cancel exactly. The second
#     difference is printed as the linearity check — it must be ~0, or the
#     slope is not a slope.
#
# One round is 40 operations on 8 items: 8 appends to a ready list, 8
# round-robin picks, 8 removals, 8 ordered inserts into a delayed list, and
# 8 more removals. That is the mix `prvAddTaskToReadyList`,
# `taskSELECT_HIGHEST_PRIORITY_TASK` and `prvAddCurrentTaskToDelayedList`
# actually make.
# It needs a full working tree, not a bare clone of the umbrella: the Rust
# arm depends on ../../../rusty_rtos_core by path and the C arm compiles
# oracle/FreeRTOS-Kernel/list.c, and both of those are gitignored siblings
# that `kairos` fetches. Run it under WSL, where gcc and valgrind live.
set -e

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
BUILD=${BUILD:-/tmp/kairos-list-cost}
mkdir -p "$BUILD"

CFLAGS="-Ioracle/harness -Ioracle/FreeRTOS-Kernel/include"
CFLAGS="$CFLAGS -Ioracle/FreeRTOS-Kernel/portable/ThirdParty/GCC/Posix"
CFLAGS="$CFLAGS -Ioracle/FreeRTOS-Kernel/portable/ThirdParty/GCC/Posix/utils"
SRC="bench/list-cost/c/list_bench.c oracle/FreeRTOS-Kernel/list.c"

cd "$ROOT"
echo "building the C arm (gcc -O2 and -O3, FreeRTOS-Kernel/list.c unmodified)"
gcc -O2 $CFLAGS $SRC -o "$BUILD/c-O2" -pthread
gcc -O3 $CFLAGS $SRC -o "$BUILD/c-O3" -pthread
# AND at the TARGET's width. Every Kairos target is 32-bit, and a cost
# shaped like a `usize` is invisible on a 64-bit host -- `heap4.rs` carried
# one worth 7.6% that measured byte-identical here. A bench that only ever
# runs 64-bit cannot report on the code that ships.
if gcc -m32 -O2 $CFLAGS $SRC -o "$BUILD/c32-O2" -pthread 2>/dev/null; then
    HAVE32=yes
else
    HAVE32=no
    echo "  (no 32-bit C arm: gcc-multilib absent)"
fi

echo "building the Rust arm (cargo release: opt-level 3, lto, one codegen unit)"
# `sh -lc` is not a login shell, so cargo may not be on PATH yet.
CARGO=${CARGO:-$(command -v cargo || echo "$HOME/.cargo/bin/cargo")}
( cd bench/list-cost/rs && CARGO_TARGET_DIR="$BUILD/rs" "$CARGO" build --release -q )
cp "$BUILD/rs/release/list-cost" "$BUILD/rust"
if [ "$HAVE32" = yes ]; then
    if ( cd bench/list-cost/rs && CARGO_TARGET_DIR="$BUILD/rs"             "$CARGO" build --release -q --target i686-unknown-linux-gnu 2>/dev/null ); then
        cp "$BUILD/rs/i686-unknown-linux-gnu/release/list-cost" "$BUILD/rust32"
    else
        HAVE32=no
        echo "  (no 32-bit Rust arm: rustup target add i686-unknown-linux-gnu)"
    fi
fi

echo
echo "correctness gate — the two arms must agree on every returned value:"
a=$( "$BUILD/c-O2" 1000 )
b=$( "$BUILD/rust" 1000 )
echo "  C    $a"
echo "  Rust $b"
[ "${a#*checksum=}" = "${b#*checksum=}" ] || { echo "MISMATCH: the arms did different work"; exit 1; }
if [ "$HAVE32" = yes ]; then
    # The 32-bit arms are gated too. An arm that is measured but not
    # checked is a number with nothing behind it, and these two are the
    # ones that describe the code Kairos ships.
    c=$( "$BUILD/c32-O2" 1000 )
    d=$( "$BUILD/rust32" 1000 )
    echo "  C   32 $c"
    echo "  Rust 32 $d"
    [ "${c#*checksum=}" = "${a#*checksum=}" ] || { echo "MISMATCH: the 32-bit C arm"; exit 1; }
    [ "${d#*checksum=}" = "${a#*checksum=}" ] || { echo "MISMATCH: the 32-bit Rust arm"; exit 1; }
fi

count() {
    valgrind --tool=callgrind --callgrind-out-file="$BUILD/cg.out" "$1" "$2" \
        >/dev/null 2>"$BUILD/cg.err"
    grep -oE 'refs: *[0-9,]+' "$BUILD/cg.err" | tr -d ' ,' | sed 's/refs://'
}

echo
echo "instructions, by arm and run length (callgrind, 40 list operations per round):"
ARMS="c-O2 c-O3 rust"
[ "$HAVE32" = yes ] && ARMS="$ARMS c32-O2 rust32"
for arm in $ARMS; do
    n1=$( count "$BUILD/$arm" 100000 )
    n2=$( count "$BUILD/$arm" 200000 )
    n3=$( count "$BUILD/$arm" 300000 )
    echo "$arm $n1 $n2 $n3"
done | awk '{
    d1 = $3 - $2; d2 = $4 - $3;
    printf "  %-6s 100k=%-12d 200k=%-12d 300k=%-12d  per round=%8.2f  per op=%6.2f  linearity=%+d\n",
           $1, $2, $3, $4, d2 / 100000, d2 / 100000 / 40, d2 - d1;
}'
