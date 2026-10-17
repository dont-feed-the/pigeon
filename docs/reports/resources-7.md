# M6 resource-control progress

M6 is not complete or released. This branch contains controller components,
configuration validation and native content shaping. No resource budget
has been claimed to pass.

## Controller components

`pigeon-core::resources` supplies the Gentle/Balanced/Fast defaults,
explicit power/network waiting decisions, the size-dependent capture quiet
period, disk back-off with capture's progress floor and quiet recovery,
delay-based rate updates, and a shared bounded token bucket. All timing is
supplied monotonic input. Manual ceilings win over automatic rate floors;
backwards clocks and rate changes cannot manufacture past credit. Oversize
reservations are refused for splitting rather than silently undercharged.

Eight hegel laws run 1000 cases each, including extreme integer clock/rate
ranges. These are pure controller laws, not measurements of foreground
workload interference, real network delay or native process pausing.

The P2P filter can distinguish content DATA bodies from protocol and Git
history bytes without changing their order or bytes. Two more 1000-case
laws check both directions and arbitrary transport chunk boundaries.
Both native relay directions now enable classification and use a single
upload bucket and download bucket for the whole daemon. Existing filter and purge laws still pass, for 22
selected core tests in total. A Clippy failure on the new fixture's constant
assertion was corrected to an explicitly justified fixture panic.

`config.toml` parses resource presets, exact decimal/binary byte rates,
battery/metered overrides and local full-speed/scrub windows. Invalid
readings are rejected whole instead of silently becoming unlimited values.
Missing details follow the preset; explicit choices are independent.
Five configuration checks pass, including comment/unknown-setting
preservation and overflow/fractional-rate cases. The daemon reloads complete valid settings every two seconds, retaining
its last valid reading after an invalid edit. CLI and browser changes
validate a candidate before writing it; manual details override presets.

## Native content boundary

Authenticated selected QUIC paths provide smoothed RTT and a cwnd/RTT
capacity estimate. The controller updates every 200 ms, with a ten-minute
minimum baseline, bounded 64 KiB reservations and explicit ceilings even
inside full-speed windows. Control/history frames do not reserve tokens.
Queued content rechecks the existing erasure/admission filter at most every
200 ms and immediately before forwarding. No new authorization rule or
positive custody proof is introduced.

Linux probes reuse D-Bus for UPower, power profiles and NetworkManager,
with a two-second sampling deadline. Unknown observations use ADR 0037's
fallback. Linux I/O pressure pauses pending content and subsequent scrub
slices, with quiet recovery. Mid-file child pausing and own-read latency
measurement are still outstanding; this is not the full disk controller.

137 selected law/native/UI/configuration tests passed locally, including a real
QUIC regression that queues bytes waiting for power, then erases their key:
no queued bytes reach the client. This is finite evidence, not an elapsed
resource-budget measurement. English and French messages explain waits
without replacing Missing or Needs you; Resume remains a person's pause.

## Remaining

- Finish macOS power/network probes; protect capture's progress and
  pause/resume native heavy children safely, including active-time deadlines.
- Integrate file quiet/hot-path scheduling, native priorities and supervisor
  behavior. Validate foreground disk contention and read-latency feedback.
- Measure the reference-tree idle/scan/memory budgets in CI and the E17
  foreground video workload on the required HDD/platform combinations.
- Validate supported filesystems, upgrade and crash behavior, integrate
  current main, complete the release gates and publish v0.7.0.

No 10-minute budget, 30-minute HDD run, 72-hour soak, seven-night result or
two-week dogfood result is inferred from these finite tests.
