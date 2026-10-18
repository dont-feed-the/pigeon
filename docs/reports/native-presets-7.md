# Native resource presets

> Mission: separate native priority transitions from internal shaping and the
> outstanding M6 hardware measurements.

## Problem and change

The user service previously retained its background CPU and I/O policy when
someone chose Fast. Internal shaping changed, but native process priority did
not. The supplied user supervisor now watches a bounded, versioned local mode
request from the validated resource reload (ADR 0083). Gentle and Balanced
share background mode. Fast requests normal mode. Only crossing that boundary
replaces the daemon; a fresh manager-created process receives the new priority.

Linux uses a dedicated priority drop-in, preserving other user configuration.
The helper reloads systemd and restarts only an active installed service.
macOS replaces Background/Standard launchd ProcessType only when the rendered
agent differs. System backup services retain their existing normal policy.
Repeated requests do not restart the process. Invalid resource configuration
keeps the previous request; malformed supervisor requests leave the current
native configuration alone. Stopping removes the watch before the daemon and
never removes identity or group data. Update-only and removal still work with
a malformed priority hint. The updater remains independent under ADR 0076.

The daemon writes only changed modes with atomic replacement. The previous
hint is read through the observed regular inode, with a 17-byte limit; a
symlink is refused without modifying its target. Neither the enum nor a
supervisor restart establishes content authority or verified custody.
Manually launched daemons still inherit their launcher's native policy.

## Validation in progress

The first four packaging regressions reproduce the missing operation: two
failures and two errors before implementation. All four pass after connecting
the helper. The subsequent complete packaging suite passes 32 checks,
including invalid-hint removal/update behavior and systemd’s actual parser. Its first broad invocation
could not initialize systemd-analyze's user manager without XDG_RUNTIME_DIR;
with a private 0700 runtime directory the same suite passes. This is an
environment prerequisite, not a product-policy change.

The initial two Rust checks pass, including a 1000-case law over preset
sequences: only crossing the native mode boundary writes, and the final hint
matches the final choice. The subsequent 21-check retake passes the real reload-loop, symlink,
configuration/preference, admission and native process-group checks, including
the 1000-case law. All-target Clippy passes that source. Later user-facing
restart wording has its own validation below.

`scripts/check-native-priority` is a required Linux packaging CI step. On a
disposable host with an actual user-systemd manager it exercises the real
path watch and atomic replacement, native background/normal/background
transitions, CPU/IO weights, child nice/I/O inheritance and repeated-choice
stability. It refuses pre-existing pigeon supervisor state. This workspace
has no running systemd manager, so local render/mocked-manager results do not
establish that native runtime verdict. The original CI result must pass before
merging; macOS dynamic runtime validation remains separate.

## Limits

The existing SIGTERM handler stops remote daemons and closes the endpoint;
normal recovery and content checks still precede resumed work. A mode switch
briefly disconnects clients and peers. Graceful signal handling alone does not
prove every Mac child termination interleaving. The native preset change does
not close the global queue, own-read latency feedback, supported-filesystem
coverage, reference-tree/HDD measurements or actual elapsed M7 gates.

English and French simulated wording reviews clarify that a page may need
a manual reload after the brief restart. The first English draft implied
automatic reconnection; the final text tells the user to wait briefly and
reload if disconnected. Both explain that Gentle/Balanced changes avoid a
restart. These are wording reviews, not actual browser-restart observations.

The first two ADR-index invocations rejected the new draft's heading and
status-line format. Both are corrected to the repository's existing format;
no accepted ADR is changed. Final structural checks follow those corrections.

The escaped-state-path parser check first rejected quoted `PathChanged`: that
setting takes an absolute literal path, unlike ExecStart’s argument syntax.
The final rendering preserves literal dollar signs/spaces and escapes systemd
percent specifiers, with no surrounding quotes. The same complete 32-check
packaging suite then passes. This original parser miss is not a runtime pass.

Current v0.6.17 main `ddf309744bddb89857af567ccadb7969b85a5af0` is integrated
before PR CI. Its incoming differences are only version/lockfile, changelog
and the preceding capture report. On that source, all 21 affected daemon
checks pass again, and all 99 selected web/i18n checks pass (one skip). These
include final bilingual restart text. Full workspace CI and the new actual
systemd priority fixture still precede merging.

## Release validation

PR #86, head `746e59f464e54820b14eed3b858e248e64faeac7`, passed exact-head CI
39083857077, lab 39083857124 and bridge
39083857192. Current v0.6.17 main was integrated
before validation. The local 21 daemon, 99 web/i18n and 32 packaging
checks, 1000-case mode law, all-target Clippy and repository checks remain
separate from hosted native systemd validation. PR packaging job
117546237019 passes actual user-systemd mode transitions: daemon and
child nice 10/idle I/O, then nice 0/best-effort priority 4, then back to
10/idle; CPUWeight/IOWeight 20, 100, 20. Atomic path replacements trigger
these changes, and equal replacements preserve MainPID. This is Linux
runtime evidence, not a Mac or hardware-budget result. The original missing-helper,
path-parser and draft-ADR formatting misses remain recorded above.
Main cache preparation is no longer an extra pre-tag gate for this
unchanged dependency lockfile. Active main Rust cache producers remain
protected from cancellation; all required tag CI/lab/slice gates and both
platform archives still precede publication.
v0.6.17 was published and independently verified before this candidate.
Mac dynamic supervisor behavior, the complete global queue, own-read
latency feedback, measured reference/HDD budgets and actual elapsed M7
gates remain open.

## Per-loop idle reconciliation follow-up

The initial adapter re-opened the native-priority hint for every valid
two-second configuration reload. Its observed-inode reader deliberately
requests uncached I/O, which is appropriate for large content but unnecessary
for an unchanged scheduling hint. A native filesystem fixture reproduces the
extra work: after the first request its state directory is moved away, and an
equal background-mode request unexpectedly recreates it. The original
assertion fails; after the correction it passes.

Each running resource loop now remembers its last successfully reconciled
mode, bound to its immutable state path. Equal modes return before filesystem
access. Gentle/Balanced changes share that entry. A new loop reconciles the
existing hint once; actual mode changes retain the bounded observed-inode
check and atomic durable write. Failed I/O clears the memo before any side
effect, because an atomic replacement may succeed before its directory fsync
fails. Retrying or reverting the preset therefore reconciles the real file.
The hint remains a local scheduling request, never authority.

A second 1000-case mode law checks per-loop idempotence against a temporarily
absent hint and exact bytes after every mode boundary. Concrete fixtures
cover unchanged reloads, failed requests, a later revert and fresh-loop
startup without rewriting matching bytes. All 24 affected daemon checks pass
in 10.999 seconds on integrated v0.6.18 main; all 35 protocol checks pass
in the same combined source, with all-target Clippy and no warnings. These
finite fixtures do not measure a device’s idle or foreground HDD budget.

## Periodic Mac maintenance runtime observation

Mac background run 39081473616 uses v0.6.16 source
`889da1814fae629deb26ab1ac78988ed1207a291`, before native presets. Workspace
job 117540787525 passes 749 checks (one skip) in 200.525 seconds. Its actual
Apple child-priority inheritance fixture passes, and
`scrub_checks_real_objects_at_idle_priority_and_keeps_a_durable_result`
passes in 4.343 seconds. All four native daemon shards pass their 53 checks
and subsequent trace validation. This is newer runtime evidence for the
maintenance fix; it is not Mac dynamic preset-switching or measured-budget
evidence. The workflow’s slice stage is in progress at this observation.
