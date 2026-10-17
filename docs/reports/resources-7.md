# M6 resource-control progress

M6 is not complete or released. This branch contains controller components,
configuration validation and a payload-classification boundary. The daemon
does not yet apply the new controllers, and no resource budget has been
claimed to pass.

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
Classification is opt-in; the current native adapters do not enable it or
apply a new limiter. Existing filter and purge laws still pass, for 22
selected core tests in total. A Clippy failure on the new fixture's constant
assertion was corrected to an explicitly justified fixture panic.

`config.toml` parses resource presets, exact decimal/binary byte rates,
battery/metered overrides and local full-speed/scrub windows. Invalid
readings are rejected whole instead of silently becoming unlimited values.
Missing details follow the preset; explicit choices are independent.
Five configuration checks pass, including comment/unknown-setting
preservation and overflow/fractional-rate cases. These checks do not mean
the settings already control native work.

## Remaining

- Connect the controller to both P2P directions, while keeping control and
  history unshaped and refreshing erasure authorization after every wait.
- Read actual path latency/capacity and platform power/network conditions;
  protect capture's progress and pause/resume native heavy children safely.
- Integrate file quiet/hot-path scheduling, priorities, presets and details
  with the CLI, status, browser and supervisor behavior.
- Measure the reference-tree idle/scan/memory budgets in CI and the E17
  foreground video workload on the required HDD/platform combinations.
- Validate supported filesystems, upgrade and crash behavior, integrate
  current main, complete the release gates and publish v0.7.0.

No 10-minute budget, 30-minute HDD run, 72-hour soak, seven-night result or
two-week dogfood result is inferred from these finite tests.
