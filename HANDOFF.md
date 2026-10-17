# pigeon v2: handoff

## Continuation on 17 October 2026

The historical snapshot below is retained for context. The user has since
authorized the whole roadmap through v1.1, pushes, green non-draft PR merges,
and publication. M0–M5 are released; the latest verified publication is
[v0.6.8](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.8), source
`67db09aaa00eeef134d053b7b762fb5bf50071d1`. The failed v0.6.0 tag remains
immutable and unpublished. See [M5 evidence](docs/reports/deletion-6.md).

M6 is in progress; [its report](docs/reports/resources-7.md) distinguishes
implemented controllers and settings from outstanding native scheduling,
filesystem checks and measured reference/HDD budgets. M7 still needs its
real elapsed gates and the broader law/trace audit. The seed-104 concrete-name
refinement and its validation are recorded
in [the trace report](docs/reports/authority-trace-7.md). Finite fixtures do
not establish elapsed nights, soak, dogfood or a month.

The user accepts agent usability sessions and considers prior dogfood done.
Declined suggestions require explicit erasure. GitHub Teams permits 60
concurrent runners; roll up validation and cancel obsolete runs. macOS tests
are periodic background evidence; both platform archives and all required
Linux gates still precede publication. Track actual asset-bearing releases
using [the publication KPI](docs/release-kpi.md), not tags.

## Historical snapshot

State on 2026-10-16 at 01:00 CEST. Repository: `dont-feed-the/pigeon`, branch
`main` at `74d55da`.

## Where things are

- **Read first:** `README.md`, `docs/development.md`, `docs/roadmap.md`,
  `docs/spec/README.md`, `docs/adr/README.md`, and the milestone reports in
  `docs/reports/`.
- **Milestones:**
  - M0 (v0.1.0) and M1 (v0.2.0) are done and tagged.
  - **M2 (v0.3)** is complete in code but not yet tagged (see "Finishing M2").
  - M3–M7 have not started. Mario asked to start no M3 or M4 work until he
    says so.
- **v1.1 scope** is listed in `docs/roadmap.md`, section "After v1.0".

## Finishing M2 (v0.3.0)

1. Wait for **models-full run 38810170422** (on `76bd044`) to pass. At
   01:00 it had 24 of 26 jobs green and 2 still running. It is the
   1000-schedule, 200-trace evidence for v0.3.0.
2. Commit the edits to `docs/reports/people-3.md`. They sit uncommitted in
   the previous agent's worktree, which commits them once the run passes;
   if it cannot, redo them:
   - "What M2 found" gains two bullets: the TLC tmpdir race fixed in
     `76bd044`, and the S11 aside keep period fixed by the early release
     in `601da51`;
   - "Release" cites run 38810170422, and the push/weekly split of
     models-full made in `74d55da`.
3. Push the commit and wait for its ci, lab and slice runs to pass.
4. Tag it: `git tag -a v0.3.0 <commit>`, then push the tag. A `v0.N.0` tag
   marks the end of a milestone; it is not a patch release.
5. Comment on and close issues #1, #2, #3, #6, #7 and #8. Each comment
   names the change that resolved the issue and links
   `docs/reports/people-3.md`.

## Open pull requests at 01:00

| PR | Author | State | What to do |
|---|---|---|---|
| #20 Nextcloud bridge | Guillaume | draft, head 7dd95a1 | Guillaume asked for a merge once its CI passes. It fails `crates/pigeond/tests/decision_retry.rs:180` and the real Nextcloud client step of `scripts/check-nextcloud-bridge`. Merge with `--no-ff` on top of `main`; the bridge's capture path may bypass held names. |
| #21 Daemon traces vs TLA+ | Guillaume | ready, head 4ae159f | CI was queued. Merge once it is green. |
| #22 TLAPS proofs | Guillaume | draft | Wait. |
| #23 clap CLI and completions | Mario | ready, head c2b7f82 | CI was queued. Merge once it is green. |

Merge only PRs that are not drafts and whose CI is green. Rebase or merge
`main` into them first if `main` has moved.

## Rules the project follows

- **ADRs** (Nygard format) only for important decisions. Accepted ADRs are
  never rewritten; supersede them instead. The next free number is
  **0073**.
- **Core logic** is written as algebras with stated laws. Each law gets a
  property test with hegel (`hegeltest` 0.49) of at least 1000 cases.
- **Background work** must not impede the computer's use (ADR 0037).
- **git-annex behaviour** is cited from its sources (10.20261006).
- **pigeon 0.9** is an older design kept under the tag `v0.9.0`; ignore it.
- **Pre-commit hooks** check:
  - every file stays under 1000 lines;
  - every file opens with a header (Rust `//!`, at most 20 lines);
  - the ADR index;
  - references into the spec.
- **Before each commit:**
  - `cargo fmt --all`;
  - `cargo clippy --workspace --all-targets --features pigeond/lab,pigeon-core/tla-trace`
    with 0 warnings;
  - each `#[allow]` carries a `reason`.
- **Tests:** `PIGEON_REQUIRE_GIT_ANNEX=1 cargo test --workspace --features pigeond/lab`
  takes 5–10 minutes.
- **Lab scenarios:** build with `--features pigeond/lab,pigeond/tla-trace`,
  then run `cargo run -p pigeon-lab -- run --bin DIR --work DIR --out DIR S11`.
- **Releases:** after a nontrivial change with green CI, tag a patch release
  (v0.3.x).
- **CI:**
  - models-full runs 200 schedules and 50 seeds on each push, and 1000
    schedules and 200 seeds weekly or by hand;
  - pushing to `main` does not cancel a running models-full;
  - cancelling stale ci, lab or slice runs of superseded commits is fine.
- **Flakes:** retry, and note each miss. The E12/E13 lab flakes are stock
  git-annex relay races (spec 16 §16.4).

## Gaps found but not yet written down anywhere

Add these to the next report or to issues:

- **Models:** held names are not modelled in `CaptureInstall.tla`.
- **Deletion counting:** a package should count as one deletion.
- **Held changes:** held links are not covered.
- **Leaving a group:**
  - deleting a folder on leave is not handled;
  - leave does not say how many files lose their safe copies.
- **Status:** the drop-risk message lacks machine names and times.
- **Joining:**
  - a backup machine cannot join in place;
  - the web UI does not ask before joining a refounded group, and has no
    join page at all.
- **Web UI:** notifications and the launcher (ADR 0029) are missing, and the
  usability test of M4 has not been run.

## Daemon round order

The rounds live in `crates/pigeond/src/daemon.rs`. Each step is a
telemetry child span.

1. locks
2. `settings::check`
3. `ensure_remotedaemon`
4. `reload_roster`
5. `recovery_txt::write`
6. `leave::round`
7. `use_relays`
8. `[recover]`
9. `asides::release`
10. `free_targets`
11. `keep`
12. `capture`
13. If paused, the round stops here.
14. `accept`
15. `install`
16. `report::receive`
17. `note_lost`
18. `place`
19. `rethin`
20. `repair`
21. `publish`
