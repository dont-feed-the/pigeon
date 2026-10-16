# Architecture decision records

One file per decision, numbered in order, in the format of Michael Nygard
(*Context*, *Decision*, *Consequences*). Started on 13 October 2026, at
Guillaume's request; ADRs 0001–0004 record decisions taken earlier.

## When to write an ADR

An ADR records a decision that shapes the system and whose reasons a later
reader would otherwise lose. Write one only when all three answers are yes:

1. **Is there a real alternative?** Another option that a reasonable person,
   knowing what we know, might choose. A rule whose alternative breaks a
   promise (P1–P6) or loses data is not a choice: it is a safety invariant,
   and the specification states it, the models and tests check it.
2. **Is it costly to reverse?** Undoing it would mean migrating data or
   formats, breaking peers that run an older version, changing what people
   were told was safe, or rewriting several crates.
3. **Does it change, rather than refine, what is already decided?** A rule
   that applies or sharpens an accepted ADR without changing it belongs to the
   specification, which cites that ADR.

Typical ADRs: the engine and the language; a protocol, wire or on-disk format;
the meaning of a guarantee people rely on (what *protected* means); how
authority is computed; a platform supported or not; a major dependency the
rest is built on.

These go to the specification instead, marked **[no ADR]** where they are
decided and listed in [16 §16.3](../spec/16-open-questions.md):

- safety invariants and the mechanisms that carry them (install's proof,
  freeing without pointers, the serving filter, lock-checked copies);
- refinements of accepted ADRs (purge by references, capacity order);
- the scope of v1 (which file kinds are shared), scheduling and resource
  policy, test plans, milestones, experiment and CI policy;
- tooling, layout and pins that are cheap to change while the code is young
  (the trace pipeline, the workspace layout, library picks and versions);
- implementation choices inside one crate, which its code documentation
  explains.

When in doubt, write the specification first; raise a rule to an ADR when a
reviewer asks for it, or when it turns out costly to reverse. While an ADR is
Proposed, a refinement revises it; once it is Accepted, a change of the
decision is a new ADR that supersedes it.

## Status

- **Status** is `Proposed`, `Accepted`, `Rejected`, `Superseded by NNNN` or
  `Withdrawn`. It also says who decided, when, and where (chat, review, vote).
- An accepted ADR is never rewritten. A change of mind is a new ADR that
  supersedes it; the old one only gets its status updated.
- A Proposed ADR may be revised until it is decided; its status line says
  when and why.
- `Withdrawn` marks a Proposed ADR that is not kept as a record of its own:
  merged into another ADR, or moved to the specification under the policy
  above. It keeps its number and file, as a short stub that says where its
  content went, so that numbers are never reused and old links still lead
  somewhere.
- Evidence is cited by experiment (`E1`–`E25`, see [`lab/git-annex/`](../../lab/git-annex/)) or by
  model ([`models/tla/`](../../models/tla/)).

## Index

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
| [0020](0020-git-annex-delivery.md) | git-annex comes from its official builds, with a checked minimum version | Superseded by 0069 |
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
| [0046](0046-protected-means-content-and-history-on-disk.md) | Protected means K custodians hold the content and the commit that names it, on disk | Accepted |
| [0047](0047-recover-from-custodians-sheets-refound-export.md) | A group is recovered from its custodians: rejoin, recovery sheets, refounding, and export without pigeon | Accepted |
| [0048](0048-unknown-formats-and-rules-are-never-guessed.md) | Replicated formats and rules are versioned; what a device does not understand, it never guesses | Accepted |
| [0051](0051-the-roster-is-a-fold-of-its-changes.md) | Authority is a fold of signed changes: roster forks merge without an admin, and deciders sign decisions on suggestions | Accepted |

## Withdrawn

Withdrawn on 14 October 2026 at Guillaume's request, under the policy above, in two
passes; 16 §16.3 lists where each rule now lives.

| # | Decision | Status |
|---|----------|--------|
| [0038](0038-deciders-sign-decisions-on-suggestions.md) | Deciders sign decisions on suggestions; authors withdraw their own | Withdrawn |
| [0039](0039-freeing-and-keeping-never-show-a-pointer.md) | Freeing and keeping a folder never show a pointer file: hide before dropping, stage before showing | Withdrawn |
| [0040](0040-install-proves-its-own-file-and-racing-saves-win.md) | Install proves a file is its own before replacing it, and a racing save always wins | Withdrawn |
| [0041](0041-captured-versions-stay-immutable-until-protected.md) | A captured version stays immutable on its laptop until it is protected; thin only after | Withdrawn |
| [0042](0042-purge-releases-references-and-completes-after-every-seal.md) | A purge releases the references its deciders saw; content goes only when no other reference holds it and every custodian has sealed | Withdrawn |
| [0043](0043-every-device-serves-peers-through-a-filter.md) | Every device serves peers through pigeond's filter: pull only, append-only on custodians, drops with explicit copy counts | Withdrawn |
| [0044](0044-share-regular-files-and-say-what-is-not-shared.md) | pigeon shares regular files and their executable bit, and says what it does not share | Withdrawn |
| [0045](0045-git-reads-only-the-configuration-pigeon-writes.md) | git and git-annex read only the configuration pigeon writes | Withdrawn |
| [0049](0049-copies-count-once-per-failure-domain.md) | Copies count once per failure domain: shared disks, machines and declared domains | Withdrawn |
| [0050](0050-capacity-is-budgeted-reserved-and-admitted.md) | Capacity is budgeted, real free space is reserved, and endangered content goes first | Withdrawn |
| [0052](0052-catalogue-from-contributions-and-the-conflicts-they-saw.md) | The catalogue is built from contributions and the conflicts they saw; merge commits never count | Withdrawn |
| [0053](0053-trace-logs-as-generated-tla-modules.md) | Trace logs reach TLC as generated TLA+ modules; TLC stays at 1.7.4 | Withdrawn |
| [0054](0054-a-copy-counts-only-if-it-can-be-locked.md) | A copy counts only if it can be locked: custodians answer `CHECKPRESENT` with a lock | Withdrawn |
| [0055](0055-install-recovery-rolls-forward-from-a-journalled-proof.md) | Install recovery rolls forward from a journalled proof | Withdrawn |
| [0056](0056-every-experiment-runs-on-both-platforms.md) | Every asserting experiment runs on Linux and macOS; a platform difference is shown, never hidden | Withdrawn |
| [0057](0057-the-first-milestone-is-a-safety-slice.md) | The first milestone is a safety slice: one laptop, two custodians, no proposals or purge | Withdrawn |
| [0058](0058-the-rust-workspace-has-a-pure-core-and-a-pinned-toolchain.md) | The Rust workspace has one crate per row of 14 §14.6, a pure core and a pinned toolchain | Withdrawn |
| [0059](0059-a-custodians-drop-counts-only-hashed-copies.md) | A custodian's drop counts only copies hashed for it, and repair counts only members that can take a key | Withdrawn |
| [0060](0060-the-platform-layer-uses-rustix-and-libproc.md) | The platform layer uses rustix, with libproc on macOS; the lab seams are a self-kill and a file | Withdrawn |
| [0061](0061-freeing-hides-as-an-install-and-guards-every-drop.md) | Freeing hides a file as an install of its absence, and no drop takes a key that a kept path uses | Withdrawn |
| [0062](0062-a-removal-revokes-what-it-had-not-seen.md) | A removal revokes what the removed device signed that the removal had not seen | Withdrawn |
| [0063](0063-a-decision-is-a-merge-by-a-decider-of-its-main.md) | A decision is a merge by a decider of the main it is made on | Withdrawn |
| [0064](0064-pigeon-core-fixes-the-readings-its-laws-need.md) | pigeon-core's first algebras fix the readings of the spec that their laws need | Withdrawn |
| [0065](0065-runs-of-the-core-are-checked-by-tlc.md) | Runs of pigeon-core are checked by TLC before the drivers exist | Withdrawn |
| [0066](0066-the-helper-names-its-repository-and-the-filter-supplies-the-token.md) | The helper names its repository by path, and the serving filter supplies git-annex's token | Withdrawn |
| [0067](0067-the-git-driver-checks-every-command-and-bounds-every-call.md) | The git driver checks every command against the allow-list and bounds every call with a deadline | Withdrawn |
| [0068](0068-messages-are-listed-with-their-variables-and-checked-in-every-bundle.md) | Messages are listed with their variables and checked in every bundle; the language follows gettext's order | Withdrawn |
| [0069](0069-pigeon-runs-a-pinned-patched-git-annex.md) | pigeon runs a pinned, patched git-annex, built by its own CI | Accepted |
| [0070](0070-a-member-may-change-an-add-only-file-only-they-wrote.md) | A member may change an add-only file that only they wrote | Accepted |
| [0071](0071-a-save-soon-after-an-install-goes-beside-it.md) | A save within ten seconds of an install goes beside it | Accepted |
| [0072](0072-capture-records-renames-and-the-merge-follows-them.md) | Capture records renames, and the merge follows them | Accepted |
| [0073](0073-parameterized-proofs-complement-model-checking.md) | Parameterized safety proofs complement model checking | Proposed |
| [0074](0074-purge-waits-survive-restarts-and-delayed-approval.md) | Purge waits survive restarts and delayed approval | Proposed |
| [0075](0075-completed-purge-epochs-fence-custodian-membership.md) | Completed purge epochs fence custodian membership | Proposed |
