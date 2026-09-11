# The Master Plan — one stack, from the interrupt vector to the marketplace

**Audience: hardware engineers.** This is the document that says what runs on
what silicon, which seams you build to, and why the board on your desk
matters to a business model four layers above it.

**Status:** written 2026-09-10, from the MATA plans, the Janus plan, the
Kairos mission plan, and the two `claude_perspective_of_mata` documents.
Every claim here is sourced; where a plan does not exist yet, this says so
rather than inventing it.

---

## 0. The one-paragraph version

Amazon acquired FreeRTOS in 2017, relicensed it to MIT and gave it away,
because they own AWS IoT Core: the kernel was an on-ramp to a cloud they
meter. **We are making the same move with one difference that changes
everything — we own the cloud too, and ours cannot read the data.** Disco is
"our distributed cloud services portal meant to look like AWS portal"; MATA
is a storage-and-compute marketplace whose impossible cold start is solved by
shipping a sovereign vault people would run anyway; Janus is the device
layer; Kairos is the kernel underneath it. The portfolio already owns every
layer from the codec to the cloud. **The scheduler was the last piece we
rented, and it was written in C.**

---

## 1. The vertical, in silicon

Four tiers of hardware. A hardware engineer works on one of them and needs to
know the other three exist.

```
  TIER 1   SENSOR / ACTUATOR            ESP32-C6, ESP32-S3, ESP32-CAM
           Janus firmware                no_std + esp-hal, or std + ESP-IDF
           KAIROS kernel  ◄── the work in this repo
                │
                │  iroh (ours) · Matter/Thread · MQTT · ESP-NOW · BLE
                ▼
  TIER 2   THE BOX                       NixOS appliance, 1–3 drives, opt. GPU
           Home Computer                 IoT hub + disco node + Asterinas
                │                        one signed image, capability-gated
                │  iroh fabric, roster-authenticated
                ▼
  TIER 3   THE MESH                      iroh P2P, bridge relay fallback
           MATA Network                  no custodian, ciphertext only
                │
                ▼
  TIER 4   THE MARKETPLACE               disco — storage + compute, metered
           $MATA / attestation           looks like AWS, cannot read you
```

The identity spine — `did:mata` / mID — runs **vertically through all four**.
It is the same primitive at every tier: a key the user holds, a claim signed
by it, and an attestation anyone can verify without asking us.

---

## 2. Two classes of device, and why Kairos exists

This is the distinction that decides what you are building.

| | **A peripheral** | **A node** |
|---|---|---|
| speaks | Matter / Thread / MQTT | iroh, with `did:mata` |
| identity | commissioned by the hub | its own, self-issued |
| the hub | polls it, owns its state | routes for it, vouches for it |
| trust | inherited from the owner | carried by the device |
| kernel | vendor's, usually C FreeRTOS | **Kairos** |

Phase 2 of the Home Computer mission ships the **peripheral** path: a Thread
border router, `rumqttd` for DIY devices, device state landing in
`DataType::HomeIotState`, automation evaluated locally with no cloud
round-trip. That is the substrate for *other people's* hardware, and it is
deliberately open-protocol — the plan defers Nest/Ring/Hue to a later
workstream.

**Kairos is the kernel of the second class.** A Janus device is not a thing
the hub polls; it is a mesh member with keys, able to originate signed data
and prove where it came from. A Matter bulb cannot do that — not because of
its radio, but because nothing in its stack carries an identity.

That is the answer to "why write a kernel at all". Not speed, not even memory
safety on its own: **a device that is a node needs a kernel that can carry an
identity, and we were renting that kernel from a C codebase whose steward
sells the opposite architecture.**

---

## 3. The three laws — the symphony

Four teams, four vocabularies, the same three rules. The Home Computer
mission states them as Architectural Posture; Kairos has been enforcing them
under different names. Wherever you work, these are the laws.

### Law 1 — The box is sacrificial

> *"No paired device's identity or data is load-bearing on any single Home
> Computer."*

**Tier 2** — a box that dies is replaced by the replacement-Node flow; state
syncs to every sibling via Y-CRDT.
**Tier 1** — the same law says a sensor must not be the only place its
reading exists, and that a device is replaceable without re-provisioning the
home.
**Kairos** — the oracle is external. The kernel's correctness is not asserted
by the kernel; it is diffed against the C kernel's own trace. Nothing is
load-bearing on our own opinion of ourselves.

### Law 2 — One stack, capability-gated

> *"One software stack across every hardware configuration. The install
> pipeline detects what's present and the daemon enables only the service
> tiers the hardware can sustain — nothing silently degrades or silently
> absents itself."*

**Tier 2** — `mata-hw-survey` writes `/etc/mata/install-tier.json`; tiers
`one`/`two`/`three` gate CloudShard replication and Asterinas intake; the IoT
tab does not appear until the HAL reports a Thread radio; a drive that
vanishes at boot produces "Drive Missing — Cloud Sharing Paused", never a
silent downgrade.
**Tier 1** — the same discipline is the firmware's: one kernel, one corpus,
and the *geometry* declared rather than assumed. Kairos's `system!` macro
derives storage from the declaration; a priority the config has no ready list
for is a `const` assertion, not a runtime surprise.
**Both** — a capability you cannot detect is a capability you must not claim.

### Law 3 — No silent failures

> *"Every per-entry failure surfaces."*

This is the law this project has paid for most often, and the corollary is
sharper than the law:

> **A check that cannot fail is worse than no check, because it reports
> success.**

Six were caught in a single day's work on Kairos, five of them ours: a grep
that matched the wrong verb; a reclamation test reading the same number on
both sides; a region probe that answered zero for every input; a "compiles
without alloc" rung that a `Box::new` sailed through; a symbol scan of an
LTO'd binary that declared a cell heap-free while it carried a global
allocator; and a benchmark that profiled a stale binary and reported an
instruction count identical to the digit across a real change.

**The practice that catches them is one line: before you quote a check, make
it fail on purpose.** Every gate in Kairos is poison-proven — the M3 context
switch by deleting `stmdb r0!, {r4-r11}`, the corpus by moving one pinned
`exits` by one, the no-alloc gate by adding a `Box`. If you cannot describe
the failure you induced, you have not got a gate.

---

## 4. The four movements

### I. The Kernel — Kairos (this repo)

**Silicon:** any. Proven on x86-64 host, Cortex-M3 (QEMU `mps2-an385`), RV32
(QEMU `virt`), ESP32-S3 (real).

**What it is:** FreeRTOS remade in memory-safe Rust — scheduler, IPC, timers,
heaps — `forbid(unsafe)` everywhere except a fenced context switch per port.

**Where it stands:** K0–K4 passed. All 17 corpus scenarios produce traces
**byte-identical to the C kernel** on three architectures. The kernel
allocates nothing, proven on the linked rlib. `heap_4` is diffed against
FreeRTOS's own `heap_4.c` for 20,000 operations. First silicon cycle numbers
taken; `rusty_alloc` beats `heap_4` by 2.4× below 256 bytes and 32× against a
fragmented free list.

**What a hardware engineer owns here:** the ports. `-cortex-m` exists and
switches. `-xtensa` and `-riscv` do not. The Xtensa one is the long pole —
LX7 register windows must be spilled, which is materially harder than
Cortex-M's PendSV.

**The commercial point, stated plainly:** "memory-safe RTOS" is a claim
anyone with capital can buy. *"Byte-identical scheduling decisions to the
kernel you already shipped, proved against its own trace on three
architectures, with the diff checked in"* is the claim that answers the only
question a buyer with certified firmware actually asks — **what breaks if I
swap this.** The corpus is not QA. It is the wedge.

### II. The Device — Janus

**Silicon in hand:** XIAO ESP32-S3 Sense, AI-Thinker ESP32-CAM.
**Silicon on order:** ESP32-C6-DevKitC-1 ×2, HLK-LD2410C, SX1262 ×2, later
ESP32-P4 and C61.

**Two tracks, and the split matters:**

- **Track A** — `std` on ESP-IDF. Espressif's C FreeRTOS fork underneath.
  Most XIAO S3 firmware is here (`-idf-` in the name). **Kairos does nothing
  here in v1** — replacing the kernel under IDF's C drivers is a C-ABI
  project, K8+.
- **Track B** — `no_std` + esp-hal. This is where Kairos lands.

**The structural finding that decides K5:** every Janus firmware using
`esp-rtos`/`esp-radio` is a **C6** firmware — and we have no C6. On the S3
there are exactly two Track B firmwares, `rusty_esp_dsp/xiao-s3-probe` and
`rusty_esp_mid/xiao-s3-keys`, and **neither has a scheduler at all**. They are
bare-metal main loops.

So the Janus joint has two halves that are blocked differently, and the
milestone as written pairs them:

| half | needs | state |
|---|---|---|
| XIAO S3 Xtensa cell | S3 + an Xtensa port | **board in hand — doable** |
| Janus S1 on a Kairos kernel | 2× C6, *and* an esp-rtos baseline | **blocked twice** |

Blocked twice because **Janus S1 is unchecked in Janus's own plan** — the
esp-rtos numbers we are told to match do not exist yet. Buying C6s does not
unblock it; Janus must run S1 on esp-rtos first, and that baseline must be
taken **before** the kernel is swapped.

### III. The Box — Home Computer

**Silicon:** commodity x86-64, 1–3 NVMe/SATA drives, optional discrete GPU,
optional Thread radio (M.2 or a $29 USB dongle), TPM.

**What it is:** one signed NixOS image, three drive tiers, capability-gated.
It is simultaneously the **IoT hub** (Matter/Thread border router, MQTT
broker, local automation), the **disco node** (the third drive is the
cloud-shard), and the **compute host** (Asterinas microVMs, internal
validation jobs now, buyer intake when the network activates).

**What a hardware engineer owns here:** `mata-hw-survey` and the tier
selection — NVMe count and sizes, GPU vendor and count, Thread radio
detection, TPM version. The rule is Law 2: the survey is deterministic and
unit-tested, the same hardware always recommends the same tier, and an
operator override is logged with its rationale.

**The seam to Tier 1:** `DataType::HomeIotState` for peripherals; the iroh
fabric for nodes. IoT data lands in **user scope and never touches the
cloud-shard or a third-party compute job** — that is a hard boundary, not a
policy.

### IV. The Cloud — disco / MATA Network

**What it is:** the marketplace. Storage first, compute second, priced
against AWS, with the one property AWS structurally cannot ship: **we
physically cannot read the data.** That is not a privacy stance, it is the
precondition for selling to regulated buyers who cannot put data on a
custodial cloud.

**Honest gap:** the Phase 2 Linux mission defers proof-of-storage, erasure
coding, node mode, reputation, the `$MATA` ledger and the Friend-Storage
wedge to a *MATA Network mission* — **and that file does not exist yet.** It
is referenced but unwritten. Every claim in this section about tokenomics,
settlement or proof-of-retrievability is therefore **inferred, not planned**,
and should be read that way until that mission is written.

---

## 5. Why the cold start works — and why it is the same trick twice

MATA solves an impossible problem: *why would a normal person run a storage
node?* Filecoin, Sia and Storj all answered "pay them", and the economics
never closed, because the real cost of running a node is **attention**.
MATA's answer is that the user is already running the vault to manage their
passwords; the node is a byproduct. That is BitTorrent's trick — seeding as a
side effect of downloading — with the payment layer BitTorrent never had.

**Kairos has the identical cold-start problem and the identical answer.**
Nobody adopts a new RTOS on a memory-safety argument; switching costs in
embedded are brutal, and that installed base *is* FreeRTOS's moat. But we do
not need anyone to adopt it, because **we already ship the devices.**

> **Janus is to Kairos what the vault is to the node network.**

That makes the Janus joint not an integration milestone but the cold-start
engine, and it is the first place the vertical is actually vertical.

### The risk, in MATA's own words

> *"The stack is deep but the wedge is light… the deep work keeps eating the
> surface work."*

That is a precise description of where Kairos stands. The failure mode is
concrete: spend weeks on the Xtensa register-window context switch — hard,
deep, genuinely satisfying — and still have **no Janus device running our
kernel**. A very good kernel that has closed no seam.

**So invert the build order.** The first Xtensa deliverable is a cell running
a real Janus workload with the smallest port that supports it — not a
complete port looking for a consumer afterwards.

---

## 6. The critical path

**Target: `rusty_esp_mid/firmware/xiao-s3-keys`.** Not because it is
available, but because it is where the vertical is shortest: sensor → kernel
→ identity → mesh, with no C and no custodian in the line. It is Track B, on
the board in hand, it is the *identity* firmware — `did:mata` touching
silicon — and it already carries a measured baseline (*"M1 on real silicon:
what a P-256 signature costs an ESP32-S3"*). It has no scheduler, so a kernel
is **additive**: the firmware gains concurrency rather than being rewritten.

1. **Task deletion in the kernel.** `SchedulerImplementation::
   schedule_task_deletion` requires it, `death.c` requires it, and it closes
   a measured five-mutant coverage gap. One feature, three payoffs.
2. **`rusty_rtos_port-xtensa`, context switch only.** Gated exactly as the
   Cortex-M one was: two tasks, real stacks, N/N resumptions, each
   re-checking every word of its own stack and a callee-saved register,
   poison-proven by deleting the register spill.
3. **The kernel drives a real Janus firmware on the S3.** Take the P-256
   baseline first; the kernel must cost nothing against a number that already
   exists.
4. **The `esp-radio-rtos-driver` traits** over the kernel — four traits, and
   our kernel already has queues, semaphores, timers, delays and yields.
5. **Janus S1** — parked on hardware *and* on Janus running S1 on esp-rtos.

### Three things to settle before any code

- **Pin the spec.** The Kairos plan names `esp-radio-rtos-driver` **0.4.1**;
  this machine has **0.3.0**. Lock it before designing against trait shapes.
  (ADR 0005: lock the spec before implementations multiply.)
- **Build a seam, not a parallel stack.** Janus consumes
  `rusty_rtos_port-xtensa` through one crate that owns the pins — the way
  `mata-alloc` wraps `rusty_alloc`. Janus pins esp-hal **1.2.0**; the Kairos
  S3 cells pin **1.2.1**. That is the companion-set drift the Janus plan
  warns about, and ADR 0003 is the story of what happens when a primitive
  gets re-implemented in a second place.
- **Never quote a number without its method line.** Counters before clocks;
  work-count parity between arms before any ratio; the null arm before any
  verdict. QEMU supplies no cycle counter — measured six ways — so timing
  rows come from silicon and correctness rows come from the emulator.

---

## 7. What we do not know

Stated so nobody mistakes inference for plan.

- **The MATA Network mission is unwritten.** Proof-of-storage, erasure
  coding, settlement and reputation are referenced, not specified.
- **An MCU is not a storage node** and never will be — 512 KB of SRAM
  contributes no meaningful bytes or cycles. What a Tier 1 device contributes
  is **origination and presence**: it is where data is born and whose identity
  vouches for it. Any plan that treats edge devices as marketplace capacity
  is wrong.
- **Track A is untouched until K8+.** Most Janus S3 firmware is Track A. A
  master plan that implies Kairos is under those devices today would be
  false.
- **Matter/Thread vendor drift and IoT regulatory exposure** are named risks
  in the Home Computer mission and have no mitigation written yet.
- **No external crypto review, and no threat-model document.** Both are
  acknowledged gaps in the MATA perspective notes.

---

## 8. The sentence to remember

Amazon gave away a kernel to sell a cloud that reads your data.

We are building a kernel to reach a cloud that cannot — and the only reason
we can is that we own every layer in between.
