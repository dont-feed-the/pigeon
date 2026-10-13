# 0037. Background work stays out of the way: low priority, adaptive limits, priorities, settings

- **Status:** Proposed, 13 October 2026, by the agent, for a requirement from
  Guillaume (chat: "having pigeon running in the background should not impede
  computer use … CPU, IO and network usage, with sensible default, adaptive
  behavior, customization capabilities, prioritization capabilities"), and
  his use case: video editing on a machine whose hard disk pigeon is syncing
  must not need pausing or settings.

## Context

`pigeond` runs all the time (ADR 0011). Its load comes from hashing new
content (`git annex add`), scans (`git status`, every 10 minutes, ADR 0013),
transfers, merges, and on backup machines scrubs (`git annex fsck`) that read
everything. git-annex offers `annex.jobs` (default 1), `annex.bwlimit` with
`-upload` and `-download` variants (not for remotes whose transfer runs in a
separate program), `annex.stalldetection` and `annex.diskreserve` (default
100 MB), and no priority between transfers (GIT-ANNEX.md). Every byte that
crosses the network goes through `pigeond`, which relays the iroh streams
(ADR 0011), so pigeon can shape traffic itself. Safety comes first: capture
(P2) must not wait for a good moment, while transfers, repair and scrubs can.

The hardest case is Guillaume's: a large folder on a hard disk (HDD), synced
while a video editor plays and scrubs files of that folder. On an HDD, two
sequential readers make the head seek between them, so even a little
background reading can halve the editor's throughput; reading for hashing also
evicts the editor's frames from the page cache. Kernel priorities help only
partly: Linux honours the idle I/O class with the BFQ scheduler, not with
`mq-deadline` or `none`; macOS throttles background-tier I/O. cgroup v2 charges
page cache to the cgroup that read it, so a memory limit on pigeon's unit makes
the kernel reclaim pigeon's pages first.

## Decision

1. **Background priority by default.** On laptops, the systemd user unit
   sets `Nice=10`, `IOSchedulingClass=idle`, `CPUWeight=20`, `IOWeight=20`;
   the launchd agent sets `ProcessType=Background`. Children inherit them.
   Backup machines run at normal priority, with scrubs in the idle I/O class.
2. **One work queue, ordered by priority**, run by `pigeond` instead of
   git-annex's own ordering: capture; exchange of history; uploads that bring
   a version toward protection (fewest custody copies first, then newest,
   then smallest); downloads of kept folders (folder priority, then recently
   changed first); repair; scrubs; rebalancing. Folder priority is a
   per-device setting: `high`, `normal` (default) or `low`.
3. **Adaptive limits.** A shaper in `pigeond`'s stream relay yields to other
   traffic: it backs off when the round-trip time of its streams grows above
   their baseline (a delay-based rule in the spirit of LEDBAT, RFC 6817) and
   grows slowly otherwise. Transfers and scrubs pause on battery below 20 %
   or in low-power mode, and on networks marked metered; capture continues.
   Scrubs on backup machines run in slices (`fsck --incremental
   --time-limit`), by default between 01:00 and 06:00 local time.
4. **Disk contention is detected, not configured.** `pigeond` watches
   the I/O pressure of the system (`/proc/pressure/io` on Linux) and the
   latency of its own reads (both platforms). When others are using the
   disk, it backs off by pausing its heavy children (`SIGSTOP`/`SIGCONT`,
   so git-annex's hashing, transfers and scrubs pause mid-file) and its own
   reads, and resumes when the disk is quiet. Only capture keeps a floor of
   progress (default 5 % of the time), so P2 never starves; transfers,
   repair and scrubs may wait as long as the disk is busy, and the status
   says "waiting: this computer's disk is busy" (ADR 0027). The systemd
   unit sets `MemoryHigh` so pigeon's page cache is reclaimed first;
   pigeon's own reads use `posix_fadvise(DONTNEED)` on Linux and
   `F_NOCACHE` on macOS.
5. **Large and hot files are damped.** The quiet period before capture
   grows with the file's size (default 2 s plus 1 s per 100 MB, at most
   60 s), and a path captured more than 6 times in an hour is captured at
   most every 10 minutes, so renders and autosaves do not fill history.
6. **Customisation, never required.** Settings → *Use of this computer*
   offers three presets (*Gentle*, the default; *Balanced*; *Fast*), and
   `config.toml` holds the details: upload and download limits (`auto` or
   a rate), behaviour on battery and on metered networks, time windows at
   full speed, folder priorities. `pigeon pause [DURATION]` stops
   everything but capture.
7. **Budgets are tested.** CI measures `pigeond` on a reference tree: idle
   below 1 % of one core averaged over 10 minutes, and its memory bounded;
   telemetry (ADR 0025) reports CPU time, bytes and queue depths per loop.
   Experiment E17 is the acceptance test for Guillaume's case, with default
   settings: a reference video workload (sequential reads at the bit rate of
   4K ProRes, plus seeks) on an HDD while pigeon captures and syncs a
   500 GB folder on it. It passes if the workload's throughput and its
   99th-percentile read latency stay within 10 % of a run with pigeon
   stopped, on Linux (BFQ and `mq-deadline`) and on macOS.

## Consequences

- pigeon issues transfers one key at a time from its queue (batch
  `get`/`copy`), and keeps `annex.jobs` at 1 on laptops.
- Platform probes: battery and metered state through UPower and
  NetworkManager over D-Bus (`zbus`) on Linux, IOKit and Network.framework on
  macOS. Missing information means "not on battery, not metered".
- A paused transfer is a reason in the status (ADR 0027): "Waiting for a
  non-metered network", so pausing never hides that files are not safe yet.
- Whether `annex.bwlimit` works over the P2P transport is to be checked
  (experiment E16); the shaper does not depend on it.
- Point 5 refines the quiet period of ADR 0013: 2 s stays the default for
  small files.
- Pausing git-annex with `SIGSTOP` must not break its locks or transfers
  beyond their stall detection; E17 checks it, and `annex.stalldetection`
  is set above the longest pause.
- With `mq-deadline` and no idle class, back-off is the only protection;
  E17 tells whether it is enough or whether pigeon must recommend BFQ for
  HDDs in `pigeon doctor`.
