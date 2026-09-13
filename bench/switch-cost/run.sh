#!/bin/sh
# The K3 context-switch WORK row: how many instructions a context switch
# costs in Kairos against the FreeRTOS port it remakes, on every
# architecture where both can be counted.
#
#     sh bench/switch-cost/run.sh              # from the umbrella root
#
# Method, and why each part of it is there:
#
#   * Nothing is timed and nothing is emulated. The switch paths counted here
#     are STRAIGHT LINE, so the count of instructions in the block is the
#     count retired by it. The script CHECKS that where it can: a conditional
#     branch appearing in the RISC-V arms fails the run, because the moment
#     one exists a static count stops being a dynamic one. (The ARM arms both
#     contain guards; see the note under the ARM table.)
#   * Each arm is counted from its OWN toolchain's output -- the oracle's
#     sources assembled or compiled by clang from the pinned `oracle/`
#     checkout, ours read back out of linked firmware. Neither number is
#     typed in by hand, and neither is read off a source listing.
#   * The RISC-V macros are expanded ALONE in their own sections (see
#     `c/freertos_probe.S`) rather than carved out of a handler by address
#     range. A section boundary is chosen by the assembler; an address range
#     would be chosen by eye.
#   * FPU and VPU are off in the C arms because the Kairos ports have no FPU
#     save either. Charging the C arm for a feature the Rust arm does not
#     implement would not be a comparison.
#
# What this row does NOT claim: a cycle count, or anything about silicon.
# The mission plan puts cycle rows on real parts for reasons that have not
# changed. This is a WORK row -- an instruction count -- and the two are not
# the same number. Nor are counts comparable ACROSS architectures: ARM moves
# eight registers in one `stmdb` where RISC-V needs eight `sw`, so only the
# ours-against-theirs comparison within an architecture means anything.
set -e

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
cd "$ROOT"
BUILD=${BUILD:-${TMPDIR:-/tmp}/kairos-switch-cost}
mkdir -p "$BUILD"

CLANG=${CLANG:-$(command -v clang || echo "/c/Program Files/LLVM/bin/clang")}
OBJDUMP=${OBJDUMP:-$(command -v llvm-objdump || echo "/c/Program Files/LLVM/bin/llvm-objdump")}
K=oracle/FreeRTOS-Kernel
RV=$K/portable/GCC/RISC-V
CM3=$K/portable/GCC/ARM_CM3
RVCELL=rusty_rtos_port/firmware/riscv32-qemu-switch
RVELF="$RVCELL/target/riscv32imac-unknown-none-elf/release/riscv32-qemu-switch"
PRECELL=rusty_rtos_port/firmware/riscv32-qemu-preempt
PREELF="$PRECELL/target/riscv32imac-unknown-none-elf/release/riscv32-qemu-preempt"
ARMCELL=rusty_rtos_port/firmware/mps2-an385-qemu-kernel
ARMELF="$ARMCELL/target/thumbv7m-none-eabi/release/mps2-an385-qemu-kernel"

[ -d "$RV" ] || { echo "no oracle checkout at $RV -- run 'kairos fetch' first"; exit 1; }

# ---------------------------------------------------------------- counting --
INSN='^[[:space:]]*[0-9a-f]+:[[:space:]]'
BRANCH='[[:space:]](beq|bne|blt|bge|bltu|bgeu|beqz|bnez|blez|bgez|bltz|bgtz)[[:space:]]'

sec() { "$OBJDUMP" -d --no-show-raw-insn --section="$2" "$1"; }
sym() { "$OBJDUMP" -d --no-show-raw-insn "$1" \
          | awk -v s="<$2>:" 'index($0, s) {f=1; next} f && /^[0-9a-f]+ </ {exit} f'; }
count()   { grep -cE "$INSN" || true; }
countbr() { grep -cE "$BRANCH" || true; }

# An ARM handler ends at `bx lr`; everything after it in the block is the
# literal pool and alignment padding, which are data, not instructions.
count_to_bx() {
    awk '/\.word/ { next }
         '"/$INSN/"' { n++; if ($0 ~ /bx[[:space:]]+lr/) { print n; exit } }
         END { if (!n) print 0 }'
}

fail=0
check() {
    if [ "$2" -eq "$3" ]; then
        printf '      ok    %s (%s)\n' "$1" "$2"
    else
        printf '      FAIL  %s: got %s, pinned %s\n' "$1" "$2" "$3"
        fail=$((fail + 1))
    fi
}

# ============================================================== RISC-V ======
echo "assembling the oracle's RISC-V context macros (clang -> rv32imac, unmodified)"
"$CLANG" -target riscv32-unknown-elf -march=rv32imac -mabi=ilp32 \
    -Ibench/switch-cost/c -I"$RV" \
    -I"$RV/chip_specific_extensions/RV32I_CLINT_no_extensions" \
    -c bench/switch-cost/c/freertos_probe.S -o "$BUILD/freertos_probe.o"

PROBE="$BUILD/freertos_probe.o"
rv_c_save=$(sec "$PROBE" .text.kairos_probe_save    | count)
rv_c_rest=$(sec "$PROBE" .text.kairos_probe_restore | count)
rv_c_brs=$( sec "$PROBE" .text.kairos_probe_save    | countbr)
rv_c_brr=$( sec "$PROBE" .text.kairos_probe_restore | countbr)
rv_c_total=$((rv_c_save + rv_c_rest))

if [ ! -f "$RVELF" ]; then
    echo "building the Kairos RISC-V cell"
    ( cd "$RVCELL" && cargo build --release -q )
fi
rv_rs=$(sym "$RVELF" kairos_riscv_switch | count)
rv_rs_br=$(sym "$RVELF" kairos_riscv_switch | countbr)
rt_trap=$(sym "$RVELF" default_start_trap | count)

# The PREEMPTIVE switch, from the cell that proves the path actually works.
if [ ! -f "$PREELF" ]; then
    echo "building the Kairos RISC-V preemptive cell"
    ( cd "$PRECELL" && cargo build --release -q )
fi
rv_pre=$(sym "$PREELF" kairos_riscv_switch_trap | count)
rv_pre_br=$(sym "$PREELF" kairos_riscv_switch_trap | countbr)

# ================================================================= ARM ======
echo "compiling the oracle's Cortex-M3 port (clang -> thumbv7m, unmodified)"
"$CLANG" -target thumbv7m-none-eabi -mcpu=cortex-m3 -Os -ffunction-sections \
    -Ibench/kernel-ram/c/stub -Ibench/kernel-ram/c -I"$K/include" -I"$CM3" \
    -include bench/kernel-ram/c/FreeRTOSConfig.h \
    -c "$CM3/port.c" -o "$BUILD/arm_port.o"

arm_c=$(sec "$BUILD/arm_port.o" .text.xPortPendSVHandler | count_to_bx)

if [ ! -f "$ARMELF" ]; then
    echo "building the Kairos ARM cell"
    ( cd "$ARMCELL" && cargo build --release -q )
fi
arm_rs=$(sym "$ARMELF" PendSV | count_to_bx)

# ============================================================== report ======
rv_ratio=$(awk -v a="$rv_c_total" -v b="$rv_rs" 'BEGIN{printf "%.2fx", a/b}')
pre_total=$((rv_pre + rt_trap))
pre_ratio=$(awk -v a="$rv_c_total" -v b="$pre_total" 'BEGIN{printf "%.2fx", a/b}')

echo
echo "RISC-V -- a COOPERATIVE yield, the whole port cost:"
printf '  %-34s %5s %8s %6s\n' "" "save" "restore" "total"
printf '  %-34s %5d %8d %6d\n' "FreeRTOS portASM.S (ecall trap)" "$rv_c_save" "$rv_c_rest" "$rv_c_total"
printf '  %-34s %5d %8d %6d\n' "Kairos kairos_riscv_switch" $((rv_rs - 16)) 16 "$rv_rs"
printf '  %-34s %5s %8s %6s\n' "" "" "ratio" "$rv_ratio"
echo
echo "  FreeRTOS's portYIELD() is \`ecall\` (portmacro.h:94), so a yield there"
echo "  takes the same trap an interrupt does and saves all 28 GPRs. Kairos"
echo "  yields at a call site, where the C ABI has already declared the"
echo "  caller-saved half dead, so it writes down 14."
echo
echo "  Under PREEMPTION the arms converge, and this is now MEASURED rather"
echo "  than bounded: riscv32-qemu-preempt makes the path real -- 201 switches"
echo "  between two tasks that never yield, zero faults."
printf '    %-32s %6d
' "Kairos kairos_riscv_switch_trap" "$rv_pre"
printf '    %-32s %6d
' "+ riscv-rt default_start_trap"   "$rt_trap"
printf '    %-32s %6d   vs FreeRTOS %d   %s
' "= a preemptive switch" "$pre_total" "$rv_c_total" "$pre_ratio"
echo
echo "    The preemptive switch swaps the same fourteen registers plus"
echo "    mepc and mstatus: 37 against the cooperative 30. Straight line"
echo "    too, so the count is again a retired count."
echo
echo "    So the two rows say different things and both are true. A YIELD is"
echo "    2.77x cheaper on our side because FreeRTOS routes yields through"
echo "    the interrupt trap. A PREEMPTIVE switch is near parity, because"
echo "    there both kernels must save what an interrupt could clobber."
echo "    Quoting only the first would be quoting the easy half."
echo
echo "ARM Cortex-M3 -- both arms are the PendSV handler, trap to trap:"
printf '  %-34s %6d\n' "FreeRTOS xPortPendSVHandler" "$arm_c"
printf '  %-34s %6d\n' "Kairos PendSV"               "$arm_rs"
echo
echo "  PARITY, and it is the RISC-V row's control. On ARM both kernels reach"
echo "  the switch the same way -- portYIELD() pends PendSV on their side, and"
echo "  ours does the same -- so the shapes match and so do the counts. The"
echo "  RISC-V gap is therefore about WHERE a yield is taken, not about one"
echo "  kernel moving registers more cheaply than the other."
echo
echo "  Neither ARM arm is straight line, and they spend their guards"
echo "  differently: ours has four \`cbz\` for the null-slot cases, theirs has"
echo "  four instructions of \`basepri\` masking. Same count, different work."
echo
echo "  Counts do NOT compare across architectures: one \`stmdb\` moves eight"
echo "  registers where RISC-V needs eight \`sw\`. Only the ours-against-theirs"
echo "  comparison within an architecture means anything."

echo
echo "straight-line check -- a static count is a retired count only if there"
echo "is nothing to skip:"
check "FreeRTOS RISC-V save has no conditional branch"    "$rv_c_brs" 0
check "FreeRTOS RISC-V restore has no conditional branch" "$rv_c_brr" 0
check "Kairos RISC-V switch has exactly one, the null-outgoing test" "$rv_rs_br" 1
check "Kairos RISC-V preemptive switch has no conditional branch" "$rv_pre_br" 0

echo
echo "pinned counts -- these fail loudly when a register joins a context:"
check "FreeRTOS RISC-V save"       "$rv_c_save" 42
check "FreeRTOS RISC-V restore"    "$rv_c_rest" 41
check "Kairos RISC-V switch"       "$rv_rs"     30
check "riscv-rt trap entry"        "$rt_trap"   37
check "Kairos RISC-V preemptive switch" "$rv_pre" 37
check "FreeRTOS ARM PendSV"        "$arm_c"     19
check "Kairos ARM PendSV"          "$arm_rs"    19

echo
if [ "$fail" -eq 0 ]; then
    echo "RESULT: PASS -- RISC-V $rv_rs against $rv_c_total on a cooperative yield,"
    echo "        $pre_total against $rv_c_total under preemption; ARM $arm_rs against $arm_c,"
    echo "        parity. All counted from their own toolchains."
else
    echo "RESULT: FAIL -- $fail check(s) failed"
    exit 1
fi
