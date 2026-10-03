#!/usr/bin/env python3
"""The executed path of one tick-work bracket, instruction by instruction, on rv32.

`bench/kernel-ir/instr.py` gives a per-instruction census on the HOST, where the
kernel runs on `SimPort` with a trace sink inlined into every hot function. The
product is rv32 on `RiscvPort`, and there the kernel calls are inlined into the
cell's `main`, so no per-function profile can see them. QEMU can: `-d
in_asm,exec,nochain` logs every translation block once with its instructions
and then every execution of a block, in order. Expanding each execution into
its instructions gives the exact retired stream, and cutting it at the cell's
`minstret` reads gives each bracket's path -- the same instructions the row
counts, with no instrument of ours inside the bracket.

    sh bench/tick-work/trace.sh                  # record (real port)
    python3 bench/tick-work/trace.py LOG DIS             # list the brackets
    python3 bench/tick-work/trace.py LOG DIS <n>         # one bracket's path

LOG is QEMU's log and DIS the cell's `llvm-objdump -d -C` listing (demangled).
A bracket is a pair of consecutive `minstret` reads; the listing prints how
many times each pair ran and the length of its median occurrence, which is the
row value plus the bracket tax (`summarise` subtracts the tax).

TRAP: a block can be cut short by an exception, in which case its tail did
not run. The cell takes no interrupt inside a bracket, so the paths here are
exact; a path that crosses a trap would overcount by the unexecuted tail.
"""
import collections
import re
import sys


def blocks(log):
    """Translation blocks (start -> instruction addresses) and the executed
    sequence of block starts, both from QEMU's log."""
    tbs = {}
    seq = []
    cur = None
    with open(log, encoding="utf-8", errors="replace") as fh:
        for line in fh:
            if line.startswith("IN:"):
                cur = []
                continue
            if cur is not None:
                m = re.match(r"0x([0-9a-f]+):", line)
                if m:
                    cur.append(int(m.group(1), 16))
                    continue
                if cur:
                    tbs.setdefault(cur[0], cur)
                cur = None
            if line.startswith("Trace "):
                m = re.search(r"\[[0-9a-f]+/([0-9a-f]+)/", line)
                if m:
                    seq.append(int(m.group(1), 16))
    return tbs, seq


def listing(dis):
    """Address -> (instruction text, enclosing symbol), from objdump."""
    ins, sym = {}, None
    for line in open(dis, encoding="utf-8", errors="replace"):
        m = re.match(r"^([0-9a-f]{8}) <(.*)>:$", line)
        if m:
            sym = m.group(2)
            continue
        m = re.match(r"^([0-9a-f]{8}):\s+(.*)$", line)
        if m:
            ins[int(m.group(1), 16)] = (m.group(2).strip(), sym)
    return ins


def short(sym):
    """The function's own name out of a DEMANGLED symbol (objdump `-C`): the
    last `::` segment, without generic arguments."""
    if not sym:
        return "?"
    depth, cut = 0, []
    for ch in sym:
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth -= 1
        elif depth == 0:
            cut.append(ch)
    tail = "".join(cut).rstrip(":").split("::")
    return tail[-1] or sym[:40]


def main():
    log, dis = sys.argv[1], sys.argv[2]
    pick = int(sys.argv[3]) if len(sys.argv) > 3 else None
    ins = listing(dis)
    marks = {a for a, (t, _) in ins.items() if re.search(r"csrr\s+\w+, minstret$", t)}
    tbs, seq = blocks(log)
    # Cut the stream at every minstret read: each piece between two reads is
    # one occurrence of the pair (start read, end read).
    pieces = collections.defaultdict(list)
    cur, start = None, None
    for tb in seq:
        for a in tbs.get(tb, [tb]):
            if a in marks:
                if start is not None:
                    pieces[(start, a)].append(cur)
                start, cur = a, []
                continue
            if cur is not None:
                cur.append(a)
    rows = [(k, v) for k, v in pieces.items() if len(v) >= 64]
    rows.sort(key=lambda kv: kv[0][0])
    if pick is None:
        print(" n   start    end      runs  median-len")
        for i, ((s, e), v) in enumerate(rows):
            lens = sorted(len(p) for p in v)
            print(f"{i:2}  {s:08x} {e:08x} {len(v):6} {lens[len(lens) // 2]:8}")
        return
    (s, e), v = rows[pick]
    lens = sorted(len(p) for p in v)
    med = lens[len(lens) // 2]
    path = next(p for p in v if len(p) == med)
    print(f"bracket {pick}: {s:08x} -> {e:08x}, {len(v)} runs, median {med} instructions")
    per = collections.Counter(short(ins.get(a, ("?", None))[1]) for a in path)
    for name, n in per.most_common():
        print(f"  {n:5}  {name}")
    last = None
    for a in path:
        t, sy = ins.get(a, ("?", None))
        name = short(sy)
        if name != last:
            print(f"  -- {name}")
            last = name
        print(f"  {a:08x}  {t}")


if __name__ == "__main__":
    main()
