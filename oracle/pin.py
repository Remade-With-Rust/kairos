#!/usr/bin/env python3
"""A corpus pin from a C oracle trace: what `rusty_rtos_demo-core`'s pin tables
hold (`src/pins.rs` for one core, `tests/smp_conformance.rs` for two).

    python3 oracle/pin.py <trace>...          # the C's stderr, verdict line last

Prints, per trace: the verdict line's counters, the trace lines before it,
their bytes, and their FNV-1a/64 digest -- the same digest
`rusty_rtos_demo_core::pins::Digest` takes of the Rust trace as it is written.
Stdlib only.
"""
import sys


def pin(path):
    text = open(path, "rb").read().replace(b"\r\n", b"\n")
    lines = text.split(b"\n")
    if lines and lines[-1] == b"":
        lines.pop()
    verdict = lines.pop() if lines and lines[-1].startswith(b"KAIROS_RESULT") else b""
    body = b"".join(line + b"\n" for line in lines)
    h = 0xCBF29CE484222325
    for byte in body:
        h ^= byte
        h = (h * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    fields = dict(f.split(b"=", 1) for f in verdict.split()[3:] if b"=" in f)
    return {
        "verdict": verdict.decode(errors="replace"),
        "ticks": int(fields.get(b"ticks", b"0")),
        "yields": int(fields.get(b"yields", b"0")),
        "exits": int(fields.get(b"exits", b"0")),
        "lines": len(lines),
        "bytes": len(body),
        "digest": "0x%016x" % h,
    }


if __name__ == "__main__":
    for p in sys.argv[1:]:
        r = pin(p)
        print("%s ticks=%d yields=%d exits=%d lines=%d bytes=%d digest=%s"
              % (p, r["ticks"], r["yields"], r["exits"], r["lines"], r["bytes"], r["digest"]))
