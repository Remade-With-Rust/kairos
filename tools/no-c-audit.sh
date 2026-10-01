#!/bin/sh
# Is there any C or C++ in what we DEPLOY? Run before every `cargo publish`.
#
#     sh tools/no-c-audit.sh            # from the Kairos umbrella, Git Bash or WSL
#
# Exit 0 only if all three layers pass. Every failure prints the file, crate or
# symbol responsible. It changes nothing: lockfiles the umbrella's path override
# rewrites while it runs are restored on exit.
#
# ---- the three layers, and why each exists -------------------------------
#
# 1. PACKAGES. Every file Cargo would upload for every published crate. Only Rust
#    source and Cargo's own metadata may ship: no build script, no `links`, no
#    .c/.cc/.cpp/.h/.S/.asm, no prebuilt .o/.a/.lib. (First run, 2026-10-01: all
#    ten crates clean, and it found an empty `kernel.rs.fixenc` that had shipped
#    in rusty_rtos_kernel-core 0.2.1.)
#
# 2. DEPENDENCIES. The resolved graph of every published crate, per shipping
#    target, all features on, normal + build edges. Fails on any package that
#    depends on C build tooling, ships native files, or declares `links` --
#    except the two in ALLOW below, each inspected by hand:
#      xtensa-lx-rt         `links` + build.rs, but it only writes LINKER SCRIPTS
#                           (link.x, exception.x) and cfgs; no code is compiled.
#      windows_x86_64_msvc  ships an import library (.lib): a table telling the
#                           linker which Windows DLL exports to bind to. No code,
#                           and only on the x86_64 Windows HOST target.
#    `libc` (host targets only) is Rust declarations of the platform C library's
#    ABI, the same one `std` binds to; it compiles no C, so it passes on rule.
#
# 3. LINKED FIRMWARE. Layers 1-2 read manifests; this reads the binary. Rust's
#    own prebuilt `compiler_builtins` for the embedded targets contains 36-40
#    objects compiled from LLVM compiler-rt C by GCC 13.2 (absvdi2, bswapsi2,
#    cmpdi2, ...). Whether any reach a firmware is decided at link time, so each
#    firmware below is built the normal way and must carry only rustc/LLD in its
#    `.comment` and NONE of those GCC-built symbols. (bench/kernel-flash's ELF is
#    deliberately NOT used: it links with --whole-archive, which drags every
#    member's `.comment` in even though --gc-sections then keeps none of their
#    code -- the first run of this audit was fooled by exactly that.)
set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT" || exit 2
LLVM=${LLVM:-"/c/Program Files/LLVM/bin"}
[ -x "$LLVM/llvm-nm" ] || [ -x "$LLVM/llvm-nm.exe" ] || LLVM=$(dirname "$(command -v llvm-nm)")
REPOS=${REPOS:-"rusty_rtos_core rusty_rtos_kernel rusty_rtos_port"}
# Never published; excluded from layers 1 and 2. Remove it from here the day it is.
EXCLUDE=${EXCLUDE:-"rusty_rtos_port-esp-radio"}
TARGETS=${TARGETS:-"riscv32imac-unknown-none-elf thumbv7m-none-eabi thumbv7em-none-eabihf xtensa-esp32s3-none-elf x86_64-pc-windows-msvc x86_64-unknown-linux-gnu"}
# firmware dir : binary : target : cargo args
FIRMWARE=${FIRMWARE:-"
rusty_rtos_kernel/firmware/riscv32-qemu-tick-work:riscv32-qemu-tick-work:riscv32imac-unknown-none-elf:--features real-port
rusty_rtos_port/firmware/riscv32-qemu-preempt:riscv32-qemu-preempt:riscv32imac-unknown-none-elf:
rusty_rtos_demo/firmware/riscv32-qemu-corpus:riscv32-qemu-corpus:riscv32imac-unknown-none-elf:
rusty_rtos_port/firmware/mps2-an385-qemu-kernel:mps2-an385-qemu-kernel:thumbv7m-none-eabi:
"}
TMP=${TMPDIR:-/tmp}/no-c-audit.$$
mkdir -p "$TMP"
restore() {
    for r in $REPOS rusty_rtos_demo; do
        git -C "$r" status --short 2>/dev/null | awk '$2 ~ /Cargo\.lock$/ {print $2}' |
            xargs -r git -C "$r" checkout -- 2>/dev/null
    done
    rm -rf "$TMP"
}
trap restore EXIT
FAIL=0
fail() { echo "      FAIL  $*"; FAIL=1; }

echo "=== 1. packages: every file each published crate would upload ==="
for r in $REPOS; do
    # One line per publishable package: its name, then any `links` key or build
    # script its manifest declares. `tr -d '\r'`: Python on Windows writes CRLF,
    # and a trailing CR silently corrupts every package name it is attached to.
    (cd "$r" && cargo metadata -q --no-deps --format-version 1 2>/dev/null) |
        python3 -c 'import sys,json
for p in json.load(sys.stdin)["packages"]:
    if p.get("publish") == []: continue
    b = [t["name"] for t in p["targets"] if "custom-build" in t["kind"]]
    print(p["name"], ("links=" + p["links"]) if p.get("links") else "", ("build-script=" + ",".join(b)) if b else "")' |
        tr -d '\r' > "$TMP/pkgs.$r"
    while read -r p man; do
        case " $EXCLUDE " in *" $p "*) printf '      skip  %-28s never published\n' "$p"; continue ;; esac
        list=$(cd "$r" && cargo package -q --list --allow-dirty -p "$p" 2>"$TMP/err" | tr -d '\r') ||
            { fail "$p: cargo package --list failed: $(head -1 "$TMP/err")"; continue; }
        n=$(printf '%s\n' "$list" | grep -c .)
        bad=$(printf '%s\n' "$list" | grep -viE '\.rs$|^Cargo\.(toml|toml\.orig|lock)$|^\.cargo_vcs_info\.json$|(^|/)(README|LICENSE|CHANGELOG)[^/]*$')
        if [ -n "$bad" ] || [ -n "$man" ]; then
            fail "$p ships: $(echo $bad) $man"
        else
            printf '      ok    %-28s %3d files, Rust and Cargo metadata only\n' "$p" "$n"
        fi
    done < "$TMP/pkgs.$r"
done

echo
echo "=== 2. dependencies: the resolved graph, every shipping target, all features ==="
for r in $REPOS; do
    for t in $TARGETS; do
        (cd "$r" && cargo metadata -q --format-version 1 --all-features --filter-platform "$t" 2>/dev/null) \
            > "$TMP/meta.$r.$t.json" || echo "      note  $r @ $t: metadata failed, skipped"
    done
done
python3 - "$TMP" "$EXCLUDE" $REPOS <<'PY' || FAIL=1
import glob, json, os, sys
tmp, exclude, repos = sys.argv[1], set(sys.argv[2].split()), sys.argv[3:]
TOOLING = {"cc", "cmake", "bindgen", "pkg-config", "vcpkg", "cxx", "cxx-build", "autotools"}
NATIVE = (".c", ".cc", ".cpp", ".cxx", ".h", ".hpp", ".hh", ".s", ".S", ".asm", ".o", ".a", ".lib", ".so", ".dll", ".dylib")
ALLOW = {
    "xtensa-lx-rt": "links + build.rs write linker scripts only (link.x, exception.x); no code compiled",
    "windows_x86_64_msvc": "Windows import library (.lib): DLL binding table, no code; x86_64 Windows host only",
}
seen, bad = {}, []
for path in sorted(glob.glob(os.path.join(tmp, "meta.*.json"))):
    if os.path.getsize(path) == 0:
        continue
    target = path.split("meta.", 1)[1].rsplit(".json", 1)[0].split(".", 1)[1]
    m = json.load(open(path, encoding="utf-8"))
    pk = {p["id"]: p for p in m["packages"]}
    nodes = {n["id"]: n for n in m["resolve"]["nodes"]}
    roots = [p["id"] for p in m["packages"] if p["id"] in m["workspace_members"]
             and p.get("publish") != [] and p["name"] not in exclude]
    stack, reach = list(roots), set()
    while stack:                                   # normal + build edges only
        i = stack.pop()
        if i in reach: continue
        reach.add(i)
        for d in nodes[i]["deps"]:
            if any(k["kind"] in (None, "build") for k in d["dep_kinds"]):
                stack.append(d["pkg"])
    for i in reach:
        p = pk[i]
        if p["source"] is None:                    # our own workspace crates: layer 1
            continue
        key = (p["name"], p["version"])
        seen.setdefault(key, set()).add(target)
        tool = sorted({d["name"] for d in p["dependencies"]} & TOOLING)
        root = os.path.dirname(p["manifest_path"])
        files = []
        for dp, _, fs in os.walk(root):
            files += [os.path.relpath(os.path.join(dp, f), root) for f in fs if f.endswith(NATIVE)]
        why = []
        if tool: why.append("C build tooling " + ",".join(tool))
        if files: why.append("native files " + " ".join(sorted(files)[:4]) + (" ..." if len(files) > 4 else ""))
        if p.get("links"): why.append("links=" + p["links"])
        if why and p["name"] not in ALLOW:
            bad.append((key, target, "; ".join(why)))
print(f"      {len(seen)} third-party packages reachable across {len(repos)} workspaces and all targets")
for (n, v), ts in sorted(seen.items()):
    note = f"  ALLOWED: {ALLOW[n]}" if n in ALLOW else ""
    print(f"      {'ok' if not any(b[0] == (n, v) for b in bad) else 'FAIL':5} {n} {v}{note}")
for (n, v), t, why in sorted(set(bad)):
    print(f"      FAIL  {n} {v} @ {t}: {why}")
sys.exit(1 if bad else 0)
PY

echo
echo "=== 3. linked firmware: compiler labels and GCC-built symbols ==="
SYSROOT=$(rustc --print sysroot)
command -v cygpath >/dev/null 2>&1 && SYSROOT=$(cygpath -u "$SYSROOT")
gcc_syms() {    # every symbol defined by a GCC-built member of compiler_builtins
    lib=$(ls "$SYSROOT/lib/rustlib/$1/lib/"libcompiler_builtins-*.rlib 2>/dev/null | head -1)
    [ -n "$lib" ] || return 1
    d="$TMP/cb.$1"; mkdir -p "$d"; (cd "$d" && "$LLVM/llvm-ar" x "$lib")
    for o in "$d"/*.o; do
        "$LLVM/llvm-readobj" -p .comment "$o" 2>/dev/null | grep -q GCC &&
            "$LLVM/llvm-nm" --defined-only "$o" 2>/dev/null | awk '$2 ~ /[TtWwRrDd]/ {print $3}'
    done | sort -u > "$TMP/gcc.$1"
}
# A plain redirect, not a pipe: a `while` at the end of a pipeline runs in a
# subshell, and a FAIL set there would never reach the exit status.
printf '%s\n' "$FIRMWARE" > "$TMP/fw.list"
while IFS=: read -r dir bin tgt args; do
    [ -n "$dir" ] || continue
    [ -s "$TMP/gcc.$tgt" ] || gcc_syms "$tgt" || { fail "$bin: toolchain for $tgt not installed"; continue; }
    (cd "$dir" && cargo build -q --release $args 2>"$TMP/err") || { fail "$bin: build failed: $(grep -m1 error "$TMP/err")"; continue; }
    elf="$dir/target/$tgt/release/$bin"
    labels=$("$LLVM/llvm-readobj" -p .comment "$elf" 2>/dev/null | sed -nE 's/^ *\[ *[0-9a-f]+\] +//p')
    other=$(printf '%s\n' "$labels" | grep -vE '^(rustc version|Linker: LLD)' | grep .)
    "$LLVM/llvm-nm" "$elf" 2>/dev/null | awk '{print $NF}' | sort -u > "$TMP/fw.syms"
    hits=$(comm -12 "$TMP/fw.syms" "$TMP/gcc.$tgt" | head -5 | tr '\n' ' ')
    if [ -n "$other" ] || [ -n "$hits" ]; then
        fail "$bin: non-Rust producer [$(echo $other)] GCC-built symbols [$hits]"
    else
        printf '      ok    %-26s rustc + LLD only; 0 of %s GCC-built compiler_builtins symbols\n' "$bin" "$(wc -l < "$TMP/gcc.$tgt")"
    fi
done < "$TMP/fw.list"

echo
if [ "$FAIL" -eq 0 ]; then
    echo "RESULT: PASS -- no C or C++ in any published crate, any dependency, or any linked firmware."
else
    echo "RESULT: FAIL -- see the lines above. Do not publish."
fi
exit "$FAIL"
