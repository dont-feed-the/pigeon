# 15. Resource use

> Mission: specify how `pigeond` keeps its background work out of the way of
> the person using the computer (CPU, disk, memory and network), with
> sensible defaults, adaptive behaviour, priorities and settings that nobody
> has to touch (ADR 0037). The reference case is Guillaume's: a large
> folder on a hard disk is being synced while a video is edited from files
> of that folder, without pausing or configuring pigeon.

Safety comes first: capture (P2) never waits for a good moment, while
transfers, repair and scrubs can. Nothing here may hide that a file is not
safe yet: every wait is a reason in the status ([06 §6.4](06-status.md)).

## 15.1 Background priority by default

| Machine | CPU | Disk | Memory |
|---|---|---|---|
| a person's Linux computer | `Nice=10`, `CPUWeight=20` | `IOSchedulingClass=idle`, `IOWeight=20` | `MemoryHigh` on the unit, so pigeon's page cache is reclaimed first |
| a person's Mac | `ProcessType = Background` (launchd), which also throttles disk I/O | (same) | `F_NOCACHE` on pigeon's own reads |
| a backup machine | normal | normal; scrubs in the idle class (`ioprio_set` on the `fsck` child) | normal |

Children (git, git-annex) inherit these settings. The units are given in
[14 §14.4](14-platforms-and-packaging.md). On Linux, the idle class is honoured
by the BFQ scheduler only; with `mq-deadline` or `none`, the back-off of §15.4
is the protection, and `pigeon doctor` says so for a hard disk.

## 15.2 One work queue

`pigeond` orders all work itself, instead of git-annex's own ordering, and
issues transfers one key at a time from batch `get` and `copy` processes
(`annex.jobs = 1` on laptops; [03 §3.4](03-repository-and-roles.md)).

| Class (highest first) | Work | Order inside the class |
|---|---|---|
| 1. capture | hashing and committing what people saved ([07 §7.3](07-capture-and-install.md)) | oldest change first |
| 2. history | fetch, validate, merge, status reports ([08 §8.2](08-custody-loops.md)) | most recent notice first |
| 3. protect | uploads and downloads that bring a version toward *protected*, and repair ([08 §8.3, §8.4](08-custody-loops.md), [08 §8.9](08-custody-loops.md)) | fewest custody copies first (a laptop copy counts for ordering only: fewer laptop copies first among equals, 13 §13.13), then endangered keys, then newest, then smallest |
| 4. download | content of kept folders, install ([07 §7.5](07-capture-and-install.md)) | folder priority (`high`, `normal`, `low`), then recently changed first |
| 5. scrub | `fsck --incremental --time-limit` slices ([08 §8.8](08-custody-loops.md)) | git-annex's incremental order |
| 6. rebalance | verified `drop --key` of keys a custodian no longer wants ([08 §8.10](08-custody-loops.md)) | — |

- A higher class preempts a lower one between keys; a transfer in progress
  is paused, not abandoned, when a higher class needs the same resource.
- The order is a pure function in `pigeon-core` with laws: it is a total
  preorder, made total by breaking every tie on the remaining fields (the
  key, the path, the peer; 13 §13.13); class 1 always runs within its floor (§15.4); a key's position
  never depends on arrival order of equal items; a key held by fewer
  custodians never waits behind one held by more ([08 §8.9](08-custody-loops.md)). Repair is no longer
  a class below downloads: a custodian spends its last space on the keys with
  the fewest copies.
- Folder priority is a per-device setting (`[groups.priority]`,
  [11 §11.2](11-local-state.md)).

## 15.3 The network

**Adaptive shaper.** Every byte between devices goes through `pigeond`'s stream
relay ([09 §9.3](09-network-and-joining.md)), so `pigeond` shapes it itself,
whether or not `annex.bwlimit` applies over the P2P transport (E16), and
although `annex-bwlimit-download` is never read in git-annex 10.20261006
(E22). The shaper
is delay-based, in the spirit of LEDBAT (RFC 6817):

- It samples the round-trip time of its QUIC paths (from iroh's path
  statistics), keeps a **base delay** (the minimum over the last 10 minutes),
  and computes the queuing delay as the current smoothed RTT minus the base.
- Every 200 ms it adjusts a token-bucket rate shared by its content streams:
  `rate += gain × rate_step × (target − queuing_delay) / target`, never below
  a floor of 16 KB/s per active class, never above the configured limit.
  Defaults: `target` 25 ms (Gentle), 60 ms (Balanced); `gain` 1; `rate_step`
  1 % of the measured path capacity **[no ADR: constants]**.
- Control traffic (history, status, protocol messages, the join exchange) is
  not shaped, only content.
- Explicit `upload` and `download` limits (a rate) are ceilings; `auto` means
  the shaper alone. Windows in `full_speed` turn the shaper off (limits still
  apply).

**When to wait.** Content transfers and scrubs pause:

| Condition | Detected by | Reason shown |
|---|---|---|
| on battery below `battery_percent` (default 20 %), or low-power mode | UPower over D-Bus (`zbus`) on Linux; IOKit power sources and `NSProcessInfo.lowPowerModeEnabled` on macOS | `paused(battery)`: "Waiting for power: this computer is on battery." |
| a metered network | NetworkManager `Metered` on Linux; `NWPath.isExpensive` / `isConstrained` (Network.framework) on macOS | `paused(metered)`: "Waiting for a non-metered network." |

- Missing information means "not on battery, not metered" (ADR 0037).
- Capture continues in every case; history exchange continues, since it is
  small **[no ADR: history on metered]**.

## 15.4 The disk

**Contention is detected, not configured.** `pigeond` watches two signals:

1. **System I/O pressure** (Linux): `some avg10` of `/proc/pressure/io` (or of
   the user's cgroup when pigeon has its own), above 10 % by default.
2. **Its own read latency** (both platforms): the median latency of pigeon's
   reads of the last 10 seconds, against a baseline measured when the disk was
   quiet; contended above 3 × the baseline.

The disk is **busy** when signal 2 says so, or when signal 1 does and pigeon is
itself reading or writing on that device (so pressure on another disk does not
slow pigeon) **[no ADR: thresholds]**.

**Back-off.** While the disk is busy:

- heavy children (git-annex hashing, transfers, `fsck`) are paused with
  `SIGSTOP` and resumed with `SIGCONT`, so they stop mid-file; pigeon's own
  reads pause too;
- capture keeps a **progress floor**: it runs at least 5 % of the time (one
  second in every twenty), so P2 never starves;
- work resumes after the disk has been quiet for 5 s, and goes back to full
  rate over 30 s **[no ADR: timings]**;
- transfers, repair and scrubs may wait as long as the disk is busy; the
  status says `paused(disk-busy)`: "Waiting: this computer's disk is busy."
- `annex.stalldetection` is set above the longest pause, so that git-annex
  does not abandon a paused transfer (E17 checks it).

**Space.** Real free space is kept by `annex.diskreserve`; a write that
fails with `ENOSPC` pauses every writer on that file system until the reserve
is back plus 1 GB, and shows *Needs you* (`disk-full`;
[08 §8.9](08-custody-loops.md), [08 §8.9](08-custody-loops.md)).

**Page cache.** pigeon's own reads use `posix_fadvise(POSIX_FADV_DONTNEED)`
after use on Linux and `F_NOCACHE` on macOS. git-annex's reads cannot be
advised; on Linux, `MemoryHigh` makes the kernel reclaim the cache charged to
pigeon's unit first. On macOS this protection is missing, and E17 measures
whether it matters.

## 15.5 Large and hot files

- **Quiet period.** Capture waits until a file has been quiet for 2 s plus 1 s
  per 100 MB of its size, at most 60 s, and two equal `stat` results
  ([07 §7.2](07-capture-and-install.md)). This refines ADR 0013's 2 s, which
  stays the value for small files.
- **Hot paths.** A path captured more than 6 times in an hour is captured at
  most every 10 minutes afterwards, until it has been quiet for an hour, so
  renders and autosaves do not fill history. The last save is always captured.
- A file still open for writing is not hashed while it grows (size changing
  between `stat`s).

## 15.6 Settings: presets and details

Settings → *Use of this computer* offers three presets. Nobody has to change
them; Guillaume's case must pass with the default (§15.8).

| | *Gentle* (default) | *Balanced* | *Fast* |
|---|---|---|---|
| process priority | background (§15.1) | background | normal |
| network shaper target | 25 ms | 60 ms | off |
| disk back-off | on | on | capture floor 50 % |
| on battery below 20 % | pause transfers | pause below 10 % | continue |
| on a metered network | pause transfers | pause transfers | continue |

**[no ADR: preset values]**. The details are keys of `[resources]` in
`config.toml` ([11 §11.2](11-local-state.md)): `upload`, `download` (`auto` or a
rate), `on_battery`, `battery_percent`, `on_metered`, `full_speed` (time
windows), `scrub_window` (backup machines, default 01:00–06:00 local time) and
the folder priorities. A detail set by hand overrides the preset.

**Pause.** `pigeon pause [DURATION]` (Settings → Pause) stops everything but
capture, for the duration or until `pigeon resume`; `paused_until` survives a
restart. The status says `paused(person)`: "Sharing is paused until 18:00", so
pausing never hides that files are not safe yet (ADR 0027).

## 15.7 Budgets

Measured by CI on a reference tree (100 000 files, 50 GB, two peers)
**[no ADR: tree]**, and by telemetry in the field (`pigeon.process.cpu`,
`pigeon.process.memory`, `pigeon.backoff.seconds`, [12 §12.2](12-observability.md)):

| Budget | Limit |
|---|---|
| CPU when idle (nothing to capture or transfer) | below 1 % of one core, averaged over 10 minutes |
| memory of `pigeond` | bounded, independent of the content size; for the reference tree at most 300 MB resident **[no ADR: value]** |
| scans | a 10-minute scan of the reference tree within its CPU budget (ADR 0013) |
| E17 | §15.8 |

## 15.8 E17: editing video on a disk pigeon is syncing

The acceptance test of Guillaume's case, run with **default settings**:

- **Machine.** A hard disk (7200 rpm), holding a 500 GB folder that pigeon
  captures and syncs to a backup machine over a LAN.
- **Workload.** A reference video workload with `fio`: sequential reads at the
  bit rate of 4K ProRes 422 HQ (about 90 MB/s), plus a seek every 2 seconds to
  another file of the folder, as when scrubbing a timeline, for 30 minutes.
- **Runs.** The workload alone (pigeon stopped), then the workload while
  pigeon captures and syncs, on Linux with BFQ, on Linux with `mq-deadline`,
  and on macOS.
- **Pass** if, in every run with pigeon, the workload's throughput and its
  99th-percentile read latency stay within 10 % of the run without pigeon,
  pigeon still makes progress on capture, and no transfer is abandoned
  because of `SIGSTOP`.
- If `mq-deadline` fails while BFQ passes, `pigeon doctor` recommends BFQ for
  hard disks (ADR 0037).

E17 runs on a self-hosted runner with a hard disk, in `models-full`
([13 §13.8](13-verification.md)).
