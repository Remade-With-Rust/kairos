#!/bin/sh
# Does tools/no-c-audit.sh actually catch C? A gate that has only ever said PASS
# has not been shown to work, so this PLANTS C, one way per case, in throwaway
# clones, and requires the audit to FAIL and name the culprit -- and requires the
# untouched clones to PASS. The real repos are never modified.
#
#     sh tools/no-c-audit-selftest.sh          # from the Kairos umbrella
#
#   control  nothing planted                                     must PASS
#   P1       evil.c + build.rs inside a published crate          layer 1
#   P2       a crates.io dependency that compiles C (libz-sys)   layer 2
#   P3       a PATH dependency that compiles C                   layer 2
#   P4       a clang-compiled C object linked into a firmware    layer 3
#   P5       a firmware calling the toolchain's GCC-built
#            compiler-rt helper __absvsi2                        layer 3
#
# Exit 0 only if every case behaves. Needs network (P2/P3 fetch `cc`, `libz-sys`).
set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
LLVM=${LLVM:-"/c/Program Files/LLVM/bin"}
[ -x "$LLVM/clang" ] || [ -x "$LLVM/clang.exe" ] || LLVM=$(dirname "$(command -v clang)")
SB=${SANDBOX:-${TMPDIR:-/tmp}/ncst}
# Keep it SHORT. Windows caps paths at 260 characters, and a firmware's host
# build scripts land ~150 characters below the sandbox root; a long root fails
# every firmware build with LNK1104 ("cannot open file"), which reads like a
# planted-C failure and is not one. That cost one wrong diagnosis on 2026-10-01.
case "$(uname -s 2>/dev/null)" in MINGW*|MSYS*|CYGWIN*)
    if [ ${#SB} -gt 60 ]; then
        echo "sandbox path is ${#SB} characters; keep it under 60 on Windows (MAX_PATH): $SB"; exit 2
    fi ;;
esac
winpath() { if command -v cygpath >/dev/null 2>&1; then cygpath -m "$1"; else printf '%s' "$1"; fi; }

KCORE=rusty_rtos_kernel/crates/rusty_rtos_kernel-core
FW=rusty_rtos_port/firmware/riscv32-qemu-preempt
FIRMWARE_ONE="$FW:riscv32-qemu-preempt:riscv32imac-unknown-none-elf:"

echo "sandbox: $SB"
rm -rf "$SB"; mkdir -p "$SB/tools"
cp "$ROOT/tools/no-c-audit.sh" "$SB/tools/"
for r in rusty_rtos_kernel rusty_rtos_port; do
    # --no-hardlinks: --local hardlinks objects, which fails when the sandbox is
    # on a different drive from the repos (F: -> C: on this box).
    git clone -q --no-hardlinks "$ROOT/$r" "$SB/$r" || { echo "clone of $r failed"; exit 2; }
done

reset() {
    for r in rusty_rtos_kernel rusty_rtos_port; do
        git -C "$SB/$r" checkout -q -- . && git -C "$SB/$r" clean -fdq -e target
    done
    rm -rf "$SB/evil-sys" "$SB/evil"
}
add_dep() {   # add_dep <Cargo.toml> <line>: append under the first [dependencies]
    python3 - "$1" "$2" <<'PY'
import io, sys
p, line = sys.argv[1], sys.argv[2]
s = io.open(p, encoding="utf-8", newline="").read()
nl = "\r\n" if "\r\n" in s else "\n"
if "[dependencies]" + nl in s:
    s = s.replace("[dependencies]" + nl, "[dependencies]" + nl + line + nl, 1)
else:
    s = s.rstrip() + nl + nl + "[dependencies]" + nl + line + nl
io.open(p, "w", encoding="utf-8", newline="").write(s)
PY
}
call_in_main() {   # call_in_main <decl> <expr>: declare an extern fn and call it from main
    python3 - "$SB/$FW/src/main.rs" "$1" "$2" <<'PY'
import io, sys
p, decl, call = sys.argv[1:4]
s = io.open(p, encoding="utf-8", newline="").read()
nl = "\r\n" if "\r\n" in s else "\n"
anchor = "fn main() -> ! {"
assert s.count(anchor) == 1, s.count(anchor)
s = s.replace(anchor, anchor + nl +
    "    #[allow(unsafe_code)]" + nl +
    f"    let selftest = unsafe {{ {call} }};" + nl +
    "    core::hint::black_box(selftest);", 1)
s += nl + 'unsafe extern "C" {' + nl + f"    {decl}" + nl + "}" + nl
io.open(p, "w", encoding="utf-8", newline="").write(s)
PY
}
link_object() {    # link_object <obj>: a firmware build script that links it
    printf 'fn main() { println!("cargo:rustc-link-arg=%s"); }\n' "$(winpath "$1")" > "$SB/$FW/build.rs"
}

PASSED=0; FAILED=0
run_case() {       # run_case <name> <expect: pass|regex> <REPOS>
    name=$1; expect=$2; repos=$3
    out="$SB/out.$name"
    (cd "$SB" && REPOS="$repos" FIRMWARE="$FIRMWARE_ONE" sh tools/no-c-audit.sh) > "$out" 2>&1
    code=$?
    if [ "$expect" = pass ]; then
        if [ $code -eq 0 ]; then verdict="ok    PASS as required"; else verdict="BAD   expected PASS, got FAIL"; fi
    elif [ $code -ne 0 ] && grep -qE "$expect" "$out"; then
        verdict="ok    caught: $(grep -m1 -E "$expect" "$out" | sed 's/^ *//' | cut -c1-110)"
    elif [ $code -eq 0 ]; then
        verdict="MISS  audit said PASS -- the planted C went UNDETECTED"
    else
        verdict="BAD   failed, but not on the planted C: $(grep -m1 'FAIL ' "$out" | sed 's/^ *//' | cut -c1-90)"
    fi
    case "$verdict" in ok*) PASSED=$((PASSED + 1)) ;; *) FAILED=$((FAILED + 1)) ;; esac
    printf '  %-8s %s\n' "$name" "$verdict"
}

echo
echo "=== the cases ==="

reset
run_case control pass "rusty_rtos_kernel rusty_rtos_port"

reset
printf 'int evil(void) { return 42; }\n' > "$SB/$KCORE/src/evil.c"
printf 'fn main() {}\n' > "$SB/$KCORE/build.rs"
run_case P1 'FAIL +rusty_rtos_kernel-core ships:.*(evil\.c|build-script)' "rusty_rtos_kernel"

reset
add_dep "$SB/$KCORE/Cargo.toml" 'libz-sys = { version = "1", default-features = false, features = ["static"] }'
run_case P2 'FAIL +libz-sys' "rusty_rtos_kernel"

reset
# A path crate that links a PREBUILT C archive -- no `cc`, no compiler at build
# time, so nothing upstream of it trips the tooling check. Only a scan of the
# path crate ITSELF can see it, which is exactly what this case tests.
mkdir -p "$SB/evil-sys/src" "$SB/evil-sys/prebuilt" "$SB/evil"
printf 'int evil(void) { return 42; }
' > "$SB/evil/evil.c"
"$LLVM/clang" --target=riscv32-unknown-elf -march=rv32imac -mabi=ilp32 -O2 -c "$SB/evil/evil.c" -o "$SB/evil/evil.o" &&
    "$LLVM/llvm-ar" rcs "$SB/evil-sys/prebuilt/libevil.a" "$SB/evil/evil.o"
cat > "$SB/evil-sys/Cargo.toml" <<'TOML'
[package]
name = "evil-sys"
version = "0.1.0"
edition = "2021"
publish = false
links = "evil"
TOML
printf 'fn main() {
    println!("cargo:rustc-link-search=native=prebuilt");
    println!("cargo:rustc-link-lib=static=evil");
}
' > "$SB/evil-sys/build.rs"
printf '#![no_std]
' > "$SB/evil-sys/src/lib.rs"
add_dep "$SB/$KCORE/Cargo.toml" "evil-sys = { path = \"$(winpath "$SB/evil-sys")\", version = \"0.1\" }"
run_case P3 'FAIL +evil-sys' "rusty_rtos_kernel"

reset
mkdir -p "$SB/evil"
printf 'int evil_add(int a, int b) { return a + b; }\n' > "$SB/evil/evil.c"
"$LLVM/clang" --target=riscv32-unknown-elf -march=rv32imac -mabi=ilp32 -O2 -c "$SB/evil/evil.c" -o "$SB/evil/evil.o" ||
    { echo "  P4       BAD   clang could not build the planted object"; FAILED=$((FAILED + 1)); }
if [ -f "$SB/evil/evil.o" ]; then
    link_object "$SB/evil/evil.o"
    call_in_main 'fn evil_add(a: i32, b: i32) -> i32;' 'evil_add(core::hint::black_box(1), 2)'
    run_case P4 'FAIL +riscv32-qemu-preempt: non-Rust producer \[[^]]*clang' "rusty_rtos_port"
fi

reset
call_in_main 'fn __absvsi2(a: i32) -> i32;' '__absvsi2(core::hint::black_box(-7))'
run_case P5 'FAIL +riscv32-qemu-preempt: .*(GCC|__absvsi2)' "rusty_rtos_port"

reset
echo
if [ $FAILED -eq 0 ]; then
    echo "RESULT: PASS -- $PASSED/$PASSED cases behaved: the clean tree passes and every planted C is caught."
else
    echo "RESULT: FAIL -- $FAILED of $((PASSED + FAILED)) cases misbehaved. Per-case output: $SB/out.<case>"
fi
exit $((FAILED > 0))
