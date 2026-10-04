#!/usr/bin/env python3
"""P5: merge a mutation survey into tools/api-census/mutants.json.

    python3 tools/api-census/mutants.py <run-A mutants.out>... [--b B.tsv]... [--c C.tsv]... [--w32 <mutants.out>]...

Run A is `cargo mutants` over one kernel-core file, judged by the kernel's
own tests -- the unit suite, the API differential (scripts, pins and sweeps,
one core and two), the typed face's equivalence test and the SMP
differential. Run B (`mutants-corpus-survivors.sh`) judges what A missed
with the corpus. A mutant is KILLED if either caught it (a timeout counts:
a mutant that stops the kernel is seen by any observer), UNVIABLE if it does
not build, and SURVIVED only if both missed it. census.py attributes each to
the innermost function holding its line, as it does code regions, and
counts a survivor as explained only if tools/api-census/equivalent-mutants.json
says why it is equivalent.

`--c` takes run C's verdicts (`mutants-long-conform.sh`: `kairos conform` at
full length against the live C), keyed by the diff each was applied from; a
mutant run C caught is killed.

`--w32` takes a run made with `--target i686-pc-windows-msvc`: code behind
`#[cfg(target_pointer_width = "32")]` is not compiled on the 64-bit host,
so the host scores its mutants MISSED for building nothing. A mutant that
the 32-bit run caught is killed, and says so.

Stdlib only. The file is written sorted, so re-running a survey that found
the same thing rewrites it byte for byte.
"""
import json
import os
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(ROOT, "tools", "api-census", "mutants.json")
SUMMARY = {"CaughtMutant": "caught", "MissedMutant": "missed", "Unviable": "unviable",
           "Timeout": "timeout", "Success": "baseline"}


def main():
    a_dirs, b_files, c_files, w32_dirs, args = [], [], [], [], sys.argv[1:]
    while args:
        x = args.pop(0)
        if x == "--b":
            b_files.append(args.pop(0))
        elif x == "--c":
            c_files.append(args.pop(0))
        elif x == "--w32":
            w32_dirs.append(args.pop(0))
        else:
            a_dirs.append(x)
    w32 = set()
    for d in w32_dirs:
        data = json.load(open(os.path.join(d, "outcomes.json"), encoding="utf-8"))
        for o in data["outcomes"]:
            m = o["scenario"]
            if isinstance(m, dict) and "Mutant" in m and o["summary"] in ("CaughtMutant", "Timeout"):
                w32.add(m["Mutant"]["name"])
    b = {}
    for f in b_files:
        for line in open(f, encoding="utf-8"):
            if "\t" in line:
                name, verdict = line.rstrip("\n").split("\t", 1)
                b[name] = verdict
    c_by_diff = {}
    for f in c_files:
        for line in open(f, encoding="utf-8"):
            if "\t" in line:
                diff, verdict = line.rstrip("\r\n").split("\t", 1)
                c_by_diff[os.path.basename(diff)] = verdict
    recs = {}
    meta = {"runs": []}
    for d in a_dirs:
        data = json.load(open(os.path.join(d, "outcomes.json"), encoding="utf-8"))
        meta["runs"].append({"total": data["total_mutants"], "caught": data["caught"],
                             "missed": data["missed"], "unviable": data["unviable"],
                             "timeout": data["timeout"], "version": data["cargo_mutants_version"]})
        for o in data["outcomes"]:
            m = o["scenario"]
            if not isinstance(m, dict) or "Mutant" not in m:
                continue
            m = m["Mutant"]
            a = SUMMARY.get(o["summary"], o["summary"])
            bv = b.get(m["name"]) if a == "missed" else None
            cv = c_by_diff.get(os.path.basename(o.get("diff_path") or "")) if bv == "missed" else None
            if (a in ("caught", "timeout") or bv in ("caught", "timeout") or cv == "caught"
                    or m["name"] in w32):
                final = "killed"
            elif a == "unviable" or bv == "unviable":
                final = "unviable"
            elif a == "missed" and bv is None:
                final = "unjudged-by-corpus"
            else:
                final = "survived"
            recs[m["name"]] = {
                "file": os.path.basename(m["file"]),
                "line": m["span"]["start"]["line"],
                "col": m["span"]["start"]["column"],
                "function": m["function"]["function_name"] if m.get("function") else "",
                "replacement": m["replacement"],
                "genre": m["genre"],
                "a": a,
                "b": bv,
                "c": cv,
                "w32": m["name"] in w32,
                "final": final,
            }
    pending = [n for n, r in recs.items() if r["final"] == "unjudged-by-corpus"]
    if pending and b_files:
        sys.exit("%d run-A survivors have no run-B verdict, e.g. %s" % (len(pending), pending[0]))
    # The kernel source the survey's lines are keyed to: census.py refuses a
    # survey of other source rather than attributing its lines to whatever
    # functions sit there now. Run B and C restore the tree to HEAD, so this is
    # the source every run judged.
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import census
    meta["kernel_src"] = census.fingerprint(census.INPUTS[0][1])
    rows = [dict(name=n, **r) for n, r in sorted(recs.items(), key=lambda kv: (kv[1]["file"], kv[1]["line"],
                                                                             kv[1]["col"], kv[0]))]
    with open(OUT, "w", encoding="utf-8", newline="\n") as f:
        json.dump({"meta": meta, "mutants": rows}, f, indent=0, sort_keys=True)
        f.write("\n")
    tally = {}
    for r in rows:
        tally[r["final"]] = tally.get(r["final"], 0) + 1
    print("%d mutants: %s -> %s" % (len(rows), ", ".join("%s %d" % kv for kv in sorted(tally.items())), OUT))


if __name__ == "__main__":
    main()
