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
The JSON also retains completed intervals between installable publications,
their timestamps, target results and sample count. Publishing resets the
current age; it does not erase an earlier missed interval. The readable report
shows the latest five intervals. Idle periods remain visible in these samples.

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

One v0.5.9 crash-schedule shard spent 38 minutes 59 seconds checking traces,
after 13 minutes 8 seconds executing its schedules. Linux slice verification
now checks two independent logs at a time in isolated processes, with private
generated models and TLC working directories. Every log, required family,
clock and failure still counts. The bound is two workers in an existing job;
macOS and ordinary local invocations retain one worker. Compare the next
release's timings before claiming a measured publication improvement.

A controlled local replay used eight copies of one recorded filesystem
trace and eight copies of the golden storage trace. Three alternating runs
per mode checked all 16 logs each time: the median was 18.488 seconds with
one worker and 11.226 seconds with two. These are local replay measurements;
the release comparison still needs the next candidate's CI evidence.


## v0.5.13 observation

[v0.5.13](https://github.com/dont-feed-the/pigeon/releases/tag/v0.5.13)
was published on 16 October at 21:09:26 UTC. Its candidate took **30 minutes
1 second** to publish, meeting the 60-minute target; v0.5.12 took 35 minutes
42 seconds. The completed publication gap was **2 hours 44 minutes 18 seconds**,
missing the two-hour target. That miss remains in the interval history.

The complete Linux slice runs are
[v0.5.12](https://github.com/dont-feed-the/pigeon/actions/runs/38942631283)
and [v0.5.13](https://github.com/dont-feed-the/pigeon/actions/runs/38963802150).
Their trace-checking steps took:

| Partition | v0.5.12, one worker | v0.5.13, two workers |
| --- | --- | --- |
| Remaining slice scenarios | 3m 15s | 1m 18s |
| S9 random shard 0 | 19m 21s | 16m 24s |
| S9 random shard 1 | 17m 49s | 1m 19s |
| S5 | 5s | 5s |

Both shard-1 logs retain 52 accepted filesystem checks and 26 accepted
roster checks. Each random shard still runs 25 schedules and its retained
regression. These are observations from two releases, with different event
histories, other code changes and runner conditions; they do not isolate
the workers' contribution. The controlled replay above supplies the separate
local comparison. There are now six candidate lead-time samples and five
completed publication intervals, too few for a reliable percentile claim.


## v0.5.14 observation

[v0.5.14](https://github.com/dont-feed-the/pigeon/releases/tag/v0.5.14)
was published on 16 October at 23:26:42 UTC, with all six required assets,
verified checksums and the public latest-release redirect. Its candidate
lead time was **16 minutes 33 seconds**, meeting the 60-minute target.
The completed publication gap was **2 hours 17 minutes 16 seconds**,
missing the two-hour target. That miss stays in the interval history.
There are seven candidate lead-time samples and six completed publication
intervals; these still do not establish a reliable percentile.

The source was the validated stable main before the file-manager rollup;
this release does not contain the file manager. Independent Linux gates
and both platforms' archives completed without waiting for the non-gating
full-model background run. Compared with v0.5.13's 30 minutes 1 second,
this is an observed improvement, not a controlled attribution to one change.

The user reported a GitHub Teams capacity of **60 concurrent runners** on
16 October. Independent validation and release jobs can use that capacity;
combined PR validation and cancellation of superseded runs still avoid
waste. The publication-gap and candidate-lead targets stay unchanged.


## v0.5.15 observation

[v0.5.15](https://github.com/dont-feed-the/pigeon/releases/tag/v0.5.15)
was published on 17 October at 00:27:06 UTC with all six required assets.
Its candidate took **18 minutes 31 seconds** to publish, meeting the
60-minute target. The completed publication interval from v0.5.14 was
**1 hour 24 seconds**, meeting the two-hour target. Prior missed intervals
remain in the history. The collector retained eight lead-time samples and
seven completed publication intervals; these are still not a reliable
percentile estimate.

Linux CI and lab completed before slice, which finished at 00:26:53 UTC.
Publication followed 13 seconds later. The increased runner allowance did
not waive any release check or the macOS archive requirement.
