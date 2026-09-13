#!/bin/sh
# The K3 footprint row, FLASH half: how much code the same set of kernel
# operations costs, Kairos against the C kernel it remakes, both on rv32,
# both dead-code-eliminated by the same linker from the same root set.
#
#     sh bench/kernel-flash/run.sh              # from the umbrella root
#
# Method, and why each part of it is there:
#
#   * **The root set is the corpus.** `docs/API-MAP.md` lists 341 FreeRTOS
#     entry points, nearly all still `planned` on our side. Linking the C
#     kernel against all of them and Kairos against the forty it implements
#     would price a feature gap, not a kernel. The operations rooted below
#     are the ones the conformance corpus exercises -- the one place the two
#     are known to do the SAME WORK -- so it is the only root set this
#     comparison can honestly use.
#   * **Neither arm is charged for the harness.** Both are linked with `-u`
#     for each operation, so the roots are the kernel's own symbols. An
#     earlier version pinned the C side with a table of function addresses
#     and paid 596 bytes for it.
#   * **Both arms are linked by `ld.lld` with `--gc-sections`**, from
#     per-function sections, at the same optimisation level, with LTO OFF on
#     the Rust side because the C side is compiled per translation unit and
#     does not get whole-program optimisation either.
#   * **Unresolved symbols are ignored on both sides**, so neither arm is
#     charged for libc. That is why `compiler_builtins` is subtracted from
#     the Rust arm below: the C arm's `memcpy` and 64-bit helpers are
#     unresolved and uncounted, so counting ours would not be symmetric.
#
# What this row does NOT claim: that this is a firmware image. It is the
# kernel and its port, with nothing above and no C library beneath.
set -e

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
cd "$ROOT"
BUILD=${BUILD:-${TMPDIR:-/tmp}/kairos-kernel-flash}
mkdir -p "$BUILD"
rm -f "$BUILD"/*.o "$BUILD"/*.elf "$BUILD"/*.map

CLANG=${CLANG:-$(command -v clang || echo "/c/Program Files/LLVM/bin/clang")}
LD=${LD:-$(command -v ld.lld || echo "/c/Program Files/LLVM/bin/ld.lld")}
NM=${NM:-$(command -v llvm-nm || echo "/c/Program Files/LLVM/bin/llvm-nm")}
SIZE=${SIZE:-$(command -v llvm-size || echo "/c/Program Files/LLVM/bin/llvm-size")}
K=oracle/FreeRTOS-Kernel
RV=$K/portable/GCC/RISC-V
CDIR=bench/kernel-ram/c          # the config and libc stubs are shared

[ -d "$K" ] || { echo "no oracle checkout at $K -- run 'kairos fetch' first"; exit 1; }

fail=0
check() {
    if [ "$2" -eq "$3" ]; then
        printf '      ok    %s (%s)\n' "$1" "$2"
    else
        printf '      FAIL  %s: got %s, pinned %s\n' "$1" "$2" "$3"
        fail=$((fail + 1))
    fi
}
text_of() { "$SIZE" -A "$1" | awk '$1 == ".text" { print $2 }'; }

# The operations the corpus exercises, as the C kernel names them.
CSYMS="xTaskCreate vTaskDelete vTaskDelay xTaskDelayUntil vTaskSuspend
vTaskResume vTaskPrioritySet uxTaskPriorityGet eTaskGetState xTaskGetHandle
xTaskAbortDelay vTaskStartScheduler vTaskSuspendAll xTaskResumeAll
xTaskIncrementTick vTaskSwitchContext xTaskGetTickCount xQueueGenericCreate
xQueueGenericSend xQueueReceive xQueuePeek uxQueueMessagesWaiting
xQueueSemaphoreTake xQueueCreateMutex xQueueCreateCountingSemaphore
xQueueGiveMutexRecursive xQueueTakeMutexRecursive xQueueGenericSendFromISR
xQueueReceiveFromISR xTaskGenericNotify xTaskGenericNotifyWait
ulTaskGenericNotifyTake xTaskGenericNotifyStateClear xTaskGenericNotifyFromISR
xTimerCreate xTimerGenericCommandFromTask xTimerIsTimerActive
xEventGroupCreate xEventGroupSetBits xEventGroupWaitBits xEventGroupClearBits
xEventGroupSync xStreamBufferGenericCreate xStreamBufferSend
xStreamBufferReceive xStreamBufferSendFromISR"

# -------------------------------------------------------------- the C arm --
echo "compiling the C kernel for rv32imac -Os (oracle checkout, unmodified)"
CFLAGS="-target riscv32-unknown-elf -march=rv32imac -mabi=ilp32 -Os"
CFLAGS="$CFLAGS -ffunction-sections -fdata-sections"
INCS="-I$CDIR/stub -I$CDIR -I$K/include -I$RV"
INCS="$INCS -I$RV/chip_specific_extensions/RV32I_CLINT_no_extensions"

for f in tasks queue list timers event_groups stream_buffer; do
    "$CLANG" $CFLAGS $INCS -include "$CDIR/FreeRTOSConfig.h" -c "$K/$f.c" -o "$BUILD/$f.o"
done
"$CLANG" $CFLAGS $INCS -include "$CDIR/FreeRTOSConfig.h" \
    -c "$K/portable/MemMang/heap_4.c" -o "$BUILD/heap_4.o"
"$CLANG" $CFLAGS $INCS -include "$CDIR/FreeRTOSConfig.h" -c "$RV/port.c" -o "$BUILD/port.o"
"$CLANG" $CFLAGS $INCS -include "$CDIR/FreeRTOSConfig.h" -c "$RV/portASM.S" -o "$BUILD/portASM.o"

CU=""
for s in $CSYMS; do CU="$CU -u $s"; done
"$LD" -o "$BUILD/c_kernel.elf" "$BUILD"/*.o $CU --gc-sections -e 0 \
    --unresolved-symbols=ignore-all -Map "$BUILD/c_kernel.map"
c_text=$(text_of "$BUILD/c_kernel.elf")

# The same link WITHOUT --gc-sections. If these two agree, the linker is
# discarding nothing and the whole comparison is measuring the wrong thing.
"$LD" -o "$BUILD/c_nogc.elf" "$BUILD"/*.o $CU -e 0 \
    --unresolved-symbols=ignore-all
c_nogc=$(text_of "$BUILD/c_nogc.elf")

# ----------------------------------------------------------- the Rust arm --
echo "building the Kairos probe for riscv32imac-unknown-none-elf"
( cd bench/kernel-flash/rs && cargo build --release -q --target riscv32imac-unknown-none-elf )
LIB=$(ls bench/kernel-flash/rs/target/riscv32imac-unknown-none-elf/release/*.a | head -1)
# The probe's operation entry points, and ONLY those. `kairos_riscv_*` are the
# port's own assembly symbols, and rooting them charged the Rust arm for both
# context switches while the C arm's roots pull in neither of theirs -- an
# asymmetry worth 150 bytes that appeared the moment the preemptive switch was
# added. The switch is measured exactly, and on both sides, by
# `bench/switch-cost`; counting it here as well would be counting it twice and
# only against one arm.
RSYMS=$("$NM" --defined-only "$LIB"           | awk '$3 ~ /^kairos_/ && $3 !~ /^kairos_riscv_/ { print $3 }' | sort -u)
RU=""
for s in $RSYMS; do RU="$RU -u $s"; done
"$LD" -o "$BUILD/rs_kernel.elf" --whole-archive "$LIB" $RU --gc-sections -e 0 \
    --unresolved-symbols=ignore-all -Map "$BUILD/rs_kernel.map"
rs_text=$(text_of "$BUILD/rs_kernel.elf")
"$LD" -o "$BUILD/rs_nogc.elf" --whole-archive "$LIB" $RU -e 0 \
    --unresolved-symbols=ignore-all
rs_nogc=$(text_of "$BUILD/rs_nogc.elf")

# `compiler_builtins` comes out because the C arm's equivalents are
# unresolved and uncounted. Counting ours would not be symmetric.
rs_builtins=$(awk '/compiler_builtins/ && /:\(\.text/ {
                       s += strtonum("0x" $3) } END { print s + 0 }' "$BUILD/rs_kernel.map")
rs_kernel=$((rs_text - rs_builtins))
ratio=$(awk -v a="$rs_kernel" -v b="$c_text" 'BEGIN { printf "%.2fx", a / b }')

# ------------------------------------------------------------------ report --
echo
echo "the same operations, dead-code-eliminated from the same root set:"
printf '  %-34s %7s\n' "" ".text"
printf '  %-34s %7d\n' "FreeRTOS kernel + RISC-V port" "$c_text"
printf '  %-34s %7d\n' "Kairos kernel + RISC-V port"   "$rs_kernel"
printf '  %-34s %7d   %s\n' "  (Kairos compiler_builtins)" "$rs_builtins" "excluded, see below"
printf '  %-34s %7s\n' "" "$ratio"
echo
printf '  Kairos costs %d bytes more flash for the same operation set.\n' \
       $((rs_kernel - c_text))
echo

echo "where the C arm's bytes are:"
awk '$0 ~ /\.o:\(\.text/ { sz = strtonum("0x" $3); n = $0
        sub(/.*\//, "", n); sub(/\.o:.*/, ".c", n); t[n] += sz }
     END { for (k in t) printf "  %-18s %6d\n", k, t[k] }' "$BUILD/c_kernel.map" | sort -k2 -nr
echo

echo "the tick-width asymmetry, which is NOT a language cost:"
echo "  Kairos uses a u64 tick; the FreeRTOS RISC-V port types TickType_t as"
echo "  portUBASE_TYPE (portmacro.h:68), so on rv32 it is 32-bit and"
echo "  configTICK_TYPE_WIDTH_IN_BITS is not honoured at all. Setting it to 64"
echo "  was tried: the type stayed 32-bit and clang reported a constant"
echo "  truncating to 0 -- a broken build, not a matched one. So part of the"
printf '  gap above is a 64-bit tick that never wraps, including the %d bytes\n' "$rs_builtins"
echo "  of compiler_builtins already excluded. It is a design difference."
echo

echo "instrument checks -- a linker that discards nothing measures nothing:"
if [ "$c_nogc" -gt "$c_text" ]; then
    printf '      ok    --gc-sections removed %d bytes from the C arm\n' $((c_nogc - c_text))
else
    printf '      FAIL  --gc-sections removed nothing from the C arm\n'
    fail=$((fail + 1))
fi
if [ "$rs_nogc" -gt "$rs_text" ]; then
    printf '      ok    --gc-sections removed %d bytes from the Rust arm\n' $((rs_nogc - rs_text))
else
    printf '      FAIL  --gc-sections removed nothing from the Rust arm\n'
    fail=$((fail + 1))
fi

echo
echo "pinned totals -- these fail loudly when either kernel changes size:"
check "FreeRTOS kernel + port" "$c_text"    13924
# 16,064 until 2026-09-11, when the root set stopped including the port's own
# `kairos_riscv_*` assembly. That was never symmetric -- the C arm's roots pull
# in neither of ITS context switches -- and it only became visible when adding
# the preemptive switch moved the number. The pin went DOWN because the bug was
# in our favour to remove, not because the kernel shrank.
check "Kairos kernel + port"   "$rs_kernel" 16008

echo
if [ "$fail" -eq 0 ]; then
    echo "RESULT: PASS -- Kairos is $ratio the C kernel's flash for the operation"
    echo "        set the corpus proves byte-identical."
else
    echo "RESULT: FAIL -- $fail check(s) failed"
    exit 1
fi
