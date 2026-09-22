#!/bin/sh
# K3's tick and switch WORK rows on rv32: what one tick of kernel costs in
# Kairos against the FreeRTOS it remakes, in RETIRED INSTRUCTIONS, with both
# arms running on the same machine under the same instrument.
#
# THE BASIS, and why it is this one.
#
# A host callgrind comparison of these two kernels was built and REFUTED on
# 2026-09-21 (docs/LEDGER.md): FreeRTOS's `xTaskIncrementTick` self cost is
# 2.7% of its inclusive and ours is 13.1%, so a self-cost ratio measures how
# differently two codebases factor a tick, not what a tick costs. Work parity
# was perfect and did NOT save it -- the anchors prove both arms did the same
# WORK, not that the boundary encloses the same THING.
#
# So the boundary is stated explicitly: two `minstret` reads around the call.
# Under `-icount shift=0` that CSR is exactly reproducible -- the sibling cell
# riscv32-qemu-switch read 199,902 three times running with it and swung 20%
# without it.
#
# This is a WORK row and NOT a cycle row. Cycles come from a part
# (rusty_rtos_kernel/firmware/xiao-s3-cycles); QEMU has no clock worth the
# name, which the Cortex-M cell measured six ways.
#
# TWO GATES, both of which can fail the run:
#
#   PARITY  both arms must report identical anchors. Different work, no
#           comparison.
#   POISON  every arm is built TWICE -- the measured call made once per
#           bracket, then twice -- and each row must move by roughly one
#           call's worth. A bracket that measures the loop instead of the
#           callee does not move, and that is the failure this catches.
set -u

cd "$(dirname "$0")/../.." || exit 1
ROOT=$(pwd)

CLANG=${CLANG:-$(command -v clang || echo "/c/Program Files/LLVM/bin/clang")}
LD=${LD:-$(command -v ld.lld || echo "/c/Program Files/LLVM/bin/ld.lld")}
QEMU=${QEMU:-$(command -v qemu-system-riscv32)}
BUILD=${BUILD:-${TMPDIR:-/tmp}/kairos-tick-work}
mkdir -p "$BUILD" || exit 1

K=oracle/FreeRTOS-Kernel
RV=$K/portable/GCC/RISC-V
CD=bench/tick-work/c
RUSTCELL=rusty_rtos_kernel/firmware/riscv32-qemu-tick-work

# QEMU here is a native Windows binary, so a `file:` argument is NOT path
# converted the way `-kernel` is. Give it a Windows path or it opens nothing
# and the run reads as a hang. (Cost an hour once; the serial output was being
# written to a path QEMU could not create, and the program looked dead.)
winpath() {
    wp_dir=$(dirname "$1")
    wp_base=$(basename "$1")
    # `pwd -W` is MSYS asking Windows what it calls this directory. Doing the
    # translation by hand works for /c/... and silently FAILS for /tmp/...,
    # where the output file is simply never created and the run reads as a
    # hang with no output at all.
    ( cd "$wp_dir" 2>/dev/null &&
      printf '%s/%s' "$(pwd -W 2>/dev/null || pwd)" "$wp_base" )
}

# ------------------------------------------------------------- the C arm ----
# FreeRTOS V11.3.1 out of the pinned oracle checkout, UNMODIFIED, with the
# oracle's own first-party RISC-V port. Nothing here is a stub or a rewrite.
build_c() {
    rep=$1
    out=$2
    cflags="-target riscv32-unknown-elf -march=rv32imac -mabi=ilp32 -O2"
    cflags="$cflags -ffreestanding -fno-builtin -Wall -DKAIROS_REPEAT=$rep"
    incs="-I$CD -Ibench/kernel-ram/c/stub -I$K/include -I$RV"
    incs="$incs -I$RV/chip_specific_extensions/RISCV_no_extensions"
    objs=""

    for f in tasks queue list timers event_groups stream_buffer; do
        "$CLANG" $cflags $incs -include "$CD/FreeRTOSConfig.h" \
            -c "$K/$f.c" -o "$BUILD/$f.o" || return 1
        objs="$objs $BUILD/$f.o"
    done

    "$CLANG" $cflags $incs -include "$CD/FreeRTOSConfig.h" \
        -c "$K/portable/MemMang/heap_4.c" -o "$BUILD/heap_4.o" || return 1
    "$CLANG" $cflags $incs -include "$CD/FreeRTOSConfig.h" \
        -c "$RV/port.c" -o "$BUILD/port.o" || return 1
    "$CLANG" $cflags $incs -include "$CD/FreeRTOSConfig.h" \
        -c "$RV/portASM.S" -o "$BUILD/portASM.o" || return 1
    "$CLANG" $cflags $incs -c "$CD/start.S" -o "$BUILD/start.o" || return 1
    "$CLANG" $cflags $incs -include "$CD/FreeRTOSConfig.h" \
        -c "$CD/main.c" -o "$BUILD/main.o" || return 1

    objs="$objs $BUILD/heap_4.o $BUILD/port.o $BUILD/portASM.o"
    objs="$objs $BUILD/start.o $BUILD/main.o"
    "$LD" -o "$out" -T "$CD/link.ld" $objs || return 1
}

run_c() {
    elf=$1
    txt=$2
    rm -f "$txt"
    "$QEMU" -machine virt -cpu rv32 -display none \
        -serial "file:$(winpath "$txt")" -bios none -icount shift=0 \
        -kernel "$elf" >/dev/null 2>&1
    cat "$txt" 2>/dev/null
}

# ---------------------------------------------------------- the Rust arm ----
run_rust() {
    feats=$1
    ( cd "$RUSTCELL" && cargo run --release $feats 2>/dev/null )
}

# ------------------------------------------------------------ the compare ---
field() { printf '%s\n' "$2" | grep "^ROW $1 " | sed 's/.*median=\([0-9]*\).*/\1/'; }
anchor() { printf '%s\n' "$1" | grep '^ANCHOR ' | head -1; }

echo "building and running the C arm (FreeRTOS V11.3.1, oracle, unmodified)"
build_c 1 "$BUILD/c1.elf" || { echo "FAIL: the C arm would not build"; exit 1; }
build_c 2 "$BUILD/c2.elf" || { echo "FAIL: the C poison arm would not build"; exit 1; }
C1=$(run_c "$BUILD/c1.elf" "$BUILD/c1.txt")
C2=$(run_c "$BUILD/c2.elf" "$BUILD/c2.txt")

echo "building and running the Kairos arm"
R1=$(run_rust "")
R2=$(run_rust "--features poison")

for out in "$C1" "$C2" "$R1" "$R2"; do
    case "$out" in
        *"RESULT: PASS"*) ;;
        *) echo "FAIL: an arm did not reach RESULT: PASS"; echo "$out"; exit 1 ;;
    esac
done

# ---- GATE 1: work parity -----------------------------------------------
ac=$(anchor "$C1")
ar=$(anchor "$R1")
if [ "$ac" != "$ar" ]; then
    echo "FAIL (PARITY): the arms did different work, so there is no comparison."
    echo "  C    $ac"
    echo "  Rust $ar"
    exit 1
fi
echo "PARITY ok -- both arms: $ac"

# ---- GATE 2: poison ----------------------------------------------------
echo
printf '%-16s %8s %8s %8s %8s\n' "" "C x1" "C x2" "Rust x1" "Rust x2"
poison_failed=0
for row in tick_idle tick_delayed switch_select; do
    c1=$(field "$row" "$C1"); c2=$(field "$row" "$C2")
    r1=$(field "$row" "$R1"); r2=$(field "$row" "$R2")
    printf '%-16s %8s %8s %8s %8s\n' "$row" "$c1" "$c2" "$r1" "$r2"
    # Doubling the call must add at least half a call's worth on both arms.
    for pair in "C:$c1:$c2" "Rust:$r1:$r2"; do
        arm=${pair%%:*}; rest=${pair#*:}; one=${rest%%:*}; two=${rest#*:}
        [ -z "$one" ] || [ -z "$two" ] && continue
        if [ "$two" -lt $(( one + one / 2 )) ]; then
            echo "  POISON FAIL: $arm $row did not move when the call was doubled"
            echo "               ($one -> $two): the bracket is not measuring the callee."
            poison_failed=1
        fi
    done
done
[ "$poison_failed" -eq 0 ] || { echo; echo "FAIL (POISON)"; exit 1; }
echo "POISON ok -- doubling the call moved every row on both arms"

# ----------------------------------------------------------- the rows -------
echo
echo "retired instructions per call, rv32imac, -O2 both arms, no LTO either"
printf '%-16s %10s %10s %8s\n' row FreeRTOS Kairos ratio
for row in tick_idle tick_delayed switch_select; do
    c=$(field "$row" "$C1"); r=$(field "$row" "$R1")
    ratio=$(awk -v a="$r" -v b="$c" 'BEGIN{ if (b>0) printf "%.2fx", a/b; else print "-" }')
    printf '%-16s %10s %10s %8s\n' "$row" "$c" "$r" "$ratio"
done

echo
echo "A SWITCH IS TWO HALVES, and this bench measures one of them."
echo "  bench/switch-cost prices the register half from the same oracle:"
echo "  cooperative 30 (Kairos) against 83 (FreeRTOS), preemptive 74 against 83."
cs=$(field switch_select "$C1"); rs=$(field switch_select "$R1")
awk -v c="$cs" -v r="$rs" 'BEGIN{
    printf "  whole cooperative switch: FreeRTOS %d+83 = %d, Kairos %d+30 = %d  (%.2fx)\n", c, c+83, r, r+30, (r+30)/(c+83);
    printf "  whole preemptive switch:  FreeRTOS %d+83 = %d, Kairos %d+74 = %d  (%.2fx)\n", c, c+83, r, r+74, (r+74)/(c+83);
}'

echo
echo "RESULT: PASS"
