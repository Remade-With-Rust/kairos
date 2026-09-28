#!/usr/bin/env python3
"""Per-INSTRUCTION Ir for a kernel symbol, and a machine sweep for walked loops.

`callgrind_annotate` gives self cost per FUNCTION, `calls.py` gives call counts
and edges, and neither says WHERE inside a function the instructions go. Thin
LTO drops the line tables, so `--auto=yes` can only print `???`. But callgrind's
`--dump-instr=yes` records cost per ADDRESS and `objdump` knows what is at each
address, so joining them gives a line census that needs no source edit, no debug
info, and cannot perturb what it measures.

    # record once (a full callgrind run; keep it out of /tmp, WSL wipes that)
    valgrind --tool=callgrind --dump-instr=yes \
        --callgrind-out-file=$HOME/kw/cg.instr \
        $HOME/.cache/kairos-prof/release/kairos-sim BlockQ 20000 >/dev/null

    python3 bench/kernel-ir/instr.py switch_context   # census one function
    python3 bench/kernel-ir/instr.py --sweep          # every symbol, worst/call

WHAT THE SWEEP IS FOR, and what it has already settled. An instruction executing
many times per call is a loop that actually walks. A function whose work should
be O(1) per call and whose worst count is 6 is running a search nobody priced, or
has a fast path that is not being taken.

Run on 2026-09-28 it reported that `switch_context` was the ONLY hot kernel
function with a walking loop -- 1.58 ready-list levels per call, since folded --
and that every other hot function is straight-line at exactly 1.00. That is what
retired the whole SIMD / `codec-vectorize-kernel` question for this kernel, with
a measurement instead of an argument: vector work needs iterations, and there are
none. It also confirmed the delayed list's `insert_sorted` append shortcut is
being taken on every call, which is a reachability check (`codec-vectorize-kernel`
Step -1) that would otherwise have cost a counter and a rebuild.

THREE TRAPS, each of which produced wrong numbers here before it was fixed:

* `positions: instr line` means each cost line is `addr line cost`, so the cost
  is the THIRD column. With no debug info `line` is always 0, so taking column
  two reports every cost as zero.
* The line after a `calls=` carries the CALLEE'S INCLUSIVE cost, attributed to
  the call instruction's address. Summing it with self costs read
  `switch_context` at 645.8 Ir/call against a true 168.0.
* * `fl=` / `fi=` / `fe=` start a new subrange for INLINED code, and `+N`
  position compression is relative to the previous line -- so carrying the base
  across one mis-anchors every relative address after it. The symptom was `int3`
  PADDING reporting 21,690 executions. Function TOTALS stayed correct, because a
  total is a sum, so only the distribution was wrong and only in functions
  containing inlined code; `switch_context` cross-validated against
  `callgrind_annotate` the whole time and hid it.

So cross-check any census you rely on: its total must agree with
`callgrind_annotate`'s self cost for that function, and no padding instruction
may show a non-zero count.
"""

import collections
import os
import re
import subprocess
import sys

HOME = os.path.expanduser("~")
DUMP = os.environ.get("CG_INSTR", f"{HOME}/kw/cg.instr")
BIN = os.environ.get("KAIROS_BIN", f"{HOME}/.cache/kairos-prof/release/kairos-sim")


def positions(path: str) -> int:
    """How many POSITION columns precede the cost columns."""
    with open(path, encoding="utf-8", errors="replace") as fh:
        for line in fh:
            if line.startswith("positions:"):
                return len(line.split()[1:])
    return 1


class Names:
    """callgrind name compression: `(id) name` defines, `(id)` references."""

    def __init__(self) -> None:
        self.seen: dict[str, str] = {}

    def resolve(self, spec: str) -> str:
        m = re.match(r"\((\d+)\)\s*(.*)", spec)
        if not m:
            return spec.strip()
        i, nm = m.group(1), m.group(2).strip()
        if nm:
            self.seen[i] = nm
        return self.seen.get(i, "?" + i)


def read(path: str, want):
    """Self cost per address per function, plus call-site inclusive costs."""
    npos = positions(path)
    names = Names()
    per_fn = collections.defaultdict(collections.Counter)
    incl = collections.Counter()
    callee = {}

    cur = None
    base = None
    pending = False
    pname = None
    keep = True

    with open(path, encoding="utf-8", errors="replace") as fh:
        for line in fh:
            line = line.rstrip("\n")
            if line.startswith("fn="):
                cur = names.resolve(line[3:])
                keep = want is None or want in cur
                base = None
                pending = False
                continue
            if line.startswith("cfn="):
                pname = names.resolve(line[4:])
                continue
            if line.startswith("calls="):
                pending = True
                continue
            if line.startswith(("fl=", "fi=", "fe=")):
                base = None          # the third trap in the module docstring
                continue
            if line.startswith(("cfi=", "cfl=", "ob=", "cob=", "jump=", "jcnd=")):
                continue
            if not cur or not keep or not line:
                continue
            parts = line.split()
            if len(parts) < npos + 1:
                continue
            tok = parts[0]
            try:
                if tok.startswith("0x"):
                    addr = int(tok, 16)
                elif tok[0] in "+-":
                    if base is None:
                        continue
                    addr = base + int(tok)
                elif tok == "*":
                    addr = base
                else:
                    continue
                val = int(parts[npos])
            except (ValueError, TypeError):
                continue
            base = addr
            if pending:
                incl[addr] += val
                callee[addr] = pname or "?"
                pending = False
            else:
                per_fn[cur][addr] += val
    return per_fn, incl, callee


def short(name: str) -> str:
    name = re.sub(r"<.*?>", "", name)
    return name.split("::")[-1] or name


def sweep() -> None:
    per_fn, _, _ = read(DUMP, None)
    rows = []
    for fn, cost in per_fn.items():
        if "rusty_rtos" not in fn or not cost:
            continue
        calls = cost[min(cost)]
        if calls < 500:
            continue
        total = sum(cost.values())
        rows.append((max(cost.values()) / calls, total, calls, total / calls, short(fn)))
    rows.sort(reverse=True)
    print(f"{'worst/call':>10} {'self Ir':>12} {'calls':>9} {'Ir/call':>8}  function")
    for worst, total, calls, per, fn in rows:
        flag = "  <-- WALKS" if worst >= 2.0 else ""
        print(f"{worst:10.2f} {total:12,} {calls:9,} {per:8.1f}  {fn[:44]}{flag}")


def census(target: str) -> None:
    per_fn, incl, callee = read(DUMP, target)
    cost = collections.Counter()
    for fn, c in per_fn.items():
        if target in fn:
            cost.update(c)
    if not cost:
        sys.exit(f"no samples for {target!r} in {DUMP}")
    total = sum(cost.values())
    lo = min(cost)
    calls = cost[lo]
    print(f"{target}: self {total:,} over {calls:,} entries = {total / calls:.1f} Ir/call")
    print("  cross-check this total against callgrind_annotate's self cost.\n")

    dis = subprocess.run(
        ["objdump", "-d", "--demangle", BIN], capture_output=True, text=True
    ).stdout
    hi = max(list(cost) + list(incl))
    for line in dis.splitlines():
        m = re.match(r"\s+([0-9a-f]+):\s+((?:[0-9a-f]{2} )+)\s*(.*)", line)
        if not m:
            continue
        addr = int(m.group(1), 16)
        if not (lo <= addr <= hi):
            continue
        text = m.group(3).strip()
        if not text:
            continue
        c = cost.get(addr, 0)
        per = c / calls
        extra = ""
        if addr in incl:
            extra = f"  [-> {short(callee.get(addr, '?'))[:28]} incl {incl[addr]:,}]"
        flag = " <<" if per >= 1.5 else ("  ." if c == 0 else "")
        print(f"{addr:6x} {c:>10,} {per:5.2f}  {text[:56]}{flag}{extra}")


if __name__ == "__main__":
    arg = sys.argv[1] if len(sys.argv) > 1 else "--sweep"
    if arg == "--sweep":
        sweep()
    else:
        census(arg)
