#!/usr/bin/env python3
"""The API census: render docs/API-COVERAGE.md from tools/api-census/run.sh.

    python3 tools/api-census/census.py            # render from out/*.json
    python3 tools/api-census/census.py --check    # fail if the doc is stale

Every function in rusty_rtos_kernel-core/src is found by parsing the source
(its body's line range, by brace matching), and every LLVM code region the
three groups recorded is attributed to the INNERMOST function whose range
holds it. Attribution is by source line, so it is the same whether the
compiler inlined the function or not, and a generic's instantiations are
unioned (a region counts if ANY instantiation ran it).

A public function's C twin is the first backticked name on its doc comment's
first line, and it counts as a twin only if the pinned FreeRTOS headers
DECLARE it (a prototype or a #define). Names the headers do not declare --
`xPendedTicks`, `uxSchedulerSuspended` -- are the C's state variables: the
function is an accessor, and there is no C call to compare it with.

Stdlib only. The summary it renders from is written to
docs/api-coverage.json, which is what --check re-renders.
"""
import json
import os
import re
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
SRC = os.path.join(ROOT, "rusty_rtos_kernel", "crates", "rusty_rtos_kernel-core", "src")
OUT = os.path.join(ROOT, "tools", "api-census", "out")
HEADERS = os.path.join(ROOT, "oracle", "FreeRTOS-Kernel", "include")
DOC = os.path.join(ROOT, "docs", "API-COVERAGE.md")
DATA = os.path.join(ROOT, "docs", "api-coverage.json")
GROUPS = ("C1", "C1L", "C2", "ANY")

FN = re.compile(r"^(\s*)(pub(\([^)]*\))?\s+)?(const\s+)?(unsafe\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)")
CNAME = re.compile(r"`([A-Za-z_][A-Za-z0-9_]*)[^`]*`")
# Not shipped code: `proofs.rs` is #[cfg(kani)], `smp_tests.rs` #[cfg(test)], and
# `flash_lib.rs` / `ram_lib.rs` are not in the library's module tree at all.
NOT_SHIPPED = {"proofs.rs", "smp_tests.rs", "flash_lib.rs", "ram_lib.rs"}


def strip_code(line, in_block):
    """The line with comments, strings and char literals blanked, so braces
    inside them are not counted. Returns (code, still_in_block_comment)."""
    out, i, n = [], 0, len(line)
    while i < n:
        if in_block:
            j = line.find("*/", i)
            if j < 0:
                return "".join(out), True
            i, in_block = j + 2, False
            continue
        c = line[i]
        if line.startswith("//", i):
            break
        if line.startswith("/*", i):
            in_block, i = True, i + 2
            continue
        if c == '"':
            j = i + 1
            while j < n and line[j] != '"':
                j += 2 if line[j] == "\\" else 1
            out.append('""')
            i = j + 1
            continue
        m = re.match(r"'(\\.|[^\\'])'", line[i:])
        if m:
            out.append("''")
            i += m.end()
            continue
        out.append(c)
        i += 1
    return "".join(out), in_block


def functions(path):
    """Every fn in the file: (name, is_pub, start, end, doc_first_line), with
    #[cfg(test)] modules excluded."""
    lines = open(path, encoding="utf-8").read().split("\n")
    code, block = [], False
    for ln in lines:
        c, block = strip_code(ln, block)
        code.append(c)
    # #[cfg(test)] module ranges
    skip = []
    for i, ln in enumerate(lines):
        if ln.strip() == "#[cfg(test)]":
            for k in range(i + 1, min(i + 4, len(lines))):
                if re.match(r"\s*(pub\s+)?mod\s+\w+\s*\{", lines[k]):
                    skip.append((k + 1, block_end(code, k) + 1))
                    break
    out = []
    for i, ln in enumerate(lines):
        m = FN.match(ln)
        if not m:
            continue
        if any(a <= i + 1 <= b for a, b in skip):
            continue
        # the body starts at the first `{` after the signature (not a `;`)
        k = i
        while k < len(code) and "{" not in code[k] and ";" not in code[k]:
            k += 1
        if k >= len(code) or ("{" not in code[k]):
            continue  # a trait method declaration with no body
        end = block_end(code, k)
        # the doc comment above, skipping attributes
        j, doc = i - 1, []
        while j >= 0 and (lines[j].strip().startswith("#[") or lines[j].strip().startswith("///")
                          or lines[j].strip().startswith("#![")):
            if lines[j].strip().startswith("///"):
                doc.insert(0, lines[j].strip()[3:].strip())
            j -= 1
        is_pub = m.group(2) is not None and m.group(3) is None
        out.append((m.group(6), is_pub, i + 1, end + 1, doc[0] if doc else ""))
    return out


def block_end(code, start):
    depth, seen = 0, False
    for k in range(start, len(code)):
        for ch in code[k]:
            if ch == "{":
                depth, seen = depth + 1, True
            elif ch == "}":
                depth -= 1
                if seen and depth == 0:
                    return k
    return len(code) - 1


def declared_in_headers():
    names = set()
    if not os.path.isdir(HEADERS):
        return None
    for f in os.listdir(HEADERS):
        if not f.endswith(".h"):
            continue
        txt = open(os.path.join(HEADERS, f), encoding="utf-8", errors="replace").read()
        names.update(re.findall(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(", txt))
        names.update(re.findall(r"#define\s+([A-Za-z_][A-Za-z0-9_]*)", txt))
    return names


def regions(group):
    """{(file_basename, line_start, col_start, line_end, col_end): executed}"""
    path = os.path.join(OUT, group + ".json")
    if not os.path.isfile(path):
        return {}
    data = json.load(open(path, encoding="utf-8"))
    out = {}
    for fn in data["data"][0]["functions"]:
        files = fn["filenames"]
        for r in fn["regions"]:
            ls, cs, le, ce, count, fid, _exp, kind = r[:8]
            if kind != 0:  # code regions only (not gap, skipped, expansion, branch)
                continue
            f = os.path.normpath(files[fid])
            if os.path.normpath(SRC) not in f:
                continue
            key = (os.path.basename(f), ls, cs, le, ce)
            out[key] = out.get(key, False) or count > 0
    return out


def measure():
    decl = declared_in_headers()
    if decl is None:
        sys.exit("the oracle checkout is missing (oracle/FreeRTOS-Kernel): run `kairos oracle fetch`")
    fns = {}
    for f in sorted(os.listdir(SRC)):
        if f.endswith(".rs") and f not in NOT_SHIPPED:
            fns[f] = functions(os.path.join(SRC, f))
    cov = {g: regions(g) for g in GROUPS}
    every = set().union(*[set(c) for c in cov.values()])

    def owner(key):
        f, ls = key[0], key[1]
        best = None
        for (name, pub, a, b, doc) in fns.get(f, []):
            if a <= ls <= b and (best is None or b - a < best[3] - best[2]):
                best = (name, pub, a, b, doc)
        return best

    per = {}  # (file, name, start) -> record
    for key in every:
        o = owner(key)
        if o is None:
            continue
        rid = (key[0], o[0], o[2])
        rec = per.setdefault(rid, {"file": key[0], "name": o[0], "pub": o[1], "line": o[2],
                                   "doc": o[4], "regions": [], })
        rec["regions"].append([key[1], key[2]] + [bool(cov[g].get(key)) for g in GROUPS])
    # public functions that recorded NO regions at all were never instantiated
    for f, lst in fns.items():
        for (name, pub, a, b, doc) in lst:
            if pub and (f, name, a) not in per:
                per[(f, name, a)] = {"file": f, "name": name, "pub": True, "line": a,
                                     "doc": doc, "regions": []}
    rows = []
    for rec in per.values():
        m = CNAME.search(rec["doc"]) if rec["doc"].startswith("`") else None
        cname = m.group(1) if m else ""
        twin = "api" if cname and cname in decl else ("state" if cname else "none")
        rs = sorted(rec["regions"])
        n = len(rs)
        # r = [line, col, C1, C1L, C2, ANY]. One core is the pins OR the long
        # runs; "any" is everything, the C-compared runs included.
        one = [r[2] or r[3] for r in rs]
        two = [r[4] for r in rs]
        anyc = [r[2] or r[3] or r[4] or r[5] for r in rs]
        missed = [[r[0], r[1]] for r, a, b in zip(rs, one, two) if not (a or b)]
        rows.append({"file": rec["file"], "name": rec["name"], "pub": rec["pub"], "line": rec["line"],
                     "c": cname, "twin": twin, "regions": n,
                     "c1": sum(one), "c1_pin": sum(1 for r in rs if r[2]),
                     "c2": sum(two), "any": sum(anyc), "missed_by_c": missed})
    rows.sort(key=lambda r: (r["file"], r["line"]))
    meta = {"long": []}
    mp = os.path.join(OUT, "C1L.meta")
    if os.path.isfile(mp):
        for ln in open(mp, encoding="utf-8"):
            name, ticks, lines = ln.split()
            meta["long"].append([name, int(ticks), int(lines)])
    return rows, meta


def verdict(r, col):
    if r["regions"] == 0:
        return "not instantiated"
    k = r[col]
    if k == 0:
        return "**never**"
    if k == r["regions"]:
        return "all %d" % k
    return "%d/%d" % (k, r["regions"])


def render(rows, meta):
    pub = [r for r in rows if r["pub"]]
    api = [r for r in pub if r["twin"] == "api"]
    def count(lst, pred):
        return sum(1 for r in lst if pred(r))
    s = []
    s.append("# API coverage: what the C differential actually judges")
    s.append("")
    s.append("Generated by `tools/api-census` -- **do not edit by hand**; `census.py --check` fails "
             "if this file disagrees with `docs/api-coverage.json`.")
    s.append("")
    s.append("**Method.** LLVM source-based coverage (`-C instrument-coverage`, rustc 1.98.0) over "
             "four groups of runs, each attributed by source line to the innermost function holding "
             "it -- so inlining and generics do not hide anything. A *region* is one straight-line "
             "piece of code; a function's regions are its arms.")
    s.append("")
    s.append("| group | what runs | compared to the C? |")
    s.append("|---|---|---|")
    s.append("| **C1** | `rusty_rtos_demo` `conformance`: all 25 pins -- the 24 scenarios plus `PollQ-typed` "
             "(2,000 ticks; `death` 4,000) -- and the async arm, each against the C kernel's own trace digest and counters | yes, one core |")
    long = meta.get("long", [])
    if long:
        ticks = sorted({t for _, t, _ in long})
        s.append("| **C1L** | `kairos-sim` at full length (%s ticks) on the %d scenarios `kairos conform` "
                 "reported identical to the LIVE C kernel in the same session, %s trace lines in all | "
                 "yes, one core |" % (" / ".join("{:,}".format(t) for t in ticks), len(long),
                                      "{:,}".format(sum(l for _, _, l in long))))
    else:
        s.append("| **C1L** | not run this time (no `kairos conform` log given) | -- |")
    s.append("| **C2** | `smp_conformance` (nine scenarios, 20,000 ticks) + `smp_differential` (two "
             "20,000-step scripts) | yes, two cores |")
    s.append("| **ANY** | every test in the kernel and the demo, both feature sets | no -- *executed*, not *compared* |")
    s.append("")
    s.append("**What a covered region does and does not prove.** A region C1 or C2 executed ran inside "
             "a run whose trace, exits and counters matched the C's -- so what it decided was compared. "
             "It is not yet proof that a WRONG decision there would have been caught; that needs a "
             "poison per API (the plan's P1). The **one core** column is C1 or C1L: the pins are "
             "what CI can re-check without a C toolchain, and the full-length runs are what they miss "
             "-- TimerDemo's Test6, for one, starts after 2,000 ticks.")
    s.append("")
    s.append("## Summary")
    s.append("")
    s.append("| | public functions |")
    s.append("|---|---:|")
    s.append("| in `rusty_rtos_kernel-core` | %d |" % len(pub))
    s.append("| with a C twin (declared in the pinned FreeRTOS headers) | %d |" % len(api))
    s.append("| accessors of C state (named, not declared, in the headers) | %d |" % count(pub, lambda r: r["twin"] == "state"))
    s.append("| Rust-only (no C name on their doc) | %d |" % count(pub, lambda r: r["twin"] == "none"))
    s.append("")
    s.append("| the %d with a C twin | one core (C1, C1L) | two cores (C2) | any run |" % len(api))
    s.append("|---|---:|---:|---:|")
    for label, pred in (
            ("entered", lambda r, c: r[c] > 0),
            ("every arm executed", lambda r, c: r["regions"] > 0 and r[c] == r["regions"]),
            ("never entered", lambda r, c: r[c] == 0)):
        s.append("| %s | %d | %d | %d |" % (label, count(api, lambda r: pred(r, "c1")),
                                             count(api, lambda r: pred(r, "c2")),
                                             count(api, lambda r: pred(r, "any"))))
    s.append("")
    long_only = [r for r in api if r["c1"] > 0 and r["c1_pin"] == 0]
    s.append("Of the one-core column, **%d** C-twinned APIs are entered ONLY by the full-length runs "
             "(C1L), never by the 2,000-tick pins CI re-checks%s" % (
                 len(long_only), (": " + ", ".join("`%s`" % r["name"] for r in long_only) + ".")
                 if long_only else "."))
    s.append("")
    never_c = [r for r in api if r["c1"] == 0 and r["c2"] == 0]
    s.append("### C-twinned APIs no C-compared run enters (%d)" % len(never_c))
    s.append("")
    if never_c:
        s.append("| API | C | file | executed by any test? |")
        s.append("|---|---|---|---|")
        for r in never_c:
            s.append("| `%s` | `%s` | `%s:%d` | %s |" % (r["name"], r["c"], r["file"], r["line"],
                                                      verdict(r, "any")))
    else:
        s.append("None.")
    s.append("")
    one_only = [r for r in api if r["c1"] > 0 and r["c2"] == 0]
    s.append("### C-twinned APIs compared on one core but never on two (%d)" % len(one_only))
    s.append("")
    s.append(", ".join("`%s`" % r["name"] for r in one_only) if one_only else "None.")
    s.append("")
    s.append("## Every public function")
    s.append("")
    s.append("| file | API | C | twin | regions | C1 | C2 | ANY |")
    s.append("|---|---|---|---|---:|---|---|---|")
    for r in pub:
        s.append("| `%s:%d` | `%s` | %s | %s | %d | %s | %s | %s |" % (
            r["file"], r["line"], r["name"], ("`%s`" % r["c"]) if r["c"] else "", r["twin"],
            r["regions"], verdict(r, "c1"), verdict(r, "c2"), verdict(r, "any")))
    s.append("")
    s.append("## Arms no C-compared run executes")
    s.append("")
    s.append("Every function -- public or internal -- with code regions neither C1 nor C2 ran, most "
             "first. Lines are where each unexecuted region starts. This is the target list for the "
             "generative differential (plan P1).")
    s.append("")
    s.append("| function | file | unexecuted / regions | at lines |")
    s.append("|---|---|---:|---|")
    arms = sorted((r for r in rows if r["missed_by_c"]), key=lambda r: (-len(r["missed_by_c"]), r["file"], r["line"]))
    for r in arms:
        lines = sorted({m[0] for m in r["missed_by_c"]})
        shown = ", ".join(str(x) for x in lines[:12]) + (" ..." if len(lines) > 12 else "")
        s.append("| `%s`%s | `%s:%d` | %d / %d | %s |" % (r["name"], "" if r["pub"] else " (internal)",
                                                        r["file"], r["line"], len(r["missed_by_c"]),
                                                        r["regions"], shown))
    s.append("")
    return "\n".join(s)


README = os.path.join(ROOT, "README.md")
BEGIN = "<!-- API-CENSUS:BEGIN (generated by tools/api-census/census.py -- do not edit) -->"
END = "<!-- API-CENSUS:END -->"


def readme_block(rows):
    """The umbrella README's few lines, from the same rows as the doc."""
    api = [r for r in rows if r["pub"] and r["twin"] == "api"]
    one = sum(1 for r in api if r["c1"] > 0)
    two = sum(1 for r in api if r["c2"] > 0)
    one_full = sum(1 for r in api if r["regions"] and r["c1"] == r["regions"])
    two_full = sum(1 for r in api if r["regions"] and r["c2"] == r["regions"])
    return "\n".join([
        BEGIN,
        "| %d public APIs with a FreeRTOS twin | entered by a run compared to the C | every arm executed |" % len(api),
        "|---|---:|---:|",
        "| one core | **%d** | %d |" % (one, one_full),
        "| two cores | **%d** | %d |" % (two, two_full),
        "",
        "The %d never compared on one core, and the %d never compared on two, are named "
        "there; closing them is [`docs/plans/api-differential.md`](docs/plans/api-differential.md)."
        % (len(api) - one, len(api) - two),
        END,
    ])


def splice_readme(text, block):
    a, b = text.index(BEGIN), text.index(END) + len(END)
    return text[:a] + block + text[b:]


def main():
    if "--check" in sys.argv:
        data = json.load(open(DATA, encoding="utf-8"))
        want = render(data["rows"], data["meta"])
        have = open(DOC, encoding="utf-8").read()
        if have != want:
            sys.exit("docs/API-COVERAGE.md is stale or hand-edited: re-run tools/api-census")
        readme = open(README, encoding="utf-8").read()
        if readme != splice_readme(readme, readme_block(data["rows"])):
            sys.exit("README.md's API-CENSUS block is stale or hand-edited: re-run tools/api-census")
        print("API-COVERAGE.md and the README block up to date")
        return
    # A missing group would render as "never entered" everywhere -- the first
    # kill test produced exactly that, from a run that ran nothing.
    for g in ("C1", "C2", "ANY"):
        if not os.path.isfile(os.path.join(OUT, g + ".json")):
            sys.exit("no %s coverage in %s: run tools/api-census/run.sh first" % (g, OUT))
    rows, meta = measure()
    with open(DATA, "w", encoding="utf-8", newline="\n") as f:
        json.dump({"meta": meta, "rows": rows}, f, indent=0, sort_keys=True)
        f.write("\n")
    with open(DOC, "w", encoding="utf-8", newline="\n") as f:
        f.write(render(rows, meta))
    readme = open(README, encoding="utf-8").read()
    with open(README, "w", encoding="utf-8", newline="") as f:
        f.write(splice_readme(readme, readme_block(rows)))
    pub = [r for r in rows if r["pub"]]
    print("%d public functions, %d with a C twin; rendered %s" % (
        len(pub), sum(1 for r in pub if r["twin"] == "api"), DOC))


if __name__ == "__main__":
    main()
