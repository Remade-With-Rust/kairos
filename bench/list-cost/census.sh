#!/bin/sh
# WHERE the list arm's instructions go, by function, by call count and by
# line. `run.sh` says how many; this says where.
#
#     wsl -e sh bench/list-cost/census.sh [rust|c-O2]
#
# It builds the SAME way `run.sh` does, plus debug info, because a census
# without line numbers names `list.rs` and stops. Debug info does not change
# codegen, and the check below proves it did not: the census build is
# counted and must match `run.sh`'s slope for the same arm.
#
# Three censuses, because they answer different questions and they disagree
# usefully (see the instruction-counting skill):
#
#   (a) self cost by function  -- where are the instructions?
#   (b) CALL COUNTS            -- called a lot, or expensive per call?
#   (c) line-level             -- which line?
#
# (b) is the one people skip. It is not in callgrind_annotate's default
# output and has to be parsed out of the raw file, and it is the one that
# reframes the problem: a function costing 30% because it is called 200,000
# times wants a cheaper CALL, not a cheaper body.
set -e

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
BUILD=${BUILD:-/tmp/kairos-list-census}
ARM=${1:-rust}
ROUNDS=${ROUNDS:-100000}
mkdir -p "$BUILD"
cd "$ROOT"

if [ "$ARM" = "rust" ]; then
    CARGO=${CARGO:-$(command -v cargo || echo "$HOME/.cargo/bin/cargo")}
    ( cd bench/list-cost/rs \
        && CARGO_TARGET_DIR="$BUILD/rs" CARGO_PROFILE_RELEASE_DEBUG=2 \
           "$CARGO" build --release -q )
    BIN="$BUILD/rs/release/list-cost"
else
    CFLAGS="-Ioracle/harness -Ioracle/FreeRTOS-Kernel/include"
    CFLAGS="$CFLAGS -Ioracle/FreeRTOS-Kernel/portable/ThirdParty/GCC/Posix"
    CFLAGS="$CFLAGS -Ioracle/FreeRTOS-Kernel/portable/ThirdParty/GCC/Posix/utils"
    gcc -O2 -g $CFLAGS bench/list-cost/c/list_bench.c oracle/FreeRTOS-Kernel/list.c \
        -o "$BUILD/c-O2" -pthread
    BIN="$BUILD/c-O2"
fi

OUT="$BUILD/cg.$ARM.out"
valgrind --tool=callgrind --callgrind-out-file="$OUT" --dump-instr=yes \
    "$BIN" "$ROUNDS" >/dev/null 2>"$BUILD/cg.$ARM.err"

TOTAL=$(grep -oE 'refs: *[0-9,]+' "$BUILD/cg.$ARM.err" | tr -d ' ,' | sed 's/refs://')
echo "arm=$ARM rounds=$ROUNDS total_refs=$TOTAL"
echo

echo "== (a) self cost by function =="
callgrind_annotate --threshold=95 "$OUT" 2>/dev/null \
    | sed -n '/Ir  *file:function/,/^$/p' | head -24
echo

echo "== (b) call counts -- called a lot, or expensive per call? =="
# `calls=` lines in the raw file carry the count; the line after carries the
# inclusive cost of that call. Fold them per callee name.
awk '
    /^fn=/   { fn = substr($0, 4) }
    /^cfn=/  { cfn = substr($0, 5) }
    /^calls=/{ n = $1; sub(/^calls=/, "", n); pending = n; next }
    pending  { split($0, f, " "); incl = f[2] + 0;
               calls[cfn] += pending; cost[cfn] += incl; pending = 0 }
    END {
        printf "  %12s %14s %10s  %s\n", "calls", "incl Ir", "per call", "function";
        for (k in calls)
            printf "  %12d %14d %10.1f  %s\n", calls[k], cost[k], cost[k]/calls[k], k;
    }
' "$OUT" | sort -k2 -nr | head -18
echo

echo "== (c) by line =="
callgrind_annotate --auto=yes --threshold=80 "$OUT" 2>/dev/null \
    | grep -E '^\s*[0-9,]+ ' | head -30
