# Release publication KPI

The delivery event is a published, non-draft GitHub release with both
platform archives, pinned git-annex archives, installer and checksums
present. Tags, merged PRs and successful builds do not count. Asset presence
is a measurement prerequisite, not a replacement for release validation.
Published prereleases count, and their prerelease status remains explicit.

During active release work, target a publication gap of at most **2 hours**
and candidate-to-publication time of at most **60 minutes**. These are
improvement targets, not existing performance claims or permission to omit
checks. Cold builds remain in the measurements. Show sample counts; do not
infer a reliable percentile from one or two publications. Idle periods are
visible in the clock and do not imply that an unnecessary release is due.

Run `python3 scripts/release-kpi --json-output /tmp/release-kpi.json`.
The report names unpublished candidates, current failed steps, and job waits,
including failures hidden inside an unfinished workflow. Wait before start
includes dependency and runner waits because the API does not distinguish
them reliably. The default workflow window is seven days; `--days` changes
it. Draft releases and publications missing assets do not reset the clock.
The collector exits unsuccessfully on an API error instead of inventing data.

The `release-kpi` workflow records a JSON artifact and a readable summary
after each release attempt and hourly. Its one short, read-only Ubuntu job
does not build Rust or run models. A missed target produces a warning and
does not block publication. Retained artifacts permit comparisons over time.

## Initial observation and targeted changes

At 2026-10-16 12:47 UTC, the last installable publication was
[v0.3.6](https://github.com/dont-feed-the/pigeon/releases/tag/v0.3.6),
published at 06:40:24 UTC: a **6 hour 7 minute** gap. Its release workflow
started at 03:03:41 UTC: **3 hours 36 minutes** from candidate to publication.
v0.4.0, v0.5.0, v0.5.1 and v0.5.2 had failed or cancelled publication
attempts; v0.5.5 was still running. No later tag was called a release.

Focus the next improvements on observed delays:

1. Finish the current candidate before creating another tag. Retry only a
   failed job after its verified dependency becomes available. Keep tags
   immutable, and rebase PRs before rerunning them.
   A validated stable release can become latest even if development on main
   has advanced. An older retry cannot displace a newer published version.
2. Retain the corrected macOS case-insensitive-folder and short-socket
   fixtures, and wait for asynchronous telemetry before asserting it. These
   failures were discovered only at release validation after Ubuntu PR CI.
3. Make verified dependency restoration tolerate short transport failures
   and report a useful failure class. Keep source provenance, producer
   success, digest, binary architecture and version checks intact.
4. Compare candidate job waits and execution before/after each change. Roll
   up source changes into one CI run, cancel superseded jobs, and preserve
   every required release gate.

On 16 October the user authorized moving macOS tests out of release gates.
Linux CI, lab and slice jobs, including required crash schedules, still gate
publication. macOS dependency and application archives still have to build.
`macos-background` serializes dependency proof, workspace, lab and slice
groups nightly; it defers while a release is active. Cross-platform reach
runs daily and by hand. macOS failures remain visible background evidence.
For an immutable older tag, the current dispatch workflow may use its exact
tag/commit's successful Linux jobs while ignoring macOS test state. Missing,
failed, skipped or cancelled required Linux jobs never authorize publication.
