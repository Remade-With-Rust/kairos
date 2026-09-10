"""What moved between two `kernel-ir` recordings.

Reports rows that changed, most-negative first, then two totals — and the
second is the verdict.

A subtotal over the rows present in BOTH files silently drops any function
that was renamed or that inlining moved into a neighbour, so a brick whose
work crossed an inlining boundary reads as a large regression that did not
happen. One did: renaming `insert` to `insert_inner` and letting its body
inline into two callers printed +1,758,490 on the rows in both, against a
true −600,203 over everything.

So sum EVERY row on each side. The two recordings cover the same program
on the same workload, so the grand totals are comparable even when the
rows are not.
"""

import sys


def read(path: str) -> dict[str, int]:
    out: dict[str, int] = {}
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            parts = line.split(None, 1)
            if len(parts) == 2 and parts[0].isdigit():
                out[parts[1].strip()] = int(parts[0])
    return out


def main() -> None:
    a, b = read(sys.argv[1]), read(sys.argv[2])
    both = sorted(set(a) & set(b), key=lambda f: b[f] - a[f])

    moved = [f for f in both if b[f] != a[f]]
    if moved:
        print(f"{'function':<52}{'baseline':>13}{'after':>13}{'delta':>11}")
        for f in moved:
            print(f"{f:<52}{a[f]:>13,}{b[f]:>13,}{b[f] - a[f]:>+11,}")

    only_a = sorted(set(a) - set(b))
    only_b = sorted(set(b) - set(a))
    if only_a or only_b:
        print("\nrows in only one recording (inlining moved, not work removed):")
        for f in only_a:
            print(f"  gone    {f:<44}{a[f]:>13,}")
        for f in only_b:
            print(f"  new     {f:<44}{b[f]:>13,}")

    ta, tb = sum(a[f] for f in both), sum(b[f] for f in both)
    print(f"\n{'subtotal over rows in both':<52}{ta:>13,}{tb:>13,}{tb - ta:>+11,}")

    ga, gb = sum(a.values()), sum(b.values())
    print(f"{'GRAND TOTAL over every row  (the verdict)':<52}{ga:>13,}{gb:>13,}{gb - ga:>+11,}")
    if ga:
        print(f"{'':<52}{'':>13}{'':>13}{(gb - ga) / ga * 100:>10.2f}%")


if __name__ == "__main__":
    main()
