# Architecture decision records

One file per decision, numbered in order, in the format of Michael Nygard
(*Context*, *Decision*, *Consequences*). Started on 13 October 2026, at
Guillaume's request; ADRs 0001–0004 record decisions taken earlier.

- **Status** is `Proposed`, `Accepted`, `Rejected` or `Superseded by NNNN`.
  It also says who decided, when, and where (chat, review, vote).
- An accepted ADR is never rewritten. A change of mind is a new ADR that
  supersedes it; the old one only gets its status updated.
- Evidence is cited by experiment (`E1`–`E10`, see [`lab/git-annex/`](../../lab/git-annex/)) or by
  model ([`models/tla/`](../../models/tla/)).

| # | Decision | Status |
|---|----------|--------|
| [0001](0001-git-annex-as-engine.md) | pigeon v1 is a policy and UX layer over git-annex | Accepted |
| [0002](0002-copy-classes-and-custodians.md) | Copy classes: untrusted thin laptops, semitrusted custodians, K custody copies | Accepted |
| [0003](0003-partial-custodians.md) | Custodians may be partial (`sizebalanced` + `maxsize`) | Accepted |
| [0004](0004-rust-driving-git-annex-cli.md) | pigeon is written in Rust and drives git-annex as a subprocess | Accepted |
| [0005](0005-platforms-v1.md) | v1 targets Linux and macOS; native Windows in v1.1 | Accepted |
| [0006](0006-iroh-transport-helper.md) | Transport: our own `git-annex-p2p-iroh` helper in Rust | Accepted |
| [0007](0007-device-keys.md) | One ed25519 key per device: iroh identity and git signing key | Accepted |
| [0008](0008-trace-validation.md) | Implementation traces are validated against the TLA+ specifications | Accepted |
| [0009](0009-roster-in-tree-proposals-as-refs.md) | The roster lives in the tree; proposals are git refs | Accepted |
| [0010](0010-git-cli-only.md) | pigeon reaches git through the git command line only | Accepted |
| [0011](0011-single-daemon.md) | One `pigeond` daemon per user; CLI and transport helper are thin clients | Accepted |
| [0012](0012-property-based-testing-and-laws.md) | Property-based tests first; the core is built from algebras with laws | Accepted |
| [0013](0013-change-detection.md) | Change detection: events are hints, the scan is the truth | Accepted |
| [0014](0014-append-only-serving.md) | Custodians serve peers append-only: no remote `REMOVE` | Accepted |
| [0015](0015-new-design-ignores-0.9.md) | The new design does not build on pigeon 0.9.0 | Accepted |
| [0016](0016-relays-and-discovery.md) | Relays and discovery: n0 by default, mDNS on the LAN, own relay optional | Accepted |
| [0017](0017-joining-by-ticket.md) | Joining by a one-time ticket; people manage their own devices | Accepted |
| [0018](0018-deletion-keeps-custody-until-purge.md) | Deleting a file never deletes its custody copies; only a purge does | Accepted |
| [0019](0019-licence.md) | pigeon v2 is licensed MIT OR Apache-2.0 | Accepted |
| [0020](0020-git-annex-delivery.md) | git-annex comes from its official builds, with a checked minimum version | Accepted |
| [0021](0021-cli-and-local-web-ui.md) | Interfaces: a CLI and a local web interface served by `pigeond` | Accepted |
| [0022](0022-logging-with-tracing.md) | Logging: `tracing`, human logs to stderr, JSON logs to a file | Accepted |
| [0023](0023-typed-errors.md) | Errors: typed enums everywhere; one serialisable problem for people | Accepted |
| [0024](0024-cli-with-clap.md) | The command line: `clap`, `--json` everywhere, stdout for results | Accepted |
| [0025](0025-telemetry-and-dashboards.md) | Telemetry: OpenTelemetry over iroh to an optional group collector; two dashboards | Accepted |
| [0026](0026-continuous-integration.md) | Continuous integration: GitHub Actions checks rules and model verdicts | Accepted |
| [0027](0027-plain-words-and-one-headline.md) | People see plain words and one headline status | Accepted |
| [0028](0028-messages-with-fluent.md) | Messages come from one Fluent catalogue, in French and English | Accepted |
| [0029](0029-status-icon-and-calm-notifications.md) | Calm notifications and a launcher in v1; the status icon waits for v1.1 | Accepted |
| [0030](0030-read-only-files-you-may-not-change.md) | Files you may not change are read-only on your disk | Rejected |
| [0031](0031-free-up-space-per-folder.md) | Space is freed per folder, with git sparse checkout | Accepted |
| [0032](0032-backup-machines-on-linux.md) | Backup machines run Linux only in v1 | Accepted |
| [0033](0033-editable-files-edits-become-suggestions.md) | Files you may not change stay editable; a saved edit becomes your suggestion | Superseded by 0034 |
| [0034](0034-never-overwrite-what-a-person-saved.md) | pigeon never overwrites what a person saved; a diverging file is a conflict they resolve | Accepted |
| [0035](0035-group-version-shown-only-in-pigeon.md) | During a conflict, the group's version is shown only in pigeon | Accepted |
| [0036](0036-local-state-in-plain-files.md) | `pigeond` keeps its local state in plain files | Accepted |
| [0037](0037-background-work-stays-out-of-the-way.md) | Background work stays out of the way: low priority, adaptive limits, priorities, settings | Accepted |
| [0038](0038-deciders-sign-decisions-on-suggestions.md) | Deciders sign decisions on suggestions; authors withdraw their own | Proposed |
| [0039](0039-freeing-and-keeping-never-show-a-pointer.md) | Freeing and keeping a folder never show a pointer file: hide before dropping, stage before showing | Proposed |
| [0040](0040-install-proves-its-own-file-and-racing-saves-win.md) | Install proves a file is its own before replacing it, and a racing save always wins | Proposed |
| [0041](0041-captured-versions-stay-immutable-until-protected.md) | A captured version stays immutable on its laptop until it is protected; thin only after | Proposed |
| [0042](0042-purge-releases-references-and-completes-after-every-seal.md) | A purge releases the references its deciders saw; content goes only when no other reference holds it and every custodian has sealed | Proposed |
| [0043](0043-every-device-serves-peers-through-a-filter.md) | Every device serves peers through pigeond's filter: pull only, append-only on custodians, drops with explicit copy counts | Proposed |
| [0044](0044-share-regular-files-and-say-what-is-not-shared.md) | pigeon shares regular files and their executable bit, and says what it does not share | Proposed |
| [0045](0045-git-reads-only-the-configuration-pigeon-writes.md) | git and git-annex read only the configuration pigeon writes | Proposed |
| [0046](0046-protected-means-content-and-history-on-disk.md) | Protected means K custodians hold the content and the commit that names it, on disk | Proposed |
| [0047](0047-recover-from-custodians-sheets-refound-export.md) | A group is recovered from its custodians: rejoin, recovery sheets, refounding, and export without pigeon | Proposed |
| [0048](0048-unknown-formats-and-rules-are-never-guessed.md) | Replicated formats and rules are versioned; what a device does not understand, it never guesses | Proposed |
| [0049](0049-copies-count-once-per-failure-domain.md) | Copies count once per failure domain: shared disks, machines and declared domains | Proposed |
| [0050](0050-capacity-is-budgeted-reserved-and-admitted.md) | Capacity is budgeted, real free space is reserved, and endangered content goes first | Proposed |
| [0051](0051-the-roster-is-a-fold-of-its-changes.md) | The roster is a fold of its changes in a canonical order; roster forks merge without an admin | Proposed |
| [0052](0052-catalogue-from-contributions-and-the-conflicts-they-saw.md) | The catalogue is built from contributions and the conflicts they saw; merge commits never count | Proposed |
| [0053](0053-trace-logs-as-generated-tla-modules.md) | Trace logs reach TLC as generated TLA+ modules; TLC stays at 1.7.4 | Proposed |
| [0054](0054-a-copy-counts-only-if-it-can-be-locked.md) | A copy counts only if it can be locked: custodians answer `CHECKPRESENT` with a lock | Proposed |
| [0055](0055-install-recovery-rolls-forward-from-a-journalled-proof.md) | Install recovery rolls forward from a journalled proof | Proposed |
| [0056](0056-every-experiment-runs-on-both-platforms.md) | Every asserting experiment runs on Linux and macOS; a platform difference is shown, never hidden | Proposed |
| [0057](0057-the-first-milestone-is-a-safety-slice.md) | The first milestone is a safety slice: one laptop, two custodians, no proposals or purge | Proposed |
| [0058](0058-the-rust-workspace-has-a-pure-core-and-a-pinned-toolchain.md) | The Rust workspace has one crate per row of 14 §14.6, a pure core and a pinned toolchain | Proposed |
| [0059](0059-a-custodians-drop-counts-only-hashed-copies.md) | A custodian's drop counts only copies hashed for it, and repair counts only members that can take a key | Proposed |
