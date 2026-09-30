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
delta=$((rs_kernel - c_text))
if [ "$delta" -lt 0 ]; then
    printf '  Kairos costs %d bytes LESS flash for the same operation set.\n' \
           $((c_text - rs_kernel))
else
    printf '  Kairos costs %d bytes more flash for the same operation set.\n' "$delta"
fi
echo

echo "where the C arm's bytes are:"
awk '$0 ~ /\.o:\(\.text/ { sz = strtonum("0x" $3); n = $0
        sub(/.*\//, "", n); sub(/\.o:.*/, ".c", n); t[n] += sz }
     END { for (k in t) printf "  %-18s %6d\n", k, t[k] }' "$BUILD/c_kernel.map" | sort -k2 -nr
echo

echo "the tick-width difference, now PRICED instead of asserted:"
echo "  Kairos uses a u64 tick; the FreeRTOS RISC-V port types TickType_t as"
echo "  portUBASE_TYPE (portmacro.h:68), so on rv32 it is 32-bit and"
echo "  configTICK_TYPE_WIDTH_IN_BITS is not honoured at all. Setting it to 64"
echo "  was tried: the type stayed 32-bit and clang reported a constant"
echo "  truncating to 0 -- a broken build, not a matched one."
echo
echo "  This text used to say part of the gap was a 64-bit tick. Nobody had"
echo "  measured that. Building this arm with Tick = Bits32 reads 13,288"
echo "  against 13,318: the u64 tick costs THIRTY BYTES of .text, not a part"
printf '  of any gap -- its helpers live in the %d bytes of compiler_builtins\n' "$rs_builtins"
echo "  already excluded from BOTH arms. We keep it: it never wraps, and it"
echo "  is very nearly free."
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
# 14,520 -> 13,986 on 2026-09-24: `prvInitialiseMutex`s give stopped going
# through the general `queue_send_generic`. See `Kernel::prime_mutex`.
#
# ★ 13,318 -> 25,762 the same day, and THAT is the number that was wrong, not
# this one. The probe passed COMPILE-TIME CONSTANTS where the C arm passes
# nothing it can fold:
#
#   * every handle was `Default::default()`, which is index 0, generation 0.
#     `Arena::resolve` tests `slot.generation != handle.generation ||
#     !live(slot.generation)`, and with a constant generation of 0 that reads
#     `g != 0 || g is even` -- TRUE for every g, because 0 is even. LLVM proved
#     the resolve always fails and deleted the whole operation.
#     `kairos_stream_buffer_send` compiled to TWO BYTES, and the linker folded
#     it with `..._receive` because both had become the same early return.
#     Worth 13,588 bytes.
#   * every block time was a literal 0, folding the blocking halves: 1,340 B.
#   * `NotifyAction`, `notify_take`s clear flag and `event_group_wait_bits`
#     two bools were literals, folding arms the C compiles: 206 B.
#
# The C arm folds none of it: `-u xQueueReceive` roots the real function with
# every parameter unknown. The two arms were not doing the same work.
#
# `rs/src/lib.rs` records that the FIRST version of this probe called everything
# from one function and "13,314 bytes were attributed to the probe". That is
# this 13,588 -- splitting into one entry point per operation is what turned the
# handles into constants. So the row never measured the real bodies, and every
# figure it published -- 2.21x, 1.22x, 1.16x, 1.04x, 1.00x, 0.96x -- was taken
# with 23 operations folded to an error return. **The 2.21x it once reported was
# approximately RIGHT**, and the session that called it stale was wrong.
#
# Every argument that gates control flow now arrives through the entry points
# own parameters, where nothing can fold it. 25,762 includes two real wins made
# the same day: -1,704 B from `Name::new` (see `name.rs`) and -986 B from the
# A4 split on `queue_send_generic`.
# 22,264 -> 22,058 on 2026-09-24, from two REPRESENTATION fixes rather than any
# design change. Both were found by attributing single opcodes to source lines,
# which needed `debug = 1` on this profile to get line tables at all -- the
# first attempt attributed 1 of 49 `mul` and 0 of 78 `srli`, which reads as
# "the opcodes are in core" and actually meant "this profile emits no debug
# info". That build is SCRATCH: debug info perturbs codegen, so it is never the
# one this pin is taken from.
#
#   * `Handle` was two `u16`s, so four bytes, so rv32 passed it in ONE register
#     and every use extracted the halves -- `slli 16`/`srli 16`, because
#     rv32imac has no `zext.h` and `andi` cannot hold a 0xFFFF immediate.
#     `handle.rs:143` alone held 41 of the 78 `srli`. Word-wide fields are two
#     registers and no extraction.
#   * `Arena::resolve` tested the generation PARITY on every call: 118 copies of
#     one `andi` between `resolve` and `resolve_mut`, half the whole `andi`
#     count. A legitimately issued handle is always ODD and the only even one is
#     NULL's zero, so the test belongs at `Handle::from_raw` -- the one boundary
#     a forged handle enters through. The use-after-free defence is INTACT: a
#     forged even generation normalises to NULL and gets the same
#     `InvalidHandle` the arena would have answered. Slots now start at
#     generation 2 and `remove` skips zero, so no slot can carry the null
#     generation and NULL can never match one.
#
# Measured: `andi` 236 -> 133, `srli` 130 -> 78, `mv` 1,027 -> 978, `slli`
# 266 -> 237. `mul` went 32 -> 58 -- a REGRESSION, because widening `Handle`
# changed `size_of::<Slot<T>>()`. It is unrepaid and is the next thing owed.
# 22,064 measured WITHOUT debug info. The attribution instrument needs
# `debug = 1` for line tables and that perturbs `.text` by 6 bytes (22,058 with,
# 22,064 without) -- enough to fail this check, so the attribution build is
# scratch and never the one a pin is taken from. The OPCODE counts below are
# identical either way: debug info moves layout, not the instruction mix.
# 19,132 over the SAME 46 operations the C arm forces, plus the one
# (`check_terminated`) whose C counterpart the forced set reaches through
# `prvIdleTask`. It was 19,490 over FIFTY-FOUR until 2026-09-25: this cell
# exported seven entry points the C arm links no symbol for, because C spells
# them as MACROS over already-forced generics -- `xSemaphoreGive` over
# `xQueueGenericSend`, `xSemaphoreCreateBinary` over `xQueueGenericCreate`,
# `xQueueOverwrite` over `xQueueGenericSend`, `xSemaphoreCreateRecursiveMutex`
# over `xQueueCreateMutex` -- and because `xTimerGenericCommandFromTask` is ONE
# symbol where we exported four (`timer_start`/`stop`/`reset`/`change_period`).
# Worth 358 bytes, charged to us for operations C got for nothing.
# 19,790 is the POST-INCIDENT baseline (2026-09-28). It was 19,244 before a
# non-atomic write truncated `kernel.rs` to zero bytes; the file was rebuilt from
# the newest backup plus the documented session record and now passes
# `conform --all` 26/26 and 102/102 kernel tests, but roughly 546 bytes of
# earlier kernel.rs work has no surviving record and is NOT in this number.
# See docs/LEDGER.md, 2026-09-28.
#
# 19,788 (2026-09-28, -2 B): `switch_context`'s ready-list walk folded its
# `Err` arm into its empty arm and proved `top < MAX_PRIORITIES` once before
# the loop instead. The fold stops the walk carrying a packed
# `Result<Option<ItemId>>` across the back edge, and the up-front proof lets
# `next_round_robin`'s own bounds check fold away -- so the change is smaller
# on flash AND -969,669 Ir on `bench/kernel-ir`, which is not the usual
# direction for this pair. `andi` +2 is the new compare; `mv` -1 is one less
# argument shuffled.
#
# 19,786 (2026-09-28, -2 B): `reset_next_task_unblock_time` asks the emptiness
# question directly, matching the C's `portMAX_DELAY` for an empty delayed list
# where `head_value` had been returning the end marker's `u64::MAX`. That makes
# `next_unblock_time <= MAX_DELAY` an invariant, so `increment_tick`'s comparison
# against it collapses from a 64-bit compare to a 32-bit one -- `bench/tick-work`
# tick_idle and tick_delayed 14 -> 9 against the C's 15. The function is `#[cold]
# #[inline(never)]` because `switch_delayed_lists` is `#[inline(always)]` and in
# line the bigger body cost the tick row 14 -> 34.
#
# 19,778 (2026-09-28, -8 B): `hand_over` takes only the INCOMING handle and reads
# the outgoing one from `self.current` itself, and `switch_context` compares
# INDICES rather than whole handles. Together those delete the load of
# `current`'s generation word; either alone leaves it live, which is why the
# index compare measured 0 on its own (A2: price the set). rv32
# `switch_select` 48 -> 47 and `block_cycle` 993 -> 989.
#
# 19,752 (2026-09-28, -26 B): `check_for_timeout` returns `Option<NonZeroU64>`
# instead of `Option<u64>`. `Option<u64>` has no niche -- sixteen bytes, two
# registers -- and `Some(0)` is unreachable on every arm, so the niche is free.
# Worth -270,175 Ir on the host, -4 on rv32 `block_cycle`, and these 26 bytes.
# `rusty-compiler-leverage` B5: change the REPRESENTATION, do not out-compute
# LLVM.
# 19,740 (2026-09-28, -12 B): `queue_take_timed_out` stopped taking `caller`.
# It is `self.current` at both call sites -- one reads it two lines above, the
# other is guarded by `if self.current != caller` -- so reading it in the callee
# is the same value for one fewer argument pair. Worth -27,967 Ir, these 12
# bytes, and 10 of the `mv` count below, with every rv32 row IDENTICAL.
#
# Doing the SAME to `queue_take_blocking` as well measured -136,107 Ir and -22 B
# and was REFUSED: it cost +1 on `recv_empty`, `send_full` and `peek_ok` and +4
# on `queue_roundtrip` against -2 on `block_cycle`. rv32 has EIGHT argument
# registers, so `peek` was never in the seventh slot there and dropping `caller`
# only adds two loads -- the win is x86-64's six-register convention, and those
# four rows are firmware paths, not SimPort bookkeeping.
# 19,762 (2026-09-28, +22 B): a TRADE, taken deliberately and priced on all
# four instruments. `place_on_event_list` proves `self.current.index() < TASKS`
# before building the event item, which folds BOTH the `saturating_add` in
# `event_item` and the node-array bound check inside `insert_keeping_value`.
# Worth -96,574 Ir (`queue_take_blocking` -24,898, `queue_send_blocking`
# -71,676, nothing positive) and rv32 `block_cycle` 977 -> 974, with every other
# rv32 row identical. These 22 bytes are that check duplicated at each inline
# site -- `#[inline]` and `#[inline(always)]` measure the same, and WITHOUT the
# hint LLVM outlines the function and the whole thing becomes +154,439.
#
# 22 bytes for 3 instructions on the blocking queue path, a host win, and a
# corrupt `self.current` reporting a stall instead of relying on a downstream
# range check. Compare the trade DECLINED at -192,102 Ir for +204 B on
# `unlock_queue`: the rate is what decides these, not the sign.
# 19,736 (2026-09-28, -26 B): the same bound proof as above, one frame further
# down, in `add_current_task_to_delayed_list` -- which by its own note is the
# single largest consumer of the blocking workload and derives FOUR
# bound-checked accesses from an index it read out of memory. Worth -262,232 Ir
# in that one row with nothing positive, rv32 `block_cycle` 974 -> 968, and
# these 26 bytes with every opcode count below falling too. It pays back the +22
# the event-item proof cost, so the two together are -4 B and -9 on block_cycle.
# 19,746 (2026-09-28, +10 B): a trade. `remove_from_event_list`'s `?` put FIVE
# constants in its entry block -- the Ok/Err tags and both error discriminants --
# before the tests that would use them, and the census says ALL 21,487 BlockQ
# calls pass every one of those tests. Producing the error in a `#[cold]
# #[inline(never)]` callee sinks them: -119,405 Ir, which is 5 x 23,881 calls to
# the instruction, and rv32 `block_cycle` 968 -> 967. These 10 bytes are the new
# out-of-line symbol.
#
# `#[cold]` WITHOUT `#[inline(never)]` measured +0 Ir for -2 B: LLVM inlines it
# back and re-materialises the constants. The hint reweights the branch; only
# moving the code sinks the setup.
# 19,738 (2026-09-30, -8 B): `next_round_robin` no longer tests whether the item
# it steps to is a marker -- the wrap above it and the `len != 0` test make that
# unreachable, so it was `li` + `bltu` on every round-robin for nothing. rv32
# switch_select 47 -> 45, block_cycle 932 -> 924, kernel-ir -571,256 in one row.
check "Kairos kernel + port"   "$rs_kernel" 19738

# ---- the opcode counts, PINNED ---------------------------------------------
#
# `.text` is one number and it hides direction. The class census that opened
# the 1.6x investigation found the gap is instruction COUNT at equal encoding
# density, and that the worst ratios are in opcodes that are not real work:
# `mv` (register pressure), `mul` (non-power-of-two indexing), `srli`/`slli`
# (u16 half-extraction), `andi` (masks).
#
# Those four were UNGATED, and it showed: widening `Handle` took `srli` 130 to
# 78 and `andi` 236 to 133 while quietly taking `mul` 32 to 58, because it
# changed `size_of::<Slot<T>>()`. Nothing failed. The regression was found only
# because somebody happened to be counting that hour.
#
# A total can fall while its parts move in opposite directions, so the parts
# are pinned separately. A pin that moves DOWN is still a failure here -- it
# means the number changed and nobody wrote down why.
#
# TWO LABELS WERE WRONG and are corrected above (2026-09-24):
#
# `mv` is NOT register pressure. The attribution says 57% of it is CALL-ARGUMENT
# SETUP: 445 of 829 sit immediately before a `jal`, and in the three functions
# holding the most, 109 of 146 are `saved -> arg` -- a receiver parked in a
# callee-saved register and re-supplied at each call. `queue_take_blocking`
# makes 26 calls and 48 of its 56 `mv` are their arguments. `mv` per call is
# 1.43 against C's 0.67, and 183 of the moves are the single `mv a0, sN` that
# rv32 cannot avoid, because `a0` is caller-saved. The lever is CALL COUNT.
#
# `andi` is not all waste. 19 of the 130 are `ListsOf::at`'s hand-written
# `& (N-1)`, which REPLACES a compare, a branch and a panic edge -- the cheapest
# bounds check the ISA offers, and C's lower count is C doing none. DRIVING THIS
# PIN DOWN CAN MAKE THE BINARY BIGGER. Read the immediates before treating a
# rise as a regression: 0x3f is a bounds mask, 0x1 is a bool, 0x4/0xfb is the
# `F_WAIT` test-and-clear.
#
# And five changes measured `mv` DOWN while costing flash: inlining the
# critical-section pair (-165 `mv`, +1,866 B), `exit_critical` alone (-70,
# +2,054 B), `add_task_to_ready_list` (-9, +212 B), narrowing `Handle` to four
# bytes (+9 -- the wrong way -- and +1,160 B) and rounding the list count.
# The flash total is what arbitrates; an opcode pin alone never does.
OD=${OD:-$(command -v llvm-objdump || echo "/c/Program Files/LLVM/bin/llvm-objdump")}
"$OD" -d --no-show-raw-insn "$BUILD/rs_kernel.elf" 2>/dev/null > "$BUILD/rs_ops.asm"
# `zext.b` IS `andi rd, rs, 0xff` -- encoding `0ff57593` is OP-IMM funct3=111
# with imm 0x0FF -- and llvm-objdump prints the PSEUDO. A census keyed on the
# mnemonic therefore missed 49 of our ANDI instructions and 2 of the C arm's,
# which is 1.6x of the andi count hiding behind a disassembler label. Count the
# pseudo as what it encodes. Same trap for any other alias this ISA prints.
# ...and `compiler_builtins` is EXCLUDED, because the byte total beside these
# counts excludes it (`rs_kernel = rs_text - rs_builtins`, above) and the C arm
# links no `mem*` at all -- zero `memcpy`/`memset`/`memcmp` symbols and zero
# calls to any. Counting them here described different code from the number they
# sit next to: `mv` read 828 for a binary whose pinned size covers 819 of them.
ops() {
    awk -v op="$1" '
        /^[0-9a-f]+ <.*>:$/ {
            ours = ($0 ~ /compiler_builtins|<mem(cpy|set|cmp|move)>/) ? 0 : 1
            next
        }
        ours && /^ +[0-9a-f]+:/ {
            m = $2
            if (m == "zext.b") m = "andi"
            if (m == op) n++
        }
        END { print n+0 }' "$BUILD/rs_ops.asm"
}
echo
echo "opcode counts -- the four the 1.6x investigation named:"
# 775 (2026-09-28, -10): the `caller` argument removed from
# `queue_take_timed_out`, above. This is the count the 1.6x investigation named
# as the tell for argument marshalling, so a signature change should move it.
# 770 (2026-09-28, -5), 78 srli / 259 slli / 166 andi (+2 / +2 / +1): the
# event-item bound proof above. The u16 extraction counts rise because the
# folded check leaves the index arithmetic in narrower registers.
# 769 / 77 / 258 / 165 (2026-09-28): the delayed-list bound proof above. Every
# one of the four fell, which is the signature of a folded check rather than a
# reshuffle -- the event-item proof raised three of them.
check "mv   (call-argument setup)" "$(ops mv)"   769
check "mul  (non-p2 indexing)"     "$(ops mul)"    0
check "srli (u16 extraction)"      "$(ops srli)"  77
check "slli (u16 extraction)"      "$(ops slli)" 258
check "andi (incl. zext.b)"        "$(ops andi)" 165

echo
if [ "$fail" -eq 0 ]; then
    echo "RESULT: PASS -- Kairos is $ratio the C kernel's flash for the operation"
    echo "        set the corpus proves byte-identical."
else
    echo "RESULT: FAIL -- $fail check(s) failed"
    exit 1
fi
