# `tools/vertical` — where the K7 packages meet something that is not ours

Each K7 package is proved on its own, against its own pinned C oracle, by its
own differential. That is the strongest evidence in this repo and it has a
shape of hole in it: **every one of those differentials runs one package, on
x86-64, against a C program that ships in the same tarball.**

This directory holds the three things that close the hole.

| | asks | needs |
|---|---|---|
| **composition** (`tests/`) | does package A work *over* package B? | nothing; `cargo test` |
| **interop** (`*.sh`) | does it work against a stack nobody here wrote? | root, `/dev/net/tun`, WSL |
| **the chip** (`chip.sh`) | are the answers the same at 32 bits? | QEMU + `thumbv7m-none-eabi`, git bash |

## Why it lives in the umbrella and not in a package

`rusty_rtos_http`, `rusty_rtos_tcp` and `rusty_rtos_mqtt` are all
**unpublished**. A path dependency between two unpublished packages breaks
hardening gate **H-07**: every package's checked-in `Cargo.lock` must resolve
in a fresh clone, and a lock naming a sibling by path does not.

The umbrella has no such obligation — it is never published and never cloned
alone — so the cross-package wiring lives here. This is a constraint being
respected, not a workaround.

## Composition — `cargo test`

```sh
cd tools/vertical && cargo test
```

* `tests/http_over_tcp.rs` — `HTTPClient_Send` completing over the TCP
  package's socket surface, on a real connection.
* `tests/mqtt_over_tcp.rs` — coreMQTT's CONNECT going out in its **vectored**
  pieces (12, 1, 5, 2, 15 bytes) through a real socket. Neither package was
  written to exercise that path; the seam is what makes it happen.

## Interop — against foreign stacks

All four rigs need **root** and `/dev/net/tun`, so they run under WSL (which
here runs as root). Each makes its own TAP device on its own subnet, so they
can run at the same time without colliding. Everything they create, they
remove.

| script | TAP | subnet | the far side |
|---|---|---|---|
| `interop.sh` | `kairos0` | 192.168.69.0/24 | the Linux kernel's TCP + `python3 -m http.server` |
| `mqtt-interop.sh` | `kairos1` | 192.168.70.0/24 | mosquitto 2.0.22 (a third-party C broker) |
| `throughput.sh` | `kairos2` | 192.168.71.0/24 | the Linux kernel's TCP, as a sink and a source |
| `rumqttd-interop.sh` | `kairos3` | 192.168.72.0/24 | rumqttd — the broker the kill test NAMES |

```sh
sudo sh tools/vertical/interop.sh
sudo SECONDS_HELD=3600 sh tools/vertical/rumqttd-interop.sh
sudo BYTES=268435456 REPS=3 sh tools/vertical/throughput.sh
```

Two things these rigs do that are worth copying:

**They state their substitutions in their own output.** When the broker was
mosquitto rather than the named rumqttd, the script said so on every run
rather than leaving it in a commit message. A substitution a reader has to go
looking for is one they will not find.

**They check work parity every repetition.** `throughput.sh` has the peer
count the bytes it actually saw and refuses to report a rate unless both ends
agree. A throughput figure computed over a transfer that silently truncated
is the classic way to get a fast wrong answer.

### rumqttd, and a correction

`cargo install rumqttd --locked` fails with E0521 in its `metrics`
dependency. **Without `--locked` it installs and runs.** The published
lockfile is what is broken, not the toolchain — and this repo briefly
recorded the opposite as a measured fact, which is corrected in
`rusty_rtos_tcp/docs/LEDGER.md`. rumqttd is reached on its **v5** listener
(port 1884), because this client speaks MQTT 5 and a v5 CONNECT at a v4
listener is a protocol refusal rather than a stack problem.

## The chip — `chip.sh`

```sh
sh tools/vertical/chip.sh      # git bash, NOT WSL
```

QEMU and the `thumbv7m-none-eabi` target are installed for the **Windows**
toolchain; WSL has neither. The failure looks like a missing target rather
than a wrong shell, which is why it is said twice.

One battery — `k7-battery` — compiled from one source for two architectures,
both asserting against one pin table measured on the host:

* `k7-battery/` — `no_std`, no allocator, six packages exercised.
* `firmware/mps2-an385-k7/` — the same battery on a Cortex-M3.

`firmware/mps2-an385-k7/README.md` has the full argument: why the digests are
comparable across pointer widths at all, the measurement showing the chip
really executes the battery rather than printing a constant its compiler
folded, and the two poisons proving the gate can fail.

## What none of this proves

Not timing, not stack depth, not interrupt behaviour, and nothing about the
`std`-only surface on a chip — the socket layer's blocking loop and the
smoltcp engine are proved by the interop rigs on a workstation, not by the
chip cell. A userspace poll loop over a TAP device is **not** a NIC on a
part; the throughput rows bound nothing about embedded performance and must
never be quoted as if they did. They are a baseline a later change can be
measured against, and evidence that bulk transfer works at all rather than
only forty-byte requests.
