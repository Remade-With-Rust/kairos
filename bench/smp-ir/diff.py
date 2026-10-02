#!/usr/bin/env python3
"""Compare two smp-ir recordings: totals (the verdict) with anchors, then
the kernel rows that moved most."""
import sys


def totals(path):
    out = {}
    for line in open(path, encoding="utf-8"):
        parts = line.split()
        if len(parts) >= 3:
            out[(parts[0], parts[1])] = (int(parts[2]), " ".join(parts[3:]))
    return out


def rows(path):
    out = {}
    for line in open(path, encoding="utf-8"):
        parts = line.rstrip("\n").split(None, 1)
        if len(parts) == 2:
            try:
                out[parts[1]] = int(parts[0].replace(",", ""))
            except ValueError:
                pass
    return out


a, b = totals(sys.argv[1]), totals(sys.argv[2])
print("shape scenario                before          after          delta   anchors")
all_same = True
for key in a:
    if key not in b:
        continue
    (x, ax), (y, ay) = a[key], b[key]
    same = "same" if ax == ay else "MOVED: " + ay
    all_same &= ax == ay
    print(f"{key[0]:5} {key[1]:14} {x:>15,} {y:>15,} {y - x:>+12,}   {same}")
print("anchors:", "all identical" if all_same else "MOVED -- the experiment changed")
ra, rb = rows(sys.argv[3]), rows(sys.argv[4])
moved = sorted(((rb.get(k, 0) - ra.get(k, 0), k) for k in set(ra) | set(rb)), key=lambda t: abs(t[0]), reverse=True)
print("\nkernel rows that moved most:")
for d, k in moved[:15]:
    if d:
        print(f"{d:>+12,}  {k[:110]}")
