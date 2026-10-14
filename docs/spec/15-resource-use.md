# 15. Resource use

> Mission: how `pigeond` keeps background work out of the way of the person
> using the computer (CPU, disk, memory, network), with defaults nobody has
> to touch (ADR 0037). The reference case is Guillaume's: a large folder on a
> hard disk syncs while a video is edited from it, without configuring
> pigeon.

Safety comes first: capture (P2) never waits for a good moment; transfers,
repair and scrubs can. Every wait is a reason in the status
([06 §6.4](06-status.md)), so nothing hides that a file is not safe yet.

## 15.1 Background priority by default

| Machine | CPU | Disk | Memory |
|---|---|---|---|
| a person's Linux computer | `Nice=10`, `CPUWeight=20` | `IOSchedulingClass=idle`, `IOWeight=20` | `MemoryHigh` on the unit, so pigeon's page cache is reclaimed first |
| a person's Mac | `ProcessType = Background` (launchd), which also throttles disk I/O | (same) | `F_NOCACHE` on pigeon's own reads |
| a backup machine | normal | normal; scrubs in the idle class (`ioprio_set` on the `fsck` child) | normal |

Children (git, git-annex) inherit these; the units are in
[14 §14.4](14-platforms-and-packaging.md). On Linux the idle class works
with BFQ only; with `mq-deadline` or `none` the back-off of §15.4 protects,
and `pigeon doctor` says so for a hard disk.

## 15.2 One work queue

`pigeond` orders all work itself and issues transfers one key at a time from
batch `get` and `copy` processes (`annex.jobs = 1` on laptops;
[03 §3.4](03-repository-and-roles.md)).

| Class (highest first) | Work | Order inside the class |
|---|---|---|
| 1. capture | hashing and committing what people saved ([07 §7.3](07-capture-and-install.md)) | oldest change first |
| 2. history | fetch, validate, merge, status reports ([08 §8.2](08-custody-loops.md)) | most recent notice first |
| 3. protect | uploads and downloads that bring a version toward *protected*, and repair ([08 §8.3, §8.4](08-custody-loops.md), [08 §8.9](08-custody-loops.md)) | fewest custody copies first (a laptop copy counts for ordering only: fewer laptop copies first among equals, 13 §13.13), then endangered keys, then newest, then smallest |
| 4. download | content of kept folders, install ([07 §7.5](07-capture-and-install.md)) | folder priority (`high`, `normal`, `low`), then recently changed first |
| 5. scrub | `fsck --incremental --time-limit` slices ([08 §8.8](08-custody-loops.md)) | git-annex's incremental order |
| 6. rebalance | verified `drop --key` of keys a custodian no longer wants ([08 §8.10](08-custody-loops.md)) | — |

- The order is `pigeon-core::queue`, and its laws are normative
  (`tests/queue.rs`): `compare_is_a_total_order` (ties broken by the
  remaining fields: key, path, peer), `a_higher_class_goes_first`,
  `order_does_not_depend_on_arrival`,
  `fewer_custody_copies_never_wait_behind_more`. Repair is in class 3, so a
  custodian spends its last space on the keys with the fewest copies
  **[no ADR]** (issue #13, [08 §8.9](08-custody-loops.md)).
- A higher class preempts a lower one between keys; a transfer in progress
  is paused, not abandoned. Class 1 always runs within its floor (§15.4).
- Folder priority is a per-device setting (`[groups.priority]`,
  [11 §11.2](11-local-state.md)).

## 15.3 The network

**Adaptive shaper.** Every byte between devices passes `pigeond`'s stream
relay ([09 §9.3](09-network-and-joining.md)), so `pigeond` shapes it itself
(`annex.bwlimit` may not apply over P2P, E16, and `annex-bwlimit-download`
is never read, E22). It is delay-based, after LEDBAT (RFC 6817)
**[no ADR: constants]**:

- the queuing delay is the smoothed RTT of its QUIC paths minus the **base
  delay**, their minimum over the last 10 minutes;
- every 200 ms, `rate += gain × rate_step × (target − queuing_delay) /
  target` for a token bucket shared by content streams, between 16 KB/s per
  active class and the configured limit; `target` 25 ms (Gentle) or 60 ms
  (Balanced), `gain` 1, `rate_step` 1 % of the measured path capacity;
- control traffic (history, status, protocol, join) is never shaped;
- `upload` and `download` rates are ceilings (`auto`: the shaper alone);
  `full_speed` windows turn the shaper off, limits still apply.

**When to wait.** Content transfers and scrubs pause:

| Condition | Detected by | Reason shown |
|---|---|---|
| on battery below `battery_percent` (default 20 %), or low-power mode | UPower over D-Bus (`zbus`) on Linux; IOKit power sources and `NSProcessInfo.lowPowerModeEnabled` on macOS | `paused(battery)`: "Waiting for power: this computer is on battery." |
| a metered network | NetworkManager `Metered` on Linux; `NWPath.isExpensive` / `isConstrained` (Network.framework) on macOS | `paused(metered)`: "Waiting for a non-metered network." |

Missing information means "not on battery, not metered" (ADR 0037). Capture
and the (small) history exchange continue **[no ADR: history on metered]**.

## 15.4 The disk

**Contention is detected, not configured** **[no ADR: thresholds]**. The
disk is **busy** when pigeon's median read latency over 10 s exceeds 3 × its
quiet baseline, or when Linux I/O pressure (`some avg10` of
`/proc/pressure/io`, or of pigeon's cgroup) exceeds 10 % while pigeon itself
uses that device. While it is busy **[no ADR: timings]**:

- heavy children (hashing, transfers, `fsck`) get `SIGSTOP` and later
  `SIGCONT`, and pigeon's own reads pause;
- capture keeps a **progress floor** of 5 % of the time (one second in
  twenty), so P2 never starves;
- work resumes after 5 s of quiet and returns to full rate over 30 s;
  transfers, repair and scrubs show `paused(disk-busy)`;
- `annex.stalldetection` is above the longest pause, so git-annex abandons
  no paused transfer (E17 checks it).

**Space** is kept by `annex.diskreserve`; `ENOSPC` pauses every writer on
that file system (*Needs you*, `disk-full`, [08 §8.9](08-custody-loops.md)).
**Page cache:** pigeon's reads use `POSIX_FADV_DONTNEED` (Linux) or
`F_NOCACHE` (macOS); git-annex's cannot be advised, so on Linux `MemoryHigh`
makes the kernel reclaim pigeon's cache first. macOS lacks this; E17
measures whether it matters.

## 15.5 Large and hot files

- **Quiet period:** 2 s plus 1 s per 100 MB, at most 60 s, and two equal
  `stat` ([07 §7.2](07-capture-and-install.md)); ADR 0013's 2 s stays for
  small files.
- **Hot paths:** more than 6 captures in an hour slow a path to one capture
  per 10 minutes until it has been quiet for an hour; the last save is
  always captured. A file growing between `stat`s is not hashed.

## 15.6 Settings: presets and details

Settings → *Use of this computer* offers three presets; Guillaume's case
must pass with the default (§15.8).

| | *Gentle* (default) | *Balanced* | *Fast* |
|---|---|---|---|
| process priority | background (§15.1) | background | normal |
| network shaper target | 25 ms | 60 ms | off |
| disk back-off | on | on | capture floor 50 % |
| on battery below 20 % | pause transfers | pause below 10 % | continue |
| on a metered network | pause transfers | pause transfers | continue |

**[no ADR: preset values]**. The details are `[resources]` keys of
`config.toml` ([11 §11.2](11-local-state.md)); one set by hand overrides the
preset. `scrub_window` defaults to 01:00–06:00 local time.

**Pause.** `pigeon pause [DURATION]` stops everything but capture until it
ends or `pigeon resume`; `paused_until` survives a restart, and the status
says `paused(person)` ("Sharing is paused until 18:00", ADR 0027).

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

The acceptance test of Guillaume's case (ADR 0037), with **default
settings**: a 7200 rpm hard disk holding a 500 GB folder that pigeon
captures and syncs to a backup machine over a LAN; an `fio` workload of
sequential reads at 4K ProRes 422 HQ bit rate (about 90 MB/s) plus a seek
to another file every 2 s, for 30 minutes; run alone, then with pigeon, on
Linux with BFQ, with `mq-deadline`, and on macOS. **Pass** if with pigeon
the throughput and 99th-percentile read latency stay within 10 % of the run
alone, capture still progresses, and no transfer is abandoned for
`SIGSTOP`. If only BFQ passes, `pigeon doctor` recommends it for hard
disks. E17 runs on a self-hosted HDD runner in `models-full`
([13 §13.8](13-verification.md)).
