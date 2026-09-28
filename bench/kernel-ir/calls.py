#!/usr/bin/env python3
"""Call counts and caller->callee edges from a callgrind file.

`callgrind_annotate` reports SELF COST per function and no call counts at
all, so a function that is expensive because it is called a million times
reads exactly like one that is expensive per call -- and those want opposite
fixes. This parses `calls=` straight out of the raw file.

    python3 bench/kernel-ir/calls.py bench/kernel-ir/out/cg.BlockQ resume_pending
"""
import re, sys, collections

path = sys.argv[1]
want = sys.argv[2] if len(sys.argv) > 2 else None

names = {}
def resolve(tok, line):
    m = re.match(r'\((\d+)\)\s*(.*)', line)
    if not m:
        return line.strip()
    i, nm = m.group(1), m.group(2).strip()
    if nm:
        names[(tok, i)] = nm
    return names.get((tok, i), f'?{i}')

calls = collections.Counter()
edges = collections.Counter()
incl = collections.Counter()
cur = None
pend = None
for line in open(path, encoding='utf-8', errors='replace'):
    line = line.rstrip('\n')
    if line.startswith('fn='):
        cur = resolve('fn', line[3:]); pend = None
    elif line.startswith('cfn='):
        pend = resolve('fn', line[4:])
    elif line.startswith('calls='):
        n = int(line[6:].split()[0])
        if pend:
            calls[pend] += n
            edges[(cur, pend)] += n
    elif pend and line[:1].isdigit():
        incl[pend] += sum(int(x) for x in line.split()[1:2])
        pend = None

def short(n):
    n = re.sub(r'<.*?>', '', n)
    return n.split('::')[-1] or n

agg = collections.Counter()
for k, v in calls.items():
    agg[short(k)] += v
if want:
    print(f'{"calls":>12}  function (matching {want!r})')
    for nm, n in agg.most_common():
        if want in nm:
            print(f'{n:>12,}  {nm}')
    print()
    print(f'{"calls":>12}  caller -> callee')
    ag2 = collections.Counter()
    for (a, b), v in edges.items():
        if want in short(b) or want in short(a):
            ag2[(short(a), short(b))] += v
    for (a, b), n in ag2.most_common(18):
        print(f'{n:>12,}  {a} -> {b}')
else:
    print(f'{"calls":>12}  function')
    for nm, n in agg.most_common(28):
        print(f'{n:>12,}  {nm}')
