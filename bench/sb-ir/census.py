"""The call-count and caller-callee censuses, from a raw callgrind file.

`callgrind_annotate` answers "where are the instructions?" and nothing
else. Two questions it cannot answer decide most bricks:

  * **called a lot, or expensive per call?** A function costing 7M over
    600k calls wants a cheaper loop; the same 7M over 900 calls wants a
    cheaper algorithm. Opposite fixes, same self-cost row.
  * **which caller is buying it?** A function's total says what it costs;
    the edges say who to change. Three call sites with three different
    per-call costs are three different decisions.

Usage:

    python3 bench/sb-ir/census.py <callgrind-out> [--top N] [--match SUBSTR]

The format is documented in callgrind's manual. Names are interned:
`fn=(7) some::name` defines id 7 and a later bare `fn=(7)` refers to it.
Cost lines are subposition-compressed, so the first field may be `+3`,
`-2` or `*` rather than an absolute line -- but only the COST columns
matter here, and they are always absolute.
"""

import re
import sys

NAME = re.compile(r"^(fl|fi|fe|fn|cfl|cfi|cfn|ob|cob)=(?:\((\d+)\))?\s*(.*)$")
CALLS = re.compile(r"^calls=(\d+)")


def strip_generics(s):
    """Remove every balanced `<...>` group, however nested.

    Shared with names.py by copy rather than by import: a regex cannot do
    it, and a bench helper that needs a package layout to run is a bench
    helper nobody runs.
    """
    out, depth = [], 0
    for ch in s:
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth = max(0, depth - 1)
        elif depth == 0:
            out.append(ch)
    return "".join(out)


def short(sym):
    sym = strip_generics(sym).replace("as ", "").strip()
    parts = [p for p in sym.split("::") if p]
    if not parts:
        return "?"
    if len(parts) >= 3:
        return f"{parts[1]}::{parts[-1]}"
    return "::".join(parts)


def parse(path):
    names = {}          # kind -> {id: name}
    self_cost = {}      # fn -> Ir
    calls = {}          # fn -> times called
    incl = {}           # fn -> inclusive Ir charged to its callers
    edges = {}          # (caller, callee) -> [count, incl]

    cur_fn = None
    pending_callee = None
    pending_calls = 0

    with open(path, encoding="utf-8", errors="replace") as fh:
        for line in fh:
            line = line.rstrip("\n")
            if not line or line.startswith("#"):
                continue

            m = NAME.match(line)
            if m:
                kind, ident, text = m.group(1), m.group(2), m.group(3)
                table = names.setdefault(kind.lstrip("c"), {})
                if ident is not None:
                    if text:
                        table[ident] = text
                    text = table.get(ident, text)
                if kind == "fn":
                    cur_fn = short(text)
                    self_cost.setdefault(cur_fn, 0)
                elif kind == "cfn":
                    pending_callee = short(text)
                continue

            m = CALLS.match(line)
            if m:
                pending_calls = int(m.group(1))
                continue

            # A cost line: "<position> <Ir> ...". The position may be
            # compressed (`+3`, `-2`, `*`); the cost never is.
            fields = line.split()
            if len(fields) < 2:
                continue
            try:
                cost = int(fields[1])
            except ValueError:
                continue

            if pending_callee is not None:
                # This line is the cost of the call that was just declared.
                key = (cur_fn, pending_callee)
                row = edges.setdefault(key, [0, 0])
                row[0] += pending_calls
                row[1] += cost
                calls[pending_callee] = calls.get(pending_callee, 0) + pending_calls
                incl[pending_callee] = incl.get(pending_callee, 0) + cost
                pending_callee = None
                pending_calls = 0
            elif cur_fn is not None:
                self_cost[cur_fn] = self_cost.get(cur_fn, 0) + cost

    return self_cost, calls, incl, edges


def main():
    path = sys.argv[1]
    top = 18
    match = ""
    args = sys.argv[2:]
    for i, a in enumerate(args):
        if a == "--top":
            top = int(args[i + 1])
        elif a == "--match":
            match = args[i + 1]

    self_cost, calls, incl, edges = parse(path)

    def keep(name):
        return match in name

    print("== (b) call counts: called a lot, or expensive per call? ==")
    print(f"{'calls':>12}{'incl Ir':>15}{'per call':>11}{'self Ir':>14}  function")
    rows = [f for f in calls if keep(f)]
    rows.sort(key=lambda f: -incl.get(f, 0))
    for f in rows[:top]:
        n = calls[f]
        i = incl.get(f, 0)
        per = i / n if n else 0.0
        print(f"{n:>12,}{i:>15,}{per:>11.1f}{self_cost.get(f, 0):>14,}  {f}")

    print()
    print("== (c) edges: which caller is buying it? ==")
    print(f"{'calls':>12}{'incl Ir':>15}{'per call':>11}  caller -> callee")
    pairs = [(k, v) for k, v in edges.items() if keep(k[1]) or keep(k[0])]
    pairs.sort(key=lambda kv: -kv[1][1])
    for (caller, callee), (n, i) in pairs[:top]:
        per = i / n if n else 0.0
        print(f"{n:>12,}{i:>15,}{per:>11.1f}  {caller} -> {callee}")


if __name__ == "__main__":
    main()
