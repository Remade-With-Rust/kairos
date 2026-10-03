#!/bin/sh
# Record the tick-work cell's executed instruction stream on the SHIPPED port
# and list its brackets; `trace.py` then prints any one of them.
#
#     sh bench/tick-work/trace.sh            # record, list the brackets
#     sh bench/tick-work/trace.sh 15         # ... and print bracket 15's path
#
# Same QEMU, same flags as the cell's runner (`-icount shift=0`), plus
# `-d in_asm,exec,nochain`, which logs each translation block once and every
# execution of one in order. The log is ~57 MB and lives in $OUT, not /tmp.
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
CELL=$ROOT/rusty_rtos_kernel/firmware/riscv32-qemu-tick-work
OUT=${OUT:-$ROOT/bench/tick-work/out}
OBJDUMP=${OBJDUMP:-$(ls "$HOME"/.rustup/toolchains/*/lib/rustlib/*/bin/llvm-objdump* 2>/dev/null | head -1)}
mkdir -p "$OUT"
( cd "$CELL" && cargo build --release --features real-port 2>/dev/null ) || { echo "build failed"; exit 1; }
ELF=$CELL/target/riscv32imac-unknown-none-elf/release/riscv32-qemu-tick-work
"$OBJDUMP" -d -C --no-show-raw-insn "$ELF" > "$OUT/trace.dis"
# QEMU is a native Windows binary here: `-D` takes a Windows path.
LOG=$OUT/trace.log
WLOG=$( (cygpath -m "$LOG") 2>/dev/null || echo "$LOG")
rm -f "$LOG"
qemu-system-riscv32 -machine virt -cpu rv32 -nographic \
    -semihosting-config enable=on,target=native -bios none -icount shift=0 \
    -d in_asm,exec,nochain -D "$WLOG" -kernel "$ELF" > "$OUT/trace.out" 2>&1
grep -q "RESULT: PASS" "$OUT/trace.out" || { echo "the cell did not pass"; exit 1; }
P() { (cygpath -m "$1") 2>/dev/null || echo "$1"; }
python3 "$ROOT/bench/tick-work/trace.py" "$(P "$LOG")" "$(P "$OUT/trace.dis")" ${1:-}
