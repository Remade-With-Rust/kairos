#!/usr/bin/env python3
"""Attribute linked `.text` bytes to the Rust source file that defines each symbol.

WHY THIS EXISTS. Group totals used to be taken with
`llvm-nm --demangle | grep -i <module>`, and that filter cannot see its own
subject. Rust mangles a method by the path of the TYPE its `impl` block is on,
not by the file the code lives in, so every helper in `stream.rs` written as
`impl Kernel<..>` demangles as `kernel::Kernel<..>::write_message` with the
string "stream" nowhere in it. On 2026-09-25 that filter hid three of seven
stream.rs symbols (472 of 1,954 bytes) and reported stream buffers as a WIN
against C when they were at 1.33x.

The tell, if it happens again: a group total that falls by more than the whole
binary does. A group cannot outrun its own binary.

So: match on the set of `fn` names the file actually defines. Pass the source
file and the linked ELF.

    python tools/flash-by-module.py <src.rs> <linked.elf> [extra_name_prefix ...]

`extra_name_prefix` picks up `#[no_mangle]` entry points that belong to the
group but are not defined in the file (the `kairos_*` C-ABI shims).
"""
import re
import subprocess
import sys

NM_CANDIDATES = [
    "llvm-nm",
    "/c/Program Files/LLVM/bin/llvm-nm",
    r"C:\Program Files\LLVM\bin\llvm-nm.exe",
]


def nm_lines(elf):
    last = None
    for exe in NM_CANDIDATES:
        try:
            out = subprocess.run(
                [exe, "--print-size", "--demangle", elf],
                capture_output=True, text=True, check=True,
            )
            return out.stdout.splitlines()
        except (OSError, subprocess.CalledProcessError) as e:
            last = e
    raise SystemExit("could not run llvm-nm: %s" % last)


def defined_names(src):
    # Only above the test module: `#[cfg(test)]` code is not linked.
    body = open(src, encoding="utf-8").read().split("mod tests")[0]
    return set(re.findall(r"\bfn (\w+)", body))


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    src, elf, prefixes = sys.argv[1], sys.argv[2], tuple(sys.argv[3:])
    names = defined_names(src)

    rows, total = [], 0
    for line in nm_lines(elf):
        f = line.split()
        if len(f) < 4:
            continue
        try:
            size = int(f[1], 16)
        except ValueError:
            continue
        full = " ".join(f[3:])
        # The trailing path component is the method name; a `#[no_mangle]`
        # symbol has no `::` at all.
        short = full.rsplit("::", 1)[-1] if "::" in full else full
        if short in names or (prefixes and short.startswith(prefixes)):
            rows.append((size, short))
            total += size

    for size, name in sorted(rows, reverse=True):
        print("%7d  %s" % (size, name))
    print("%7d  TOTAL (%d symbols from %s)" % (total, len(rows), src))


if __name__ == "__main__":
    main()
