# 0013. Change detection: events are hints, the scan is the truth

- **Status:** Accepted, 13 October 2026, by Guillaume (tooling interview, Q7).

## Context

Capture (P2) starts when pigeon notices a change. Filesystem events are
lossy: inotify drops events when its queue overflows and is limited by
`fs.inotify.max_user_watches`; FSEvents coalesces events and may only say
"rescan this directory". Applications save through temporary files and
renames, and large files are written over seconds. git already compares the
working tree with the index from stat data, and git-annex's clean filter
hashes only files whose stat changed.

## Decision

- `pigeond` watches each repository with the `notify` crate (inotify on
  Linux, FSEvents on macOS). An event only marks a path as dirty.
- A dirty path is captured after a quiet period (default 2 s) and two
  consecutive equal `stat` results (size, mtime, inode).
- What changed is decided by `git status --porcelain=v2 -z`, never by the
  events themselves.
- A full scan runs at start-up, periodically (default every 10 minutes),
  and at once when the watcher reports an overflow or a rescan flag.
- On macOS, `core.fsmonitor=true` (git's built-in monitor) speeds up
  `git status`. Linux has no built-in monitor; the scan stays as it is.
- If the watch limit is reached, pigeon falls back to scans and shows the
  folder as "watched by scan only" (P6).

## Consequences

- Law (ADR 0012): the captured catalogue depends on the filesystem state at
  scan time, not on which events arrived; dropping or duplicating events
  changes latency only. Tested by randomly dropping events.
- Writes between two captures remain best effort, as P2 already says.
- Capture latency is at least the quiet period.
