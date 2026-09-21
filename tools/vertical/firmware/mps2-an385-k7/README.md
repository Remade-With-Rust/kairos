# `mps2-an385-k7` — the K7 packages on a Cortex-M3

Six K7 packages — TCP, HTTP, MQTT, JSON, SNTP, backoff — are `no_std` and
build for `thumbv7m-none-eabi`. Every one was proved against C by a
differential, and every one of those differentials ran on x86-64.

This cell asks what they could not: **are the answers the same at half the
pointer width?**

```sh
cd tools/vertical/firmware/mps2-an385-k7
cargo run --release          # builds for thumbv7m and boots it in QEMU
```

Or from the umbrella, `sh tools/vertical/chip.sh`.

## The shape

One battery — `k7-battery` — compiled twice from one source:

| arm | how it runs | what it asserts against |
|---|---|---|
| x86-64 | `cargo test` in `tools/vertical/k7-battery` | `k7_battery::PINNED` |
| ARMv7-M | this cell, in QEMU | `k7_battery::PINNED` |

**The cell holds no expected values of its own.** Both arms compare to the
same constant, which was measured on the host. A cell carrying its own pins
could be re-pinned from a chip run and would then agree with itself forever;
this one cannot.

## Why a mismatch would mean something

`Fnv::push_usize` folds a `usize` as eight **big-endian** bytes rather than
its native bytes. That is deliberate. Folding the native representation would
make the two digests differ on pointer width alone — every run would "fail",
the failure would carry no information, and the instrument would be measuring
`size_of::<usize>()` instead of the code.

With the width normalised, a difference means a length, an offset or a count
came out with a different **value** on a 32-bit machine. That is a real
defect, and no amount of host testing would find it.

## Two things that were checked before the PASS was believed

**1. The chip really executes the battery.** It is a pure function of
literals, so LLVM would be within its rights to fold the whole thing to a
constant at compile time — and it would fold it with the *target's*
semantics, so the printed digest would still be the right number. The chip
would then be reporting its compiler's arithmetic.

Measured: making all 44 input sites opaque with `core::hint::black_box` grew
the ELF by **60 bytes**. Had the battery been folded away, forcing six
parsers back into the image would have cost kilobytes. So the code was
already there and running — and `black_box` now keeps it that way.

**2. The comparison can fail.** Two poisons, each run on the chip:

| poison | effect | result |
|---|---|---|
| one pin altered | `tcp` pin set to a wrong value | `FAIL tcp`, exit 1 |
| one input byte altered | `IP_HEADER[0]` `0x45` → `0x46` | `tcp` digest moved to `0x459e…`, `ALL` moved too, exit 1 |

The second is the load-bearing one: the chip computed a *different* digest
from the changed input, which is only possible if it is genuinely running the
checksum over the data.

## What this does not cover

The `std`-only surface — the socket layer's blocking loop and the smoltcp
engine — is not here, because it cannot be. That surface is proved instead by
the interop rigs in `tools/vertical`, against foreign stacks on a
workstation. Nothing here measures timing, stack depth, or interrupt
behaviour.

## Re-pinning

If a K7 package changes behaviour the digests move, and they are meant to.
Re-pin from the host, then re-run the chip:

```sh
cd tools/vertical/k7-battery && cargo test   # fails, printing both values
# update PINNED in src/lib.rs, then:
cd ../firmware/mps2-an385-k7 && cargo run --release
```

What must never happen is the two arms disagreeing.
