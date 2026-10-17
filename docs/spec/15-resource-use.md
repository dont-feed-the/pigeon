# 15. Resource use

> Mission: background work out of the person's way (CPU, disk, memory,
> network), with defaults nobody touches (ADR 0037). Reference case,
> Guillaume's: a large folder on a hard disk syncs while a video is edited
> from it. The queue, content controller, native probes and settings are
> implemented in stages; [the M6 report](../reports/resources-7.md) records
> actual coverage and the resource budgets still to measure.

Capture (P2) never waits for a good moment; transfers, repair and scrubs
can, and every wait is a reason in the status ([06 §6.4](06-status.md)).

## 15.1 Background priority by default

| Machine | CPU | Disk | Memory |
|---|---|---|---|
| a person's Linux computer | `Nice=10`, `CPUWeight=20` | `IOSchedulingClass=idle`, `IOWeight=20` | `MemoryHigh` on the unit, so pigeon's page cache is reclaimed first |
| a person's Mac | `ProcessType = Background` (launchd), which also throttles disk I/O | (same) | `F_NOCACHE` on pigeon's own reads |
| a backup machine | normal | normal; scrubs in the idle class (`ioprio_set` on the `fsck` child) | normal |

Children inherit these (units: [14 §14.4](14-platforms-and-packaging.md)).
The idle class works with BFQ only; with `mq-deadline` or `none` the
back-off of §15.4 protects, and `pigeon doctor` says so for a hard disk.

## 15.2 One work queue

`pigeond` orders all work and issues transfers one key at a time from batch
`get` and `copy` (`annex.jobs = 1` on laptops, [03 §3.4](03-repository-and-roles.md)).

| Class (highest first) | Work | Order inside the class |
|---|---|---|
| 1. capture | hashing and committing what people saved ([07 §7.3](07-capture-and-install.md)) | oldest change first |
| 2. history | fetch, validate, merge, status reports ([08 §8.2](08-custody-loops.md)) | most recent notice first |
| 3. protect | uploads and downloads that bring a version toward *protected*, and repair ([08 §8.3, §8.4](08-custody-loops.md), [08 §8.9](08-custody-loops.md)) | fewest custody copies first (a laptop copy counts for ordering only: fewer laptop copies first among equals, 13 §13.13), then endangered keys, then newest, then smallest |
| 4. download | content of kept folders, install ([07 §7.5](07-capture-and-install.md)) | folder priority (`high`, `normal`, `low`), then recently changed first |
| 5. scrub | `fsck --incremental --time-limit` slices ([08 §8.8](08-custody-loops.md)) | git-annex's incremental order |
| 6. rebalance | verified `drop --key` of keys a custodian no longer wants ([08 §8.10](08-custody-loops.md)) | — |

- Normative: `pigeon-core::queue` and its laws (`tests/queue.rs`):
  `compare_is_a_total_order` (ties broken by key, path, peer),
  `a_higher_class_goes_first`, `order_does_not_depend_on_arrival`,
  `fewer_custody_copies_never_wait_behind_more`, and
  `newer_equal_risk_work_precedes_older_smaller_work` (`tests/queue_recency.rs`).
  Shared bytes use their latest retained reference birth for recency; inheriting
  a parent's pointer in a merge does not introduce it again. Repair is in class 3, so a
  custodian spends its last space on the fewest-copy keys **[no ADR]**
  (issue #13, [16 §16.2](16-open-questions.md)).
- A higher class preempts a lower one between keys; a transfer in progress
  is paused, not abandoned. Class 1 always runs within its floor (§15.4).
  Folder priority is `[groups.priority]` ([11 §11.2](11-local-state.md)).

## 15.3 The network

**Adaptive shaper** **[no ADR: constants]**. Every byte between devices
passes `pigeond`'s stream relay ([09 §9.3](09-network-and-joining.md)), so
it shapes them itself (`annex.bwlimit` may not apply over P2P, E16;
`annex-bwlimit-download` is never read, E22), delay-based after LEDBAT
(RFC 6817):

- queuing delay: the smoothed RTT of its QUIC paths minus the **base
  delay**, their minimum over the last 10 minutes;
- every 200 ms, `rate += gain × rate_step × (target − queuing_delay) /
  target` for a token bucket shared by content streams, between 16 KB/s per
  active class and the limit; `target` 25 ms (Gentle) or 60 ms (Balanced),
  `gain` 1, `rate_step` 1 % of the measured path capacity;
- control traffic (history, status, protocol, join) is never shaped;
  `upload` and `download` are ceilings (`auto`: the shaper alone);
  `full_speed` windows turn the shaper off, limits still apply.

**When to wait.** Content transfers and scrubs pause:

| Condition | Detected by | Reason shown |
|---|---|---|
| on battery below `battery_percent` (default 20 %), or low-power mode | UPower over D-Bus (`zbus`) on Linux; IOKit power sources and `NSProcessInfo.lowPowerModeEnabled` on macOS | `paused(battery)`: "Waiting for power: this computer is on battery." |
| a metered network | NetworkManager `Metered` on Linux; `NWPath.isExpensive` / `isConstrained` (Network.framework) on macOS | `paused(metered)`: "Waiting for a non-metered network." |

Missing information means "not on battery, not metered" (ADR 0037); capture
and the history exchange continue **[no ADR: history on metered]**.

## 15.4 The disk

**Contention is detected, not configured** **[no ADR: thresholds]**: the
disk is **busy** when pigeon's median read latency over 10 s exceeds 3 × its
quiet baseline, or Linux I/O pressure (`some avg10` of `/proc/pressure/io`,
or of pigeon's cgroup) exceeds 10 % while pigeon uses that device. While
busy **[no ADR: timings]**: heavy children (hashing, transfers, `fsck`) get
`SIGSTOP`, later `SIGCONT`, and pigeon's reads pause; capture keeps a
**progress floor** of 5 % of the time, so P2 never starves; work resumes
after 5 s of quiet, at full rate over 30 s; transfers, repair and scrubs
show `paused(disk-busy)`. Managed repositories and peers disable
`annex.stalldetection` and its directional overrides: intentional power or
disk waits have no finite maximum. Pigeon's operation deadline counts active
time and excludes its own stream waits. This also keeps transfers in the
owned process group; explicit annex stall detection otherwise creates a
separately grouped transferrer (`Annex/Transfer.hs`,
`Annex/TransferrerPool.hs`, git-annex 10.20261006). Genuine active stalls
still time out; E17 must verify that deliberate pauses abandon no transfer.

**Space:** `annex.diskreserve`; `ENOSPC` pauses every writer on that file
system (`disk-full`, [08 §8.9](08-custody-loops.md)). **Page cache:** pigeon's
reads use `POSIX_FADV_DONTNEED` or `F_NOCACHE`; git-annex's cannot be
advised, so on Linux `MemoryHigh` reclaims pigeon's cache first (macOS lacks
this; E17 measures whether it matters).

## 15.5 Large and hot files

- **Quiet period:** 2 s plus 1 s per 100 MB, at most 60 s, and two equal
  `stat` ([07 §7.2](07-capture-and-install.md)); ADR 0013's 2 s for small
  files. A file growing between `stat`s is not hashed.
- **Hot paths:** over 6 captures in an hour slow a path to one per 10
  minutes until quiet for an hour; the last save is always captured.

## 15.6 Settings: presets and details

Settings → *Use of this computer* offers three presets; Guillaume's case
passes with the default (§15.8).

| | *Gentle* (default) | *Balanced* | *Fast* |
|---|---|---|---|
| process priority | background (§15.1) | background | normal |
| network shaper target | 25 ms | 60 ms | off |
| disk back-off | on | on | capture floor 50 % |
| on battery below 20 % | pause transfers | pause below 10 % | continue |
| on a metered network | pause transfers | pause transfers | continue |

**[no ADR: preset values]**. The details are `[resources]` keys of
`config.toml` ([11 §11.2](11-local-state.md)); one set by hand overrides the
preset; `scrub_window` defaults to 01:00–06:00. `pigeon pause [DURATION]`
stops all but capture until it ends or `pigeon resume`; `paused_until`
survives a restart; the status says `paused(person)` (ADR 0027).
*Coded so far:* `pause` and `resume` write `paused_until` (`"resume"` for
a pause without end, DURATION as `45m`, `2h` or `3d`); a paused round only
captures and the headline says until when ("until 14:38", in this
computer's local time), unless something needs the person. The pause is
said even then: `pigeon status` adds its line under that headline (the
status report's `paused`), and every web page shows it as a banner with
*Share again* (`Paused`, 10 §10.3); Settings offers only the action that
applies, the pause choices while sharing, *Share again* while paused.
Peers may still fetch from this computer **[no ADR: serve while
paused]**.

## 15.7 Budgets

Measured by CI on a reference tree (100 000 files, 50 GB, two peers)
**[no ADR: tree]** and in the field by telemetry
([12 §12.2](12-observability.md)):

| Budget | Limit |
|---|---|
| CPU when idle (nothing to capture or transfer) | below 1 % of one core, averaged over 10 minutes |
| memory of `pigeond` | bounded, independent of the content size; for the reference tree at most 300 MB resident **[no ADR: value]** |
| scans | a 10-minute scan of the reference tree within its CPU budget (ADR 0013) |
| E17 | §15.8 |

## 15.8 E17: editing video on a disk pigeon is syncing

Guillaume's case (ADR 0037), with **default settings**: a 7200 rpm hard
disk holding a 500 GB folder pigeon captures and syncs to a backup machine
over a LAN; `fio` reads at 4K ProRes 422 HQ rate (about 90 MB/s) with a
seek every 2 s, for 30 minutes; alone, then with pigeon, on Linux with BFQ,
with `mq-deadline`, and on macOS. **Pass:** throughput and p99 read latency
within 10 % of the run alone, capture progressing, no transfer abandoned
for `SIGSTOP`. If only BFQ passes, `pigeon doctor` recommends it. E17 runs
on a self-hosted HDD runner in `models-full` ([13 §13.8](13-verification.md)).
