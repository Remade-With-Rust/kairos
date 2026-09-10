"""Turn `callgrind_annotate` rows into `<Ir> <crate::module::function>`.

A regex cannot do this. Rust symbols here look like

    ???:<rusty_rtos_kernel_core::kernel::Kernel<C, P, T, H, 24, 80, ...>>::queue_send_generic [/tmp/bin]

and the generic argument list contains `::`, `<`, `>` and commas of its
own, so `s/<[^>]*>//g` eats the wrong half and leaves rows that differ
between scenarios only in the wreckage — which then fail to merge, and the
same function is reported twice with two different totals. That is the
shape of a census that lies (`codec-measurement`: a broken instrument does
not fail everywhere equally).

Strip `<...>` by counting depth, then keep the last two `::` segments.
"""

import re
import sys

IR = re.compile(r"^\s*([\d,]+)\s*\([^)]*\)\s*(.*)$")


def strip_generics(s: str) -> str:
    """Remove every balanced `<...>` group, however nested."""
    out, depth = [], 0
    for ch in s:
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth = max(0, depth - 1)
        elif depth == 0:
            out.append(ch)
    return "".join(out)


def short(sym: str) -> str:
    sym = sym.split(" [", 1)[0]          # drop the binary path
    sym = sym.split("???:", 1)[-1]       # drop the file placeholder
    sym = strip_generics(sym)
    sym = sym.replace("as ", "").strip()
    parts = [p for p in sym.split("::") if p]
    if not parts:
        return "?"
    # `crate::module::Type::method` -> `module::method`; a bare function
    # keeps its module too.
    if len(parts) >= 3:
        return f"{parts[1]}::{parts[-1]}"
    return "::".join(parts)


def main() -> None:
    totals: dict[str, int] = {}
    for line in sys.stdin:
        m = IR.match(line)
        if not m:
            continue
        ir = int(m.group(1).replace(",", ""))
        name = short(m.group(2))
        totals[name] = totals.get(name, 0) + ir
    for name, ir in sorted(totals.items(), key=lambda kv: -kv[1]):
        print(f"{ir} {name}")


if __name__ == "__main__":
    main()
