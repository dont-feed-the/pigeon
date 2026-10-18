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

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0513-observation).

## v0.5.14 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0514-observation).

## v0.5.15 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0515-observation).

## v0.5.16 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0516-observation).

## Additional runner capacity: 17 October

See [the retained observation](reports/release-kpi-history-through-v0636.md#additional-runner-capacity-17-october).

## v0.6.1 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v061-observation).

## Bridge integration baseline: 17 October

See [the retained observation](reports/release-kpi-history-through-v0636.md#bridge-integration-baseline-17-october).

## PR #70 follow-up observations

See [the retained observation](reports/release-kpi-history-through-v0636.md#pr-70-follow-up-observations).

## v0.6.2 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v062-observation).

## First dependency-cache observations after PR #71

See [the retained observation](reports/release-kpi-history-through-v0636.md#first-dependency-cache-observations-after-pr-71).

## v0.6.3 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v063-observation).

## v0.6.4 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v064-observation).

## v0.6.5 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v065-observation).

## v0.6.6 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v066-observation).

## v0.6.7 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v067-observation).

## v0.6.8 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v068-observation).

## v0.6.9 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v069-observation).

## v0.6.10 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0610-observation).

## v0.6.11 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0611-observation).

## v0.6.12 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0612-observation).

## v0.6.13 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0613-observation).

## v0.6.14 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0614-observation).

## v0.6.15 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0615-observation).

## v0.6.16 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0616-observation).

## v0.6.17 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0617-observation).

## v0.6.18 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0618-observation).

## v0.6.19 blocked candidate

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0619-blocked-candidate).

## v0.6.20 observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0620-observation).

## Native cache transfer checkpoint

See [the retained observation](reports/release-kpi-history-through-v0636.md#native-cache-transfer-checkpoint).

## Saved dependency archive observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#saved-dependency-archive-observation).

## v0.6.22 publication and next critical-path target

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0622-publication-and-next-critical-path-target).

## Verified publications at 05:01 UTC, 18 October

See [the retained observation](reports/release-kpi-history-through-v0636.md#verified-publications-at-0501-utc-18-october).

## v0.6.27 publication observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0627-publication-observation).

## v0.6.29 publication observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0629-publication-observation).

## v0.6.30 publication observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0630-publication-observation).

## v0.6.31 publication observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0631-publication-observation).

## v0.6.32 and v0.6.33 publication observations

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0632-and-v0633-publication-observations).

## v0.6.34 publication observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0634-publication-observation).

## v0.6.35 publication observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0635-publication-observation).

## v0.6.36 publication observation

See [the retained observation](reports/release-kpi-history-through-v0636.md#v0636-publication-observation).

## v0.6.37 publication observation

[v0.6.37](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.37), source
`79d290b6f3d7885f76be199630d29e6faeaba1bb`, is published at 12:08:09 UTC
on 18 October. Six assets, four archive checksums, native Linux installation,
pinned annex and completions are independently verified. All four tag gates
pass on their first attempt. Candidate lead is 642 seconds; the completed
publication interval is 2,855 seconds. Both targets are met, with 43 lead
samples and 42 completed intervals. The bounded pointer proof reduces
129 Git processes to three in its controlled fixture; full reference and
foreground-HDD budgets remain separate, unproved gates.

## v0.6.38 publication observation

[v0.6.38](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.38), source
`24dcd39b382b7e3a961b8554c5002c6a15d04a4d`, is published at 12:40:06 UTC
on 18 October. Independent verification passes six assets, four archive
checksums, pinned annex, native Linux installation and completions. All
four tag gates pass first attempt. Candidate lead is 546 seconds; the
publication gap is 1,917 seconds. Both targets are met, with 44 lead samples
and 43 completed intervals. The corrected focused Mac run passes all six
cases on integrated PR #107; full periodic run 39128408817 is separate,
pending background evidence. Cache-save reordering is not assigned a
causal end-to-end speedup from this one bundled publication.

## v0.6.39 publication observation

[v0.6.39](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.39), source
`9f8929e67e0ee8d61b0f9f04ee051a77244993f8`, is published at 13:12:11 UTC
on 18 October. Six assets, four archive checksums, pinned annex, native Linux
installation and completions are independently verified. All four tag gates
pass first attempt. Candidate lead is 669 seconds and publication gap is
1,925 seconds, meeting both targets. There are 45 lead samples and 44 completed
intervals. Preparation observability improves diagnosis; the full resource
and elapsed gates remain open.

## v0.6.40 publication observation

[v0.6.40](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.40), source
`1dbcf729a7f33de98afbe215c309371963d676d8`, is published on 18 October at
14:00:50 UTC. Six assets, four archive checksums, pinned annex, native Linux
installation and completions pass independent verification. All four tag
gates pass first attempt. Candidate lead is 689 seconds and publication gap
is 2,919 seconds, meeting both targets, with 46 lead samples and 45 completed
intervals. The host interruption is recorded separately and supplies no
continuous-soak credit. Initial per-device admission isolation ships in this
release; broader security and elapsed qualification remain separate.

## v0.6.41 publication observation

[v0.6.41](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.41), source
`c08b5274cc99a5e89fee4c7bf2c952ce69f159a5`, is published on 18 October at
14:22:03 UTC. Independent checks pass six assets, four archive checksums,
pinned annex, native Linux installation and completions. All four tag gates
pass first attempt. Candidate lead is 901 seconds and publication gap
is 1,273 seconds, meeting both targets, with 47 lead samples and 46 completed
intervals. Mode correction preserves staged pointers and avoids its clean
filters; the full diagnostic still expires in ingestion. No full-workload
speedup, resource-budget pass or elapsed qualification is inferred.

## v0.6.42 publication observation

[v0.6.42](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.42), source
`9b5588ff669dc1f889d43ff8be870164758a99bb`, is published on 18 October at
14:53:00 UTC. Independent verification passes six assets, four archive
checksums, pinned annex, native Linux installation and completions. All four
tag gates pass first attempt. Candidate lead is 625 seconds and publication gap
is 1,857 seconds, meeting both targets, with 48 lead samples and 47 completed
intervals. Four finite mixed-release lab cases support the upgrade checkpoint;
actual old archives and final-RC coverage remain distinct. The longer Fast
reference diagnostic and actual soak continue without an inferred pass.

## v0.6.43 and v0.6.44 observations

[v0.6.43](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.43),
source `dde2472b1c2e1ccd20216943ec2931aca1b0e256`, publishes on 18 October
at 16:03:32 UTC. Candidate-to-publication time is **657 seconds (10m57s)**;
the completed gap from v0.6.42 is **4232 seconds (1h10m32s)**. The collector
has 49 lead-time samples and 48 completed intervals. Both targets are met.

[v0.6.44](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.44),
source `31e86654127e9733229026d2a7b8637c1bc8eee3`, publishes at 16:21:21 UTC.
Its lead time is **666 seconds (11m06s)** and its gap is **1069 seconds
(17m49s)**, with 50 lead-time samples and 49 completed intervals. Both
targets are met; earlier misses remain in the retained history.

Both candidates pass ci/lab/slice/release gates on the first attempt. Each
independent publication check verifies six assets, all four archive hashes,
native installation, pinned annex, completions and the public latest target.
PR #112 combines published-archive and byte-observer work in one CI batch;
PR #113 validates against current main once, then waits for the preceding
publication verification before merging. The separate 90-minute Fast
reference diagnostic fails preparation and does not supply a budget pass.
Its failure is retained and does not redefine publication throughput.

## v0.6.45 publication observation

[v0.6.45](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.45),
source `95f2643a2fbc7aa6b9b8e15ddc6a6b5391055e97`, publishes on 18 October
at 17:16:28 UTC. Candidate lead is **832 seconds (13m52s)**; the completed
gap is **3307 seconds (55m07s)**. Both targets are met, with 51 lead-time
samples and 50 completed intervals. All four tag gates pass first attempt.
Independent verification passes six assets, four archive checksums, native
Linux installation, pinned annex, completions and latest. The release ships
bounded metadata startup and reusable fresh worker lookups. It supplies no
full resource/HDD qualification. The changed-source 90-minute Fast diagnostic
and frozen actual soak remain separate evidence.

## v0.6.46 publication observation

[v0.6.46](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.46),
source `95c30ff69ec7a815453e1358b52c75df76b45ac4`, publishes on 18 October
at 17:56:47 UTC. Candidate lead is **653 seconds (10m53s)**; its completed
gap is **2419 seconds (40m19s)**. Both targets are met, with 52 lead-time
samples and 51 completed intervals. All four tag gates pass first attempt;
independent checks verify six assets, four archive checksums, native Linux
installation, pinned annex, completions and latest. PR #115 folds temporary
grouping, weighted cached views and borrowed comparisons into one final
validation and publication. Its initial grouping-only CI also passed;
that head's unfinished bridge run was cancelled for the changed-source rollup.
No unchanged-source CI retry is used. The pure-engine allocation improvement
is separate from the still-open full resource/HDD budgets and real elapsed gates.

## v0.6.47 publication observation

[v0.6.47](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.47),
source `16f0bfac5a2b84f020a0f26f782ec21d8e698c45`, publishes on 18 October
at 18:32:05 UTC. Its release workflow starts at 18:21:20 UTC: candidate lead
is **645 seconds (10m45s)**, and the completed gap is **2118 seconds (35m18s)**.
Both targets are met. All four tag gates pass on their first attempt. Six
assets, four archive checksums, native Linux installation, pinned annex,
completions and latest are independently verified. The CLI credential expires
during publication; the connected GitHub app confirms the completed gates,
and public HTTPS supplies the release assets for the same verification.
The aggregate collector cannot refresh while its CLI authentication fails;
the previous snapshot has 52 lead samples and 51 completed intervals, and
this publication adds one of each. The release ships refusal of unreadable
joined/recovered file objects; full resource/HDD and elapsed gates remain open.
