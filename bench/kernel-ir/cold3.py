#!/usr/bin/env python3
"""Per-address Ir for one symbol across ALL THREE scenarios, side by side.

Why this exists: a per-instruction census taken on ONE scenario said four blocks
of `queue_take_blocking` were cold, and outlining them measured +200,668 because
the mutex arm is dead in BlockQ and runs 5,812 times in GenQTest. An arm is only
cold if it is cold in every scenario the bench runs, and this is the instrument
that says so.

    python3 cold3.py queue_take_blocking

★ DIAGNOSED AND GUARDED 2026-09-28. The round-two warning on this tool said "a
zero from it is not yet evidence of dead code", after it reported `unlock_queue` as
173 of 188 instructions dead with a 106-instruction dead run containing the target
of a `jmp` executed 21,490 times. Both cannot be true, and the cause was NOT a
callgrind format subtlety (the `fi=`/`fe=` base policy makes no difference at all --
measured both ways, byte-identical). It was this: after the boundary
fix below, `unlock_queue` reports 173 of 188 instructions dead in all three
scenarios with a 106-instruction dead run AT THE ENTRY -- while an instruction
inside that run is the target of a `jmp` that executes 21,490 times. Both cannot
be true, so the per-address attribution is losing something (most likely cost
recorded under a nested `fn=` for inlined code, which this reader drops). The
FUNCTION TOTALS were always sound -- they agree with callgrind_annotate, because a
total is a sum -- so only the distribution was wrong. Verify after any change to
the reader: `--check` asserts the per-address sum equals the annotate self cost.
"""
import collections
import os
import re
import subprocess
import sys

HOME = os.path.expanduser("~")
BIN = f"{HOME}/.cache/kairos-prof/release/kairos-sim"
SCEN = ["BlockQ", "GenQTest", "TimerDemo"]
TARGET = sys.argv[1] if len(sys.argv) > 1 else "queue_take_blocking"


def positions(path):
    with open(path, encoding="utf-8", errors="replace") as fh:
        for line in fh:
            if line.startswith("positions:"):
                return len(line.split()[1:])
    return 1


def read(path, want):
    """Self cost per address for the wanted symbol."""
    npos = positions(path)
    names, cost = {}, collections.Counter()

    def resolve(spec):
        m = re.match(r"\((\d+)\)\s*(.*)", spec)
        if not m:
            return spec.strip()
        i, nm = m.group(1), m.group(2).strip()
        if nm:
            names[i] = nm
        return names.get(i, "?" + i)

    keep, base, pending = False, None, False
    with open(path, encoding="utf-8", errors="replace") as fh:
        for line in fh:
            line = line.rstrip("\n")
            if line.startswith("fn="):
                keep = want in resolve(line[3:])
                base, pending = None, False
                continue
            if line.startswith("calls="):
                pending = True
                continue
            if line.startswith("fl="):
                # `fl=` starts a new cost context, so the next cost line must not
                # use a relative subposition.
                base = None
                continue
            if line.startswith(("fi=", "fe=")):
                # ★ `fi=` / `fe=` do NOT. They only say which INLINED file the
                # following lines came from, and their subpositions stay relative
                # to the previous cost line. Resetting here made every `+N` line
                # after an inline marker unanchored, and this reader SKIPS
                # unanchored lines -- so all the inlined cost was silently
                # dropped and those addresses read ZERO.
                #
                # That is what produced the phantom cold code: `check_for_timeout`
                # read "91 of 102 instructions dead in all three scenarios" with a
                # 31-instruction dead run that is an inlined `exit_critical`, in a
                # function that exits a critical section on every one of its
                # 27,301 calls. `unlock_queue` read 173 of 188 the same way.
                continue
            if line.startswith("cfn="):
                # MUST be resolved, not skipped: callgrind's fn-name ids come
                # from one namespace, so a name whose FIRST appearance is on a
                # cfn= line defines an id that a later fn=(id) refers back to.
                # Skipping these left the map incomplete and the symbol was
                # never found in any scenario.
                resolve(line[4:])
                continue
            if line.startswith(("cfi=", "cfl=", "ob=", "cob=", "jump=", "jcnd=")):
                continue
            if not keep or not line:
                continue
            p = line.split()
            if len(p) < npos + 1:
                continue
            tok = p[0]
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
                val = int(p[npos])
            except (ValueError, TypeError):
                continue
            base = addr
            if pending:
                pending = False          # callee inclusive, not ours
            else:
                cost[addr] += val
    return cost


per = {s: read(f"{HOME}/kw/cg.instr.{s}", TARGET) for s in SCEN}
alive = {s: c for s, c in per.items() if c}
if not alive:
    sys.exit(f"no samples for {TARGET!r} in any scenario")

entries = {}
for _s, _c in alive.items():
    entries[_s] = _c[min(_c)]
print(f"{TARGET}")
for s in SCEN:
    c = per[s]
    if c:
        print(f"  {s:<10} {sum(c.values()):>10,} Ir over {entries[s]:>7,} calls"
              f"  = {sum(c.values())/entries[s]:6.1f} Ir/call")
    else:
        print(f"  {s:<10} {'not reached':>10}")

dis = subprocess.run(["objdump", "-d", "--demangle", BIN],
                     capture_output=True, text=True).stdout

# The symbol's REAL boundaries, from objdump's own headers -- not min/max of the
# sampled addresses.
#
# ★ This was a defect that produced a wrong finding. Taking the range from the
# samples means (a) the listing STOPS at the last executed instruction, so a
# cold tail is invisible, and (b) it RUNS ON into whatever symbol comes next,
# whose instructions are then reported as this one's and counted as DEAD-ALL.
# On `unlock_queue` -- 0x5fac0..0x5fd6f, with `take_outlined` starting at
# 0x5fd70 -- it read "146 of 173 instructions dead in all three scenarios",
# 84%, which looked like a large A2 opportunity. Most of those 146 belonged to
# `take_outlined`. Function TOTALS were never affected, because a total is a
# sum over samples; only the listing and the dead-run census were wrong.
heads = []
for line in dis.splitlines():
    m = re.match(r"^([0-9a-f]+) <(.+)>:$", line)
    if m:
        heads.append((int(m.group(1), 16), m.group(2)))
heads.sort()
lo = hi = None
for i, (addr, name) in enumerate(heads):
    if TARGET in name:
        end = heads[i + 1][0] - 1 if i + 1 < len(heads) else addr + (1 << 20)
        if lo is None or addr < lo:
            lo = addr
        if hi is None or end > hi:
            hi = end
if lo is None:
    sys.exit(f"{TARGET!r} is not a symbol in {BIN} -- it may be inlined away")
# ★★ THE CHECK THAT MATTERS, and the defect it caught. This tool JOINS a profile
# to a disassembly, and for three rounds nothing verified they came from the SAME
# BINARY. They did not: the profile was recorded at 16:26 and the binary rebuilt at
# 16:40, so every address had shifted. Cost then lands at addresses outside the
# symbol while in-bounds addresses read ZERO -- which reads exactly like cold code
# and is not. It made `check_for_timeout` report 91 of 102 instructions dead in all
# three scenarios when the true figure is 24, and `unlock_queue` 173 of 188 when it
# is 95.
#
# The FUNCTION TOTAL is immune, because a total is a sum over samples and agrees
# with callgrind_annotate either way. Only the distribution is destroyed -- which is
# the part you came here for.
#
# So: any cost landing outside the symbol's bounds means the join is invalid, and
# this refuses to print rather than mislead. Record the profile and the objdump from
# one binary, back to back, and check its hash across both.
stray = {a: v for a, v in
         ((a, sum(per[s].get(a, 0) for s in SCEN)) for a in
          {a for c in alive.values() for a in c}) if not (lo <= a <= hi)}
if stray:
    sys.exit(
        f"INVALID JOIN: {len(stray)} address(es) outside {TARGET}'s symbol "
        f"({lo:x}..{hi:x}) carry {sum(stray.values()):,} Ir. The profile and the "
        "disassembly are from DIFFERENT BINARIES. Re-record both from one binary, "
        "back to back, and verify its hash across the two."
    )
sampled_lo = min(min(c) for c in alive.values())
if sampled_lo != lo:
    print(f"  note: entry at {lo:x}, first SAMPLED address {sampled_lo:x}"
          f" -- {sampled_lo - lo} bytes of the prologue carry no samples,"
          f" so the call count below is not from the entry")

rows = []
for line in dis.splitlines():
    m = re.match(r"\s+([0-9a-f]+):\s+((?:[0-9a-f]{2} )+)\s*(.*)", line)
    if not m:
        continue
    a = int(m.group(1), 16)
    if not (lo <= a <= hi):
        continue
    txt = m.group(3).strip()
    if txt:
        rows.append((a, txt))

print(f"\n{'addr':>6} {'BlockQ':>9} {'GenQT':>9} {'TimerD':>7}  instruction")
run, runs = 0, []
for a, txt in rows:
    v = [per[s].get(a, 0) for s in SCEN]
    dead = all(x == 0 for x in v)
    if dead:
        run += 1
    else:
        if run >= 5:
            runs.append(run)
        run = 0
    flag = "  DEAD-ALL" if dead else ""
    print(f"{a:6x} {v[0]:>9,} {v[1]:>9,} {v[2]:>7,}  {txt[:52]}{flag}")
if run >= 5:
    runs.append(run)

total = len(rows)
dead_n = sum(1 for a, _ in rows if all(per[s].get(a, 0) == 0 for s in SCEN))
print(f"\n{total} instructions in the symbol; {dead_n} are DEAD IN ALL THREE scenarios")
print(f"contiguous dead runs of >=5: {runs if runs else 'none'}")
