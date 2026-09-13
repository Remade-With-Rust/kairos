#!/bin/sh
# The K3 footprint row, RAM half: what a kernel occupies before any task
# exists, and what each additional task costs -- Kairos against the C kernel
# it remakes, both on rv32, both at the same configuration.
#
#     sh bench/kernel-ram/run.sh                # from the umbrella root
#
# Method, and why each part of it is there:
#
#   * Both arms are measured ON THE TARGET. `size_of` on the host is a
#     different number, because a host `usize` is eight bytes and rv32's is
#     four. The C arm is compiled for rv32imac; the Rust arm's sizes are the
#     LENGTHS of arrays in an rv32 staticlib, read back with `llvm-nm`.
#     Neither number is typed in, and neither comes from this box's word
#     size.
#   * The configurations are matched where they can be: 5 priorities,
#     16-byte task names, a 10-deep timer queue, one notification entry.
#     Those are the knobs that size the arrays, and they hold the same
#     values on both sides. Matching them is what makes the two totals
#     comparable rather than merely adjacent.
#   * Each Rust figure differs from the baseline in EXACTLY ONE dimension,
#     so what is reported per unit is a slope between two measured points,
#     never a total divided by a count.
#   * Two of those slopes are known in advance: a notification slot is a
#     u64 and a stream-buffer byte is a byte. The script CHECKS them. An
#     instrument that cannot reproduce a number true by construction has not
#     earned belief in the numbers that are not.
#
# What this row does NOT claim: that the two kernels cost what they cost for
# the same reason. They do not, and "the two floors" below is the finding.
set -e

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
cd "$ROOT"
BUILD=${BUILD:-${TMPDIR:-/tmp}/kairos-kernel-ram}
mkdir -p "$BUILD"

CLANG=${CLANG:-$(command -v clang || echo "/c/Program Files/LLVM/bin/clang")}
NM=${NM:-$(command -v llvm-nm || echo "/c/Program Files/LLVM/bin/llvm-nm")}
SIZE=${SIZE:-$(command -v llvm-size || echo "/c/Program Files/LLVM/bin/llvm-size")}
K=oracle/FreeRTOS-Kernel
RV=$K/portable/GCC/RISC-V
CDIR=bench/kernel-ram/c

[ -d "$K" ] || { echo "no oracle checkout at $K -- run 'kairos fetch' first"; exit 1; }

fail=0
check() {
    if [ "$2" -eq "$3" ]; then
        printf '      ok    %s (%s)\n' "$1" "$2"
    else
        printf '      FAIL  %s: got %s, expected %s\n' "$1" "$2" "$3"
        fail=$((fail + 1))
    fi
}

# -------------------------------------------------------------- the C arm --
echo "compiling the C kernel for rv32imac -Os (oracle checkout, unmodified)"
CFLAGS="-target riscv32-unknown-elf -march=rv32imac -mabi=ilp32 -Os"
CFLAGS="$CFLAGS -ffunction-sections -fdata-sections"
INCS="-I$CDIR/stub -I$CDIR -I$K/include -I$RV"
INCS="$INCS -I$RV/chip_specific_extensions/RV32I_CLINT_no_extensions"

c_text=0
c_bss=0
for f in tasks queue list timers event_groups; do
    "$CLANG" $CFLAGS $INCS -include "$CDIR/FreeRTOSConfig.h" -c "$K/$f.c" -o "$BUILD/c_$f.o"
    set -- $("$SIZE" "$BUILD/c_$f.o" | tail -1)
    printf '  %-16s text %6d   bss %5d\n' "$f.c" "$1" "$3"
    c_text=$((c_text + $1))
    c_bss=$((c_bss + $3))
done
printf '  %-16s text %6d   bss %5d\n' "= kernel" "$c_text" "$c_bss"

# Struct sizes come from clang's own record layouts rather than a header read
# by eye: TCB_t and Queue_t are private to their translation units.
"$CLANG" $CFLAGS $INCS -include "$CDIR/FreeRTOSConfig.h" \
    -Xclang -fdump-record-layouts -fsyntax-only "$K/tasks.c" > "$BUILD/layouts.txt" 2>&1
for f in queue timers event_groups; do
    "$CLANG" $CFLAGS $INCS -include "$CDIR/FreeRTOSConfig.h" \
        -Xclang -fdump-record-layouts -fsyntax-only "$K/$f.c" >> "$BUILD/layouts.txt" 2>&1
done
layout() {
    awk -v want="$1" '/\| struct /{n=$NF}
        /\[sizeof=/{ if(n==want){ match($0,/sizeof=[0-9]+/);
            print substr($0,RSTART+7,RLENGTH-7); exit } }' "$BUILD/layouts.txt"
}
c_tcb=$(layout tskTaskControlBlock)
c_queue=$(layout QueueDefinition)
c_timer=$(layout tmrTimerControl)
c_group=$(layout EventGroupDef_t)
c_stack=$(grep -oE 'configMINIMAL_STACK_SIZE +[0-9]+' "$CDIR/FreeRTOSConfig.h" | grep -oE '[0-9]+$')
c_stack_bytes=$((c_stack * 4))
c_task_total=$((c_tcb + c_stack_bytes))

# ----------------------------------------------------------- the Rust arm --
echo
echo "building the Kairos probe for riscv32imac-unknown-none-elf"
( cd bench/kernel-ram/rs && cargo build --release -q --target riscv32imac-unknown-none-elf )
LIB=$(ls bench/kernel-ram/rs/target/riscv32imac-unknown-none-elf/release/*.a | head -1)
# `llvm-nm --radix=d` zero-pads, and the shell reads a leading zero as
# octal -- so 00007960 is an error, not 7960. awk's `+0` drops the
# padding by making it a number.
sym() { "$NM" -S --radix=d "$LIB" | awk -v s="$1" '$4 == s {print $2+0; exit}'; }

base=$(sym KAIROS_RAM_BASE)
per_task=$((   ($(sym KAIROS_RAM_TASKS_16)   - base) / 8 ))
per_queue=$((  ($(sym KAIROS_RAM_QUEUES_16)  - base) / 8 ))
per_slot=$((   ($(sym KAIROS_RAM_SLOTS_128)  - base) / 64 ))
per_buffer=$(( ($(sym KAIROS_RAM_BUFFERS_8)  - base) / 4 ))
per_byte=$((   ($(sym KAIROS_RAM_BYTES_2048) - base) / 1024 ))
per_timer=$((  ($(sym KAIROS_RAM_TIMERS_32)  - base) / 16 ))
per_group=$((  ($(sym KAIROS_RAM_GROUPS_4)   - base) / 2 ))
corpus=$(sym KAIROS_RAM_CORPUS)
ratio=$(awk -v a="$c_task_total" -v b="$per_task" 'BEGIN{printf "%.1fx", a/b}')

# ------------------------------------------------------------------ report --
echo
echo "Kairos per-unit cost, each a slope between two measured geometries:"
printf '  %-22s %6s   %s\n' dimension bytes note
printf '  %-22s %6d   %s\n' "a task"          "$per_task"   "TCB + its two list items, and NO STACK"
printf '  %-22s %6d\n'      "a queue"         "$per_queue"
printf '  %-22s %6d\n'      "a timer"         "$per_timer"
printf '  %-22s %6d\n'      "an event group"  "$per_group"
printf '  %-22s %6d\n'      "a stream buffer" "$per_buffer"
printf '  %-22s %6d   %s\n' "a notify slot"   "$per_slot"   "one u64"
printf '  %-22s %6d   %s\n' "a buffer byte"   "$per_byte"   "one byte"

echo
echo "the two floors -- different FUNCTIONS, not one function tuned twice:"
echo
printf '  C FreeRTOS   static   %d B, and it does NOT move when a task is added\n' "$c_bss"
printf '               per task %d B TCB + %d B stack + a heap header\n' "$c_tcb" "$c_stack_bytes"
printf '                        = %d B minimum, from the heap, on demand\n' "$c_task_total"
printf '               per queue %d B + storage, timer %d B, group %d B\n' \
       "$c_queue" "$c_timer" "$c_group"
echo
printf '  Kairos       static   %d B of arenas at 8 tasks / 8 queues / 16 timers\n' "$base"
printf '               per task %d B, and that is the WHOLE cost\n' "$per_task"
echo "               heap     0 B. There is no on-demand allocation to size."
echo
echo "  The gap is the stack. A blocking Kairos call keeps its locals in the"
echo "  TCB (the WaitFrame), so a task can be suspended mid-call without a"
echo "  stack of its own -- the same design fact that let the corpus"
printf '  run on four architectures with no context-switch port. At %d B of\n' "$c_stack_bytes"
printf '  stack a C task costs %d B against our %d B: %s.\n' \
       "$c_task_total" "$per_task" "$ratio"
echo
echo "  The honest counterweight: this is the KERNEL cost, and an application"
echo "  still needs somewhere for its own state. A Kairos task holds it in a"
echo "  struct sized to what it actually uses; a C task holds it on a stack"
echo "  reserved for the worst case. The saving is real, but it comes from"
echo "  EXACT SIZING -- not from the state ceasing to exist. A task whose"
echo "  state genuinely needs 512 B pays 512 B in either kernel."
echo
printf '  the corpus geometry (24 tasks, 12 queues, 32 timers) is %d B, all static\n' "$corpus"

echo
echo "identity checks -- slopes that are true by construction:"
check "a notification slot is one u64" "$per_slot" 8
check "a stream-buffer byte is one byte" "$per_byte" 1
check "the C TCB still carries its two list items" "$c_tcb" 84

echo
if [ "$fail" -eq 0 ]; then
    echo "RESULT: PASS"
else
    echo "RESULT: FAIL -- $fail check(s) failed"
    exit 1
fi
