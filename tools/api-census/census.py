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
GROUPS = ("C1", "C1L", "C2", "EQ", "ANY")
REASONS = os.path.join(ROOT, "tools", "api-census", "reasons.json")
# The typed face: the only file the wrapper-equivalence test (EQ) can judge.
WRAPPERS = "typed.rs"
# H2's five: Rust spellings with no C twin to differ from. They keep their
# unit tests (the ANY column); the census names them so nobody counts them
# as a gap again.
DIAGNOSTICS = {"task_at", "ready_cursor", "ready_items", "with_tick_hook", "notify_value"}

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


# Plan decision D1: an arm that only ever answers an ERROR -- a stale handle, a
# refused argument -- has no C answer to compare against, so the C-compared runs
# cannot judge it and it is not a gap in them. A region counts as such ONLY when
# its code begins with an error construct; anything else stays a gap. The test
# can miss contract arms (an `Ok(false)` refusal, say) but cannot call a real arm
# one, so the column it feeds can understate and never overclaim.
CONTRACT = re.compile(r"^(\?|Err\(|return Err\b|Err\(e\) =>)")
# Three more shapes that do nothing but hand an error back, found by P3's
# reading of what the first test left: an arm that passes an error through
# (`Err(e) => Err(e),`, whose region LLVM starts inside the pattern), a
# section's exit whose next statement returns an error, and an `Err(e) => {`
# block that holds only those two. Anything else in the block keeps it a gap.
PASSTHROUGH = re.compile(r"^Err\([a-z_]\w*\) => Err\(")
LEAVE_ONLY = re.compile(r"^(self\.exit_critical\(\);|return Err\(.*\);)$")


def is_contract(src, f, line, col):
    text = src.get(f)
    if text is None:
        text = src[f] = open(os.path.join(SRC, f), encoding="utf-8").read().splitlines()
    if not 0 < line <= len(text):
        return False
    here = text[line - 1][col - 1:].lstrip()
    whole = text[line - 1].strip()
    if CONTRACT.match(here) or PASSTHROUGH.match(whole):
        return True
    rest = [t.strip() for t in text[line:line + 6] if t.strip()]
    if whole == "self.exit_critical();" and rest and rest[0].startswith("return Err("):
        return True
    if re.match(r"^Err\([a-z_]\w*\) => \{$", whole):
        body = []
        for t in rest:
            if t == "}" or t == "},":
                break
            body.append(t)
        return bool(body) and all(LEAVE_ONLY.match(t) for t in body)
    return False


def load_reasons():
    """tools/api-census/reasons.json: the arms no C-compared run executes,
    each with a written reason. An entry names a function and the text the
    arm's source line begins with -- every region LLVM cuts from that line
    shares it; `after`, if given, must appear on one of the three lines above
    (for a bare `}`, which only means something by what it closes)."""
    if not os.path.isfile(REASONS):
        return {"classes": {}, "arms": []}
    return json.load(open(REASONS, encoding="utf-8"))


def reason_for(reasons, src, f, name, line, col):
    text = src[f]
    here = text[line - 1].strip() if 0 < line <= len(text) else ""
    above = " ".join(text[max(0, line - 4):line - 1])
    for i, e in enumerate(reasons["arms"]):
        if e["file"] != f or e["fn"] != name:
            continue
        if not here.startswith(e["text"]):
            continue
        if e.get("after") and e["after"] not in above:
            continue
        return i
    return None


def measure():
    decl = declared_in_headers()
    reasons = load_reasons()
    used = set()
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
    src = {}
    for rec in per.values():
        m = CNAME.search(rec["doc"]) if rec["doc"].startswith("`") else None
        cname = m.group(1) if m else ""
        twin = "api" if cname and cname in decl else ("state" if cname else "none")
        rs = sorted(rec["regions"])
        n = len(rs)
        # r = [line, col, C1, C1L, C2, EQ, ANY]. One core is the pins OR the
        # long runs; "any" is everything, the C-compared runs included.
        one = [r[2] or r[3] for r in rs]
        two = [r[4] for r in rs]
        eq = [r[5] and rec["file"] == WRAPPERS for r in rs]
        anyc = [r[2] or r[3] or r[4] or r[5] or r[6] for r in rs]
        missed = [[r[0], r[1]] for r, a, b in zip(rs, one, two) if not (a or b)]
        contract = [is_contract(src, rec["file"], r[0], r[1]) for r in rs]
        # P3: an arm no C-compared run executed is still JUDGED if it is
        # contract-only, or the equivalence test ran it, or it carries a
        # written reason. Reasons are looked up only for arms that need one,
        # so a reason that matches nothing unjudged is stale (checked below).
        why = []
        for r, a, b, k, e in zip(rs, one, two, contract, eq):
            if a or b or k or e:
                why.append(None)
                continue
            i = reason_for(reasons, src, rec["file"], rec["name"], r[0], r[1])
            if i is not None:
                used.add(i)
            why.append(i)
        rows.append({"file": rec["file"], "name": rec["name"], "pub": rec["pub"], "line": rec["line"],
                     "c": cname, "twin": twin, "regions": n,
                     "c1": sum(one), "c1_pin": sum(1 for r in rs if r[2]),
                     "c2": sum(two), "any": sum(anyc), "missed_by_c": missed,
                     # D1: compared OR contract-only, per column
                     "c1k": sum(1 for o, k in zip(one, contract) if o or k),
                     "c2k": sum(1 for o, k in zip(two, contract) if o or k),
                     "contract_missed": sum(1 for r, a, b, k in zip(rs, one, two, contract)
                                            if k and not (a or b)),
                     "eq": sum(1 for e in eq if e),
                     "eq_missed": sum(1 for a, b, k, e in zip(one, two, contract, eq)
                                      if e and not (a or b or k)),
                     "reasoned": sorted({reasons["arms"][i]["class"] for i in why if i is not None}),
                     "reasoned_missed": sum(1 for i in why if i is not None),
                     "unjudged": [[r[0], r[1]] for r, a, b, k, e, i in
                                  zip(rs, one, two, contract, eq, why)
                                  if not (a or b or k or e) and i is None],
                     "diagnostic": rec["name"] in DIAGNOSTICS and rec["pub"]})
    rows.sort(key=lambda r: (r["file"], r["line"]))
    stale = [e for i, e in enumerate(reasons["arms"]) if i not in used]
    if stale:
        sys.exit("reasons.json: %d reason(s) match no unjudged arm -- the arm is judged now, or "
                 "its code moved: %s" % (len(stale), "; ".join("%s %s `%s`" % (e["file"], e["fn"], e["text"])
                                                                for e in stale)))
    meta = {"long": [], "classes": reasons["classes"],
            "reasons": [{"file": e["file"], "fn": e["fn"], "text": e["text"], "class": e["class"]}
                        for e in reasons["arms"]]}
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
             "five groups of runs, each attributed by source line to the innermost function holding "
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
    s.append("| **EQ** | `typed_equivalence`: every typed wrapper on a real kernel, its calls recorded at the `Raw` "
             "boundary and replayed directly on a twin kernel (plan P4) | no -- each wrapper proved to make exactly "
             "the C-twinned calls C1/C2 compare; credited to `typed.rs` only |")
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
            ("every arm executed, or contract-only (D1)",
             lambda r, c: r["regions"] > 0 and r.get(c + "k", r[c]) == r["regions"]),
            ("never entered", lambda r, c: r[c] == 0)):
        s.append("| %s | %d | %d | %d |" % (label, count(api, lambda r: pred(r, "c1")),
                                             count(api, lambda r: pred(r, "c2")),
                                             count(api, lambda r: pred(r, "any"))))
    s.append("")
    s.append("*Contract-only* (plan decision D1): a region whose code begins with an error "
             "construct (`?`, `Err(`, `return Err`), passes an error through (`Err(e) => Err(e)`), "
             "or only leaves the critical section to return one -- a stale handle, a refused "
             "argument -- has no C answer to compare against. The test is deliberately narrow: it can leave a "
             "contract arm counted as a gap, never the reverse. Such arms are judged by the "
             "kernel's own tests, which the *any run* column counts.")
    s.append("")
    judged = [r for r in api if r["regions"] > 0 and not r.get("unjudged")]
    unjudged = [r for r in api if r["regions"] == 0 or r.get("unjudged")]
    s.append("**Every arm judged (plan P3): %d of the %d.** An arm is judged when a C-compared run on "
             "either build executed it, or it is contract-only (D1), or the wrapper-equivalence test "
             "ran it (EQ, `typed.rs`), or it carries a written reason in `tools/api-census/reasons.json` "
             "(below)%s" % (len(judged), len(api),
                            (". Unjudged: " + ", ".join("`%s`" % r["name"] for r in unjudged) + ".")
                            if unjudged else "; none is left without one."))
    s.append("")
    diags = [r for r in pub if r.get("diagnostic")]
    s.append("**Diagnostics (H2), %d:** %s -- Rust spellings with no C call to differ from; they keep "
             "their unit tests (the *any run* column) and are not gaps." % (
                 len(diags), ", ".join("`%s`" % r["name"] for r in diags)))
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
            r["file"], r["line"], r["name"], ("`%s`" % r["c"]) if r["c"] else "",
            r["twin"] + (" (diagnostic)" if r.get("diagnostic") else ""),
            r["regions"], verdict(r, "c1"), verdict(r, "c2"), verdict(r, "any")))
    s.append("")
    s.append("## Arms no C-compared run executes")
    s.append("")
    s.append("Every function -- public or internal -- with code regions neither C1 nor C2 ran, most "
             "first. Lines are where each unexecuted region starts. This is the target list for the "
             "generative differential (plan P1).")
    s.append("")
    s.append("| function | file | unexecuted / regions | contract-only | EQ | reasoned | **unjudged** | unjudged at lines |")
    s.append("|---|---|---:|---:|---:|---:|---:|---|")
    arms = sorted((r for r in rows if r["missed_by_c"]),
                  key=lambda r: (-len(r.get("unjudged", [])), -len(r["missed_by_c"]), r["file"], r["line"]))
    for r in arms:
        lines = sorted({m[0] for m in r.get("unjudged", [])})
        shown = ", ".join(str(x) for x in lines[:12]) + (" ..." if len(lines) > 12 else "")
        s.append("| `%s`%s | `%s:%d` | %d / %d | %d | %d | %d | %d | %s |" % (
            r["name"], "" if r["pub"] else " (internal)", r["file"], r["line"], len(r["missed_by_c"]),
            r["regions"], r.get("contract_missed", 0), r.get("eq_missed", 0), r.get("reasoned_missed", 0),
            len(r.get("unjudged", [])), shown))
    s.append("")
    classes = meta.get("classes", {})
    if classes:
        s.append("## Arms judged by a written reason")
        s.append("")
        s.append("Each reason is an entry in `tools/api-census/reasons.json`, keyed by function and the "
                 "text its arm begins with; the census refuses a reason that no longer matches an "
                 "unjudged arm, so a reason cannot outlive its code.")
        s.append("")
        s.append("| class | what it means | entries | functions |")
        s.append("|---|---|---:|---|")
        for c in sorted(classes):
            fns = sorted({r["name"] for r in rows if c in r.get("reasoned", [])})
            n = sum(1 for e in meta.get("reasons", []) if e["class"] == c)
            s.append("| %s | %s | %d | %s |" % (c, classes[c], n, ", ".join("`%s`" % f for f in fns)))
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
    judged = sum(1 for r in api if r["regions"] and not r.get("unjudged"))
    return "\n".join([
        BEGIN,
        "| %d public APIs with a FreeRTOS twin | entered by a run compared to the C | every arm executed |" % len(api),
        "|---|---:|---:|",
        "| one core | **%d** | %d |" % (one, one_full),
        "| two cores | **%d** | %d |" % (two, two_full),
        "",
        "**Every arm of %d of the %d is judged** -- compared on either build, contract-only, "
        "proved by the typed face's equivalence test, or carrying a written reason the census "
        "checks. The %d never compared on one core, and the %d never compared on two, are named "
        "in [`docs/API-COVERAGE.md`](docs/API-COVERAGE.md); the mission is "
        "[`docs/plans/api-differential.md`](docs/plans/api-differential.md)."
        % (judged, len(api), len(api) - one, len(api) - two),
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
