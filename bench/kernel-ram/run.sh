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

# ---- the per-unit slopes, taken ONE unit at a time --------------------------
#
# These were taken across a DOUBLING until 2026-09-24, and `list_slots_for`
# ends in `next_power_of_two()`: 8 -> 16 tasks and 16 -> 32 timers each cross a
# 64 -> 128 slot step, and dividing by the unit count charged a share of that
# one-off step to every unit. It made a timer read 104 bytes when its marginal
# cost is 40. The ledger diagnosed exactly this in September and published the
# corrected 40 -- and left the bench computing 104, so the number CI checked
# and the number the scorecard quoted were different numbers.
#
# A one-unit neighbour has no step in it PROVIDED the slot count did not move,
# and `slope_guard` below proves that rather than assuming it.
per_task=$((   $(sym KAIROS_RAM_TASKS_9)   - base ))
per_queue=$((  $(sym KAIROS_RAM_QUEUES_9)  - base ))
per_buffer=$(( $(sym KAIROS_RAM_BUFFERS_5) - base ))
per_timer=$((  $(sym KAIROS_RAM_TIMERS_17) - base ))
per_group=$((  $(sym KAIROS_RAM_GROUPS_3)  - base ))
# Slots and bytes are raw arena dimensions feeding no `next_power_of_two`, so
# their slopes are linear and a wide span is the more accurate measurement.
per_slot=$((   ($(sym KAIROS_RAM_SLOTS_128)  - base) / 64 ))
per_byte=$((   ($(sym KAIROS_RAM_BYTES_2048) - base) / 1024 ))

# The doubling figures, kept so the report can price the step itself.
step_task=$((  ($(sym KAIROS_RAM_TASKS_16)  - base) / 8 ))
step_queue=$(( ($(sym KAIROS_RAM_QUEUES_16) - base) / 8 ))
step_timer=$(( ($(sym KAIROS_RAM_TIMERS_32) - base) / 16 ))
step_group=$(( ($(sym KAIROS_RAM_GROUPS_4)  - base) / 2 ))

slots_base=$(sym KAIROS_SLOTS_BASE)
slope_guard() {
    if [ "$2" -eq "$slots_base" ]; then
        printf '      ok    %s slope has no granularity step in it (%d slots)\n' "$1" "$2"
    else
        printf '      FAIL  %s slope straddles a slot step: %d -> %d\n' "$1" "$slots_base" "$2"
        fail=$((fail + 1))
    fi
}
corpus=$(sym KAIROS_RAM_CORPUS)
ratio=$(awk -v a="$c_task_total" -v b="$per_task" 'BEGIN{printf "%.1fx", a/b}')

# ------------------------------------------------------------------ report --
echo
echo "Kairos per-unit cost, each a slope between two measured geometries:"
printf '  %-22s %6s   %s\n' dimension bytes note
printf '  %-22s %6d   %s\n' "a task"          "$per_task"   "the TCB slot (88) + the per-task side arrays (48)"
printf '  %-22s %6d\n'      "a queue"         "$per_queue"
printf '  %-22s %6d\n'      "a timer"         "$per_timer"
printf '  %-22s %6d\n'      "an event group"  "$per_group"
printf '  %-22s %6d\n'      "a stream buffer" "$per_buffer"
printf '  %-22s %6d   %s\n' "a notify slot"   "$per_slot"   "one u64"
printf '  %-22s %6d   %s\n' "a buffer byte"   "$per_byte"   "one byte"
echo
echo "  what the OLD method reported, and what it was charging you for:"
echo "  each divided a DOUBLING by the unit count, so the one-off"
echo "  next_power_of_two step in the list arena landed on every unit."
printf '  %-22s %8s %9s %10s\n' dimension marginal doubling inflation
printf '  %-22s %8d %9d %9d%%\n' "a task"         "$per_task"  "$step_task" \
       $(( (step_task - per_task) * 100 / per_task ))
printf '  %-22s %8d %9d %9d%%\n' "a queue"        "$per_queue" "$step_queue" \
       $(( (step_queue - per_queue) * 100 / per_queue ))
printf '  %-22s %8d %9d %9d%%\n' "a timer"        "$per_timer" "$step_timer" \
       $(( (step_timer - per_timer) * 100 / per_timer ))
printf '  %-22s %8d %9d %9d%%\n' "an event group" "$per_group" "$step_group" \
       $(( (step_group - per_group) * 100 / per_group ))
echo
echo "  The step is REAL memory -- the list arena genuinely rounds up, and"
echo "  crossing a band genuinely costs it. What it is not is a PER-UNIT"
echo "  cost: it is paid once at the boundary, by whichever unit happens to"
echo "  cross it, and charging a share of it to every unit overstates the"
echo "  marginal cost of the next one. Both numbers are above so that"
echo "  sizing a part uses the marginal and budgeting a geometry uses"
echo "  FOOTPRINT at that geometry, which is exact and needs no slope."

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
echo "slope admissibility -- a one-unit neighbour with the SAME slot count:"
slope_guard "task"        "$(sym KAIROS_SLOTS_TASKS_9)"
slope_guard "queue"       "$(sym KAIROS_SLOTS_QUEUES_9)"
slope_guard "timer"       "$(sym KAIROS_SLOTS_TIMERS_17)"
slope_guard "event group" "$(sym KAIROS_SLOTS_GROUPS_3)"
echo
echo "identity checks -- slopes that are true by construction:"
# ---- the sizes themselves, which nothing used to check -----------------------
#
# `STRIDE_TCB`, `STRIDE_QUEUE` and the two TCB-footprint probes were computed by
# the probe crate and READ BY NOTHING. So the row's headline number -- the
# per-task slope -- was reported by a `grep` and guarded by no pin, and on
# 2026-09-25 it read 184 B against a 176 B recorded earlier the same day with no
# way to tell which was right or what had moved. Five pins guard the flash row's
# opcodes; this row had none of its own sizes.
#
# The stride pin is the one that matters structurally: `Tcb::_stride_pad` exists
# solely to keep it a POWER OF TWO so `index * stride` is one `slli` and the
# kernel contains no multiply. If it ever moves off 128, `mul` comes back and the
# flash row moves with it -- and that is a fact about this bench's arena, which
# the flash bench cannot see.
stride_tcb=$(sym STRIDE_TCB)
stride_queue=$(sym STRIDE_QUEUE)
check "the TCB slot stride is a power of two" "$stride_tcb"   128
check "the queue slot stride"                 "$stride_queue"  48
check "one more task costs one more slot"           "$(( $(sym KAIROS_TP_TCBS) - $(sym KAIROS_TB_TCBS) ))" "$stride_tcb"
# 176 is the POST-INCIDENT value (2026-09-28): 8 bytes of per-task side-array
# work did not survive the kernel.rs truncation. See docs/LEDGER.md.
check "per-task RAM, the row's headline"      "$per_task"     176

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
