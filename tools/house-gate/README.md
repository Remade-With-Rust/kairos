# tools/house-gate — the house-stack compile gate

The question `docs/HOUSE-STACK.md` answers is not "does the house have a crate
for X" but "does that crate build where Kairos builds": `no_std`, on a
Cortex-M4F and an RV32 with no 64-bit atomics, at the pin the house names,
under this family's `deny.toml`. This workspace is that question as a command.

One member per house crate, each a `#![no_std]` shell that uses one item of
the crate, so a failure names its crate and nothing else:

| member | crate | pin |
|---|---|---|
| `gate-alloc` | `rusty_alloc-api` | `=2.0.4`, `default-features = false` (needs the two cfgs below) |
| `gate-symbols` | `rusty_symbols` | `=0.1.0`, `default-features = false` |
| `gate-thoth` | `thoth` | git tag `v0.3.0`, `default-features = false` |
| `gate-json` | `rusty_json_turbo` (lib `serde_json`) | git, `0.1.0`, `no_std + alloc` |
| `gate-zstd` | `rusty_zstd` | `=0.2.5`, `no_std + alloc` |
| `gate-erasure` | `rusty_erasure-core` | `=0.4.0` |
| `gate-time` | `rusty_time-core` | `=0.1.10` |
| `gate-xml` | `rusty_xml` | `=0.8.1` |

```sh
cd tools/house-gate
for g in symbols thoth json zstd erasure time xml; do
  for t in thumbv7em-none-eabihf riscv32imac-unknown-none-elf; do
    cargo check -p gate-$g --target $t && echo "gate-$g @ $t: OK" || echo "gate-$g @ $t: FAIL"
  done
done
RUSTFLAGS="--cfg ra_single_threaded --cfg ra_small_profile" \
  cargo check -p gate-alloc --target thumbv7em-none-eabihf
cargo deny check                       # the Kairos policy over the whole graph
(cd host && cargo check && cargo deny check)             # SpaceDB, FFAI, rusty_time, rusty_xml, rusty_erasure, rusty_zstd on the host
(cd hostgit && CARGO_NET_GIT_FETCH_WITH_CLI=true cargo check)   # remade-ffmpeg (rff) and rmap-core by git URL
```

`host/` is a second workspace for the crates that only make sense on a host
(`spacedb-sdk`, `ffai-core`, and the `std` builds of the others); `hostgit/`
a third for the two that come by git URL (`remade-ffmpeg`, the `rff` facade,
`default-features = false`; `rmap-core` from the private `rusty_maps`, which
needs `CARGO_NET_GIT_FETCH_WITH_CLI=true` so the gh credentials apply). Both
compile with `cargo check` on the host (26 s and 40 s on 2026-09-10): nothing
in an RTOS links them, but the pins are proven, not read.

**Each carries its own `[workspace]` table, and must.** They are packages
inside this directory, and this directory's workspace says
`members = ["gate-*"]`, which does not match them -- so without that table
cargo walks up, finds a workspace they are not members of, and refuses to
build at all. That is exactly what it did until 2026-09-10, and the two
rungs were silently unrunnable while this README said they passed.
**A rung nobody runs is not a gate**; re-run all three workspaces when a
pin moves, not just this one.

`host` also fails `cargo deny check advisories` on three `unmaintained`
crates arriving through `spacedb-sdk` and `ffai-core` -- see
`docs/HOUSE-STACK.md`. None is a vulnerability and none is in a bare-metal
graph, which is why the gate workspace itself is clean.

Verdicts and the date they were taken: `docs/HOUSE-STACK.md`. Re-run the gate
when a pin moves, and move the pin in one commit with the verdict.
