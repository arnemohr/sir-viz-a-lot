# Phase 5 — DMX transport library decision (P5.0.1, W0)

**Status:** decision record. The actual transport thread, universe buffer,
and channel wiring land in W2 follow-up tasks once this decision is
ratified.

## Constraints

- **macOS-only target** through v1 (`CLAUDE.md`: "v1 is macOS-only by
  design").
- **Show-day frame budget must be unaffected.** All network I/O runs on
  a background thread; the render thread never blocks on a send (same
  pattern as `src/controls/osc.rs`'s `Arc<AtomicBool>` stop flag +
  crossbeam bounded channel).
- **No tokio.** `CLAUDE.md` is explicit: async wgpu calls use
  `pollster::block_on`; adding tokio for DMX would be an inconsistency
  with no payoff given the transport sends at ~44 Hz, not thousands of
  concurrent futures.
- **Cargo feature gate.** Phase 5 ships behind `lighting` (off by
  default), mirroring `audio` / `midi` / `osc`. `cargo build
  --no-default-features` must succeed.
- **Small live shows, single machine.** The roadmap §1.1 framing is
  "single-machine, single-projector event-scale." Up to 16 DMX universes
  is the stated acceptance criterion; a 50-universe broadcast-storm
  scaler is permanently out of scope.
- **Art-Net and/or sACN.** Phase 5's plan says "and/or" — this doc
  resolves that ambiguity.
- **`DmxTransport` abstraction.** Phase 7 is expected to extend lighting
  (RGBW, colour temperature). The transport binding must be behind a
  trait so Phase 7 can add sACN or a stub transport without re-
  architecting `DmxUniverse`.

## Candidates evaluated

### 1. `artnet_protocol` v0.4.4 (MIT) — chosen

- **Last release:** 2025-10-04 (v0.4.4); v0.4.3 was 2024-08-19. Active.
- **Downloads:** 65 093 (crates.io, 2026-05-12).
- **Repository:** <https://github.com/trangar/artnet_protocol>
- **API shape:** encode/decode of Art-Net PDUs (`ArtCommand`, `ArtDmx`,
  `ArtPoll`, `ArtPollReply`). Pure Rust, no system deps. Serialises to
  a `Vec<u8>` that goes straight onto a `UdpSocket`.
- **Pros:**
  - Actively maintained through late 2025.
  - MIT license — no redistribution friction, no `cargo bundle` concern.
  - Zero system install: UDP sockets + pure-Rust encode. `make setup`
    stays unchanged.
  - Art-Net UDP port 6454, broadcast or directed unicast — covers every
    small-event Art-Net node (Enttec, Artistic Licence, Chinese budget
    nodes) without multicast configuration.
  - Familiar protocol: operators at small live events own Art-Net nodes
    far more often than sACN receivers.
  - Packet structure is trivially parseable in Wireshark / a CI packet-
    capture fixture — Phase 5's acceptance criterion calls this out
    explicitly.
- **Cons:**
  - Art-Net is a proprietary (now open) spec, whereas sACN is an ANSI
    standard. No practical consequence for v1.
  - Broadcast UDP does not scale past subnet limits; not relevant for
    single-machine use.
  - No built-in rate-limiting; the sender loop must throttle to ~44 Hz
    itself (trivial with `thread::sleep`).

### 2. `sacn` v0.11.1 (MIT OR Apache-2.0) — deferred to Phase 7+

- **Last release:** 2026-01-04 (v0.11.1). Very actively maintained.
- **Downloads:** 54 692 (crates.io, 2026-05-12).
- **Repository:** <https://github.com/RustLight/sacn>
- **API shape:** full E1.31 source + receiver, multicast or unicast.
- **Pros:**
  - ANSI E1.31 standard — the architecturally correct protocol for larger
    installs and integration with pro lighting consoles.
  - MIT/Apache-2.0 dual license.
  - Higher baseline adoption in architectural and permanent-install
    markets.
- **Cons:**
  - Multicast requires network configuration (IGMP-enabled switch,
    correct VLAN); in a rental-venue or improvised club setup this adds
    real setup friction.
  - E1.31 has mandatory priority and synchronisation framing that is
    unnecessary overhead for the "drag a canvas region, watch the
    fixture" UX in Phase 5.
  - The Phase 5 usability rule ("the first thing an operator does after
    wiring an Art-Net node…") assumes Art-Net; rewriting the spec in
    sACN terms would misrepresent the target operator.
  - Art-Net is the dominant protocol in the small-show market; sACN
    adoption grows with install scale, not event scale.

### 3. Raw `UdpSocket` + hand-rolled Art-Net framing — rejected

- **Pros:** zero dependencies, full control.
- **Cons:** Art-Net's PDU framing (opcode, universe addressing, sequence
  numbers) has enough detail to make a bespoke encoder a maintenance
  liability. `artnet_protocol` already handles this correctly and is MIT.

### 4. FFI against the Enttec open DMX SDK — rejected

- **Pros:** well-known brand in the small-event market.
- **Cons:** C library; `make setup` burden; bundle complexity; no benefit
  over pure UDP for Art-Net unicast.

## Decision

**`artnet_protocol` is the chosen transport for Phase 5.**

The target operator is the small-show / club / event DP who owns a
budget Art-Net node (Enttec ODE Mk 3, Artistic Licence Ether-Lynx,
or a generic Chinese node). Art-Net broadcast or unicast on a laptop's
Wi-Fi or a show switch is the default wiring method; no multicast
configuration is required. Phase 5's usability rule explicitly puts
"wire an Art-Net node, define a fixture group, sample a canvas region"
as the first-operator experience — sACN does not fit that sentence.

`artnet_protocol` v0.4.4 is actively maintained (October 2025), MIT
licensed, zero system deps, and has 65 000 downloads indicating real-
world use. The pure-Rust UDP-encode model slots directly into the
existing `crossbeam_channel` + background-thread pattern used by OSC
(`src/controls/osc.rs`).

sACN remains the architecturally correct choice for large-scale or
permanent-install use. The `DmxTransport` trait (P5.2.1) abstracts the
socket from the universe-sending logic so Phase 7 can add a
`SacnTransport` impl without touching `DmxUniverse` or the fixture
model.

## Architecture (for W2 follow-up tasks)

### Threading model

Mirror `OscSource` exactly:

```
Render thread
    ──crossbeam send (bounded 4)──► LightingThread
                                        │
                                   UdpSocket (Art-Net port 6454)
                                        │
                                   ──► Art-Net node on LAN
```

- The render thread calls `lighting_tx.try_send(universe_snapshot)`.
  If the channel is full (backpressure from a slow network), `try_send`
  returns `Err(Full)` and the frame continues. The DMX output may skip
  a packet; the visual output is never delayed.
- The lighting thread owns its own ~44 Hz ticker (`thread::sleep` between
  sends). It redrains the channel on each tick, taking only the latest
  snapshot (older ones are superseded and dropped).
- Stop signal: `Arc<AtomicBool>`, same as `OscSource::stop`.

### `DmxTransport` trait

```rust
pub trait DmxTransport: Send + 'static {
    /// Send one universe. Called from the lighting thread (~44 Hz).
    /// Must not block the render thread.
    fn send_universe(&mut self, universe: u16, data: &[u8; 512])
        -> Result<(), LightingError>;
}
```

`ArtNetTransport` implements this for Phase 5. `SacnTransport` (Phase 7)
and `NullTransport` (tests) implement the same trait.

### W2 tasks (for task breakdown)

- **P5.2.1** — `DmxTransport` trait + `ArtNetTransport` impl wrapping
  `artnet_protocol`.
- **P5.2.2** — `DmxUniverse` newtype (`[u8; 512]`) + channel type;
  `LightingThread::start` / `stop` mirroring `OscSource`.
- **P5.2.3** — `LightingThread` background loop: ~44 Hz Art-Net
  `ArtDmx` sends with sequence-number increment; `try_send` non-blocking
  handshake.
- **P5.2.4** — `lighting` cargo feature gate; `cargo build
  --no-default-features` CI check.

## Acceptance gates (for W2 integration tasks)

- [ ] `artnet_protocol` added behind `lighting` feature only.
- [ ] `cargo build --no-default-features` succeeds.
- [ ] `cargo build --features lighting` succeeds.
- [ ] Lighting thread starts on `EnterGoLive`, stops on `ExitGoLive`.
- [ ] Render thread `try_send` never blocks; verified by frame-time
      assertions.
- [ ] Art-Net packets visible in Wireshark on port 6454 with correct
      opcode (`0x5000` / `ArtDmx`).
- [ ] Up to 16 simultaneous universes send at ~44 Hz without frame-time
      regression (show-day perf gate).

## Out of scope for Phase 5

- sACN (E1.31) transport — deferred to Phase 7.
- Art-Net `ArtPoll` / `ArtPollReply` discovery — Phase 5 uses directed
  unicast or subnet broadcast; discovery is a Phase 7 UX polish item.
- Art-Net input (receive) — Phase 5 is output-only.
- More than 16 universes — roadmap §11 ("huge protocol surface area
  early on" is permanently deprioritised).
- Console interop (Hog, MA, EOS) — permanently out of scope per Phase 5
  plan.
