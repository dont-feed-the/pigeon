# 0083. Supervisors apply native resource preset changes

- **Status:** Proposed, 18 October 2026.

## Context

ADR 0037 gives Gentle and Balanced background CPU and I/O priority and Fast
normal priority. The installed user service previously stayed at background
priority for every preset. A daemon cannot reliably raise its own nice value
back to normal without privileges; changing pooled worker-thread priorities
also leaks policy between jobs. A valid resource edit already applies to the
shared admission and shaping controller without restarting it.

## Decision

The validated resource reload writes a local, versioned `resource-priority`
hint in the device's state directory. Its only complete values are
`1 background\n` and `1 normal\n`. Equal modes do not rewrite it. An invalid
complete configuration leaves both the controller and hint unchanged. This
hint is a scheduling request, never content, membership or release authority.

The supplied user supervisor watches the path. Its installed `pigeon-service`
helper accepts only the complete bounded enum. On Linux it changes only
pigeon's priority drop-in, reloads systemd and restarts the active daemon.
On macOS it renders Background or Standard ProcessType and replaces the
launchd job only when the rendered agent differs. A fresh manager-created
process can receive normal priority without escalating the old daemon.
Gentle and Balanced share a native mode and do not restart one another.

Only a change of native mode restarts the daemon. Its existing termination
handler closes the node and stops remote daemons; normal crash-recovery and
content verification remain required after restart. Linux's cgroup retains
its normal child cleanup. System backup services keep their existing normal
priority and have no priority watcher. Other user drop-ins remain in place.
A manually started daemon still inherits its launcher's native policy.

The automatic updater remains independent: ADR 0076 still downloads the
latest installer afresh. The scheduling helper performs no update, download,
repository edit or privileged priority increase. Stopping an installation
removes its watch before removing its daemon configuration, retaining data.

## Consequences

A Fast/background switch briefly disconnects local clients and peers while
the replacement starts; it does not interrupt Gentle/Balanced switches.
An unavailable user manager cannot apply the native change. Runtime tests
must verify native priorities and child inheritance through actual systemd;
rendered units and mocked launchctl cannot establish macOS runtime behavior.
The reference-tree and HDD performance budgets remain separate measurements.
