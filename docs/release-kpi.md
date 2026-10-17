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


## v0.5.16 observation

[v0.5.16](https://github.com/dont-feed-the/pigeon/releases/tag/v0.5.16)
was published on 17 October at 01:48:22 UTC. All six assets, archive
checksums, small downloads and the public latest-release redirect were
verified. Candidate lead time was **20 minutes 45 seconds**; the completed
publication interval was **1 hour 21 minutes 16 seconds**. Both targets
were met. Nine lead-time samples and eight completed intervals still do
not establish a reliable percentile; earlier misses remain recorded.

The first tag CI attempt hit an `index.lock` race in the old-roster leave
fixture beyond its existing bounded retry. Only that failed shard and its
dependent selection check were rerun. The publication job was retried
afterward with the successful archives retained. Final Linux CI, lab,
slice and release checks all passed. The next rollup removes the fixture's
unnecessary working-index mutation rather than extending the retry or
removing a lock. Full-model background checks remain separate evidence.


## Additional runner capacity: 17 October

The user reports a GitHub Teams limit of 60 concurrent runners. In the
successful v0.5.16 slice run (38990943017), the serial non-S5 scenarios ran
from 01:34:08 to 01:46:07 UTC: 11 minutes 59 seconds. The same partition
blocked v0.6.0 while its other required Linux groups had finished; its
failed S12/S13 diagnostic checks are recorded in the M5 report.

The slice now divides the live lab catalogue into five disjoint partitions:
core, crash fixtures, recovery, people and S5. Each partition still runs
its scenario laws, model, isolation check, real executor and TLC traces.
The selector checks complete coverage of the live manifest, rejects malformed
or duplicate IDs, and includes new scenarios automatically. Linux can run all
five independently; periodic macOS validation retains its serial limit.
This is a targeted change to a measured serial step, not a measured speedup
claim. The next actual publication will supply the before/after observation.
The failed v0.6.0 candidate and the interruption remain in the KPI history.

## v0.6.1 observation

[v0.6.1](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.1)
published at 09:07:42 UTC on 17 October with all six assets. Archive
checksums, asset digests, the native Linux installer, completions, pinned
annex and public latest redirect were verified. Candidate lead time was
**15 minutes 49 seconds**, meeting the one-hour target. The interval from
v0.5.16 was **7 hours 19 minutes 20 seconds**, missing the two-hour target;
the failed v0.6.0 candidate and interruption remain visible. The two-day
collection has ten lead-time samples and nine completed intervals, still
insufficient for a reliable percentile.

All five Linux slice partitions and both random crash shards passed.
Their total job durations, including setup/build/trace checking, were
10m24s (core), 9m06s (crashes), 9m38s (recovery), 8m18s (people) and 6m55s
(S5). Random shard 1 was the final release gate at 09:07:27; publication
followed 15 seconds later. These measurements show the new partitions
completed independently; differing histories and runner conditions prevent
attributing the whole candidate improvement to partitioning.

Completed workflow elapsed time now stops at completion. Its separate
candidate age and the actual publication gap continue to grow. Previously
the readable report mislabeled a completed candidate's growing age as its
elapsed execution, even though its publication lead-time measure was right.


## Bridge integration baseline: 17 October

The successful PR #69 bridge job
[117352256007](https://github.com/dont-feed-the/pigeon/actions/runs/39017382261/job/117352256007)
ran from 08:27:07 to 08:51:06 UTC: **23 minutes 59 seconds**. Its serial
phases included 2m15s restoring the Rust cache, 35s building the native
programs, 3m12s checking laptop storage, 33s checking delayed decisions,
3m48s checking custodian storage, 2m37s checking native traces and 10m23s
exercising the portal and real Nextcloud client. Full CI and lab were
already green while this final PR check continued.

The next rollup builds and tests the matching programs once, then gives
laptop, custodian and real-client execution independent runners. Programs
travel as a compressed, checksummed archive with the immutable workflow
source recorded and verified before extraction. Executable permissions
survive the archive; execution jobs do not restore the large Rust cache or
rebuild the programs. Every selected build, execution and trace check still
must succeed. The real laptop-storage check passed locally using an
extracted bundle. This is preparation for a measured improvement, not a
reported reduction in CI or publication time.

The first actual parallel workflow,
[39029050268](https://github.com/dont-feed-the/pigeon/actions/runs/39029050268),
passed all selected checks on `879fad0`. It ran from 11:31:54 to 11:43:25
UTC: **11 minutes 31 seconds**, including selection. The shared build took
4m33s; the laptop, custodian and real-client jobs then took 2m04s, 4m37s and
6m40s respectively, starting together. This is an observed reduction from
the prior 23m59s serial bridge job, with different cache/build conditions
and content-controller changes; it is not a controlled attribution or an
actual publication. Other PR CI exposed two failures, recorded in the M6
report, so this head was not merged or tagged.

## PR #70 follow-up observations

The subsequent `47974c5` head passed lab and the whole bridge workflow, but
again failed the 102-file protection deadline. Its retained fixture holds
102 verified laptop objects and 80 backup objects; transfer-lock conflicts
and per-key process launches are recorded in the M6 report. Placement now
reuses controlled batches, and local-content observations use bounded batches.
The rolled-up native run passes 65 checks, with that lifecycle taking 30.5 s.
CI and an actual publication are still required; this is no published result.

The preceding bridge build job 117396374972 restored an 8,463,386,667-byte
Rust cache from 12:30:01 to 12:34:30 UTC on 17 October: about 4m29s before
building. Download completed at 12:30:58; unpacking accounts for most of the
remaining time. Reducing cached build contents is the next measured CI target,
with first-build and warm-cache times to compare before claiming an improvement.


## v0.6.2 observation

[v0.6.2](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.2)
was published on 17 October at 13:42:34 UTC from
`3439d9d67fc8b1a6368f4eb15e2633a28a2ee6dd`. All six assets, four archive
checksums, the native Linux install, pinned annex, shell completions and
public latest-release redirect were verified. Candidate lead time was
**26 minutes 29 seconds**, meeting the one-hour target. The publication gap
was **4 hours 34 minutes 52 seconds**, missing the two-hour target. Eleven
lead-time samples and ten completed intervals still do not support a
reliable percentile claim.

PR #70's final head passed CI, lab and all three parallel bridge cases.
The tag's first CI attempt hit a competing peer metadata-ref lock during
backup rejoin. Its first core-slice attempt passed all S3 byte/status checks
and TLC, but the scenario expected a new restore install when a normal
round had already installed the retained request. S2 passed all eleven
checks in 40 seconds and its trace passed TLC. Each failed partition was
retried once, retaining all other results. Publication alone was then
retried with both successful platform archives retained. Exact-source
Linux CI, lab, slice and publication all finished successfully.

The follow-up removes the redundant remote-ref write during welcome import:
annex already merges the imported private metadata ref. A regression holds
the competing lock across all four recovery crash points without removing
or changing it. Restore traces now observe the actual requested bytes at
completion, including an already completed request, instead of requiring
an extra install. A native S3 run passes fourteen checks and fifty
filesystem observations; recorded no-op snapshots are accepted by TLC,
while different requested or visible bytes must produce counterexamples.

The measured 8.46 GB Rust-cache restore above motivates retaining compiled
dependencies while excluding old test executables in the Rust workspace
and shared bridge build. A new cache namespace avoids restoring the old
archive. Only main writes caches; its first successful warm-up must be
retained. Cold and warm runner measurements are still required before
claiming a speedup. None of these follow-up changes is part of v0.6.2.


## First dependency-cache observations after PR #71

PR #71's bridge build, job 117411641261, had a cold namespace and passed
in 2m39s (13:47:29–13:50:08 UTC). Main's matching build then saved a
459,746,369-byte dependency cache in job 117414069081 at 14:02:49–14:02:55.
The v0.6.3 tag's Rust-workspace job 117414959357 restored it successfully
at 14:04:40–14:04:47: **7 seconds** for that step. The earlier observed
bridge restore was 8,463,386,667 bytes and about 4m29s. These observations
use different build/feature contexts and runner conditions; the cache
excludes old test executables in both producers. Actual publication lead
time still depends on every required gate and both platform archives.


## v0.6.3 observation

[v0.6.3](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.3)
was published on 17 October at 14:19:57 UTC from
`3f5e5efd554dc9421d488e5d8e453552a79492cc`. All six assets, archive
checksums, native Linux installation, pinned annex, completions and the
public latest-release redirect were verified. Candidate lead time was
**16 minutes**, and the publication gap was **37 minutes 23 seconds**.
Both targets were met. Twelve lead-time samples and eleven publication
intervals still do not support a reliable percentile claim; previous
misses remain in the history.

CI, lab, slice and publication passed on their first attempts. The recovery
lock and completed-restore trace failures from v0.6.2 did not recur. Lead
time fell from 26m29s to 16m, with different event histories, cache conditions
and no retry; this does not isolate the contribution of the cache change.
The first warm restore's seven seconds is the separate step observation.

The core slice had no cache to restore and spent 4m57s building its matching
release programs (14:05:53–14:10:50 UTC, job 117414939258). Earlier main
slice runs had been canceled before saving that cache. One core job from
validated main merge eaef33a was restarted specifically to prime it; current
main differs from that source only in its release version, lock and changelog.
The canceled current-version run had created no jobs to rerun. This primer
is separate from publication. Job 117417325666 saved a 488,810,880-byte
slice cache at 14:22:53–14:23:00 UTC; the cache API confirms it is present
on main. Its native scenario/trace check continues, and the future warm-slice
comparison is still pending.


## v0.6.4 observation

[v0.6.4](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.4)
published on 17 October at 16:58:43 UTC from
`fa00dcc013e98f98a6a0dcdd36949fe109e13d5b`. Its six assets, four archive
checksums/API digests, Linux install, pinned annex version, three shell
completions and public latest redirect were verified. A transient HTTP 503
interrupted the first download verification; the next completed successfully.

Candidate lead time was **24 minutes 29 seconds**, meeting the one-hour
target. The completed gap from v0.6.3 was **2 hours 38 minutes 46 seconds**,
missing the two-hour target. The collector retains 13 lead-time and 12
publication-interval samples; no reliable percentile is inferred. Publication
resets the current clock, without hiding this missed interval.

PR #72's preparation found a status request bypassing file settling, a real
clock-fence race, and historical protection stranded by equivalent sibling
merges. Review also caught an erasure acknowledgement that needed to stay
conservative. These were fixed before merging, with native regressions; no
assertion timeout or release gate was relaxed. The final source passed all
730 local checks, all 13 driver/model scenarios and negative checks, S2/S3
with TLC, and exact-head PR CI. Main cache producers completed before tagging.

Tag CI, lab, both platform builds and native install checks passed first time.
S9 shard 0 passed its 25 native schedules but rejected one unstable filesystem
observation at clock 49. One targeted retry repeated seed 156199965296 and
the identical schedules; every required trace then passed. Only the failed
publish job was rerun, retaining the successful archives. The
[original failed trace](reports/evidence/v064-s9-observation-failure.ndjson)
is retained, together with its artifact digest in the M6 report. This miss
and retry remain part of the candidate's observed lead time.

The next follow-up preserves the three-attempt observation bound and full
stat/hash checks, but yields between unstable observations. The failing
schedule is a named native regression. Its actual publication will supply
the next observation; no measured reduction in failure rate is claimed now.

## v0.6.5 observation

[v0.6.5](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.5)
published at 2026-10-17T17:33:03Z from
`987941a93ea0ba47ca74b88fe68fa4e87bc9c94f`. All six assets, four archive
checksums, Linux installation, pinned annex, shell completions and latest
redirect were independently verified. Candidate lead time was **666 seconds**
(target 3600); the completed publication gap was **2060 seconds**
(target 7200). The collector retains
14 lead-time and 13 completed-gap samples.
No percentile or measured reduction in S9 failure rate is inferred.

PR #73 passed all 737 parallel local checks and exact-head CI before merge.
Its restore-lock and bounded trace-retry changes ship in this publication.
The original S9 failed observation remains retained. Duplicate queued main
workflows were canceled in favor of the tag gates; active main cache producers
were preserved through their completed save steps.

## v0.6.6 observation

[v0.6.6](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.6)
published at 2026-10-17T18:06:25Z from
`152d2a7d6da3b483f4a3e73f3ff14be6b344dde3`. All six assets, four archive
checksums, Linux installation, pinned annex, shell completions and latest
redirect were independently verified. Candidate lead was **764 seconds**
and the completed publication gap was **2002 seconds**, meeting
the one-hour and two-hour targets. There are 15
lead-time and 14 completed-gap samples;
no percentile or sustained failure-rate improvement is inferred.

Filesystem admission ships with native ext4/btrfs/xfs evidence and accepted
English/French usability retakes. The original five local fixture failures,
their focused retake and the final exact-head CI remain separate records.
All Linux release gates and both platform builds passed. Duplicate queued
main runs were canceled; active main cache producers were preserved.

## v0.6.7 observation

[v0.6.7](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.7)
published at 2026-10-17T18:57:37Z from
`eb3065b2730c4a7d5b0bac77a78220f349f8478c`. All six assets, four archive
checksums, Linux install, pinned annex, completions and latest redirect
were independently verified. Candidate lead was **1711 seconds**
(met the one-hour target); publication gap was
**3072 seconds** (met the two-hour target).
There are 16 lead and
15 completed-gap samples; no
percentile or sustained failure-rate improvement is inferred.

The original CI shard 3 had 10 passes and two timeouts: held-file protection
reported disk busy; a capacity fixture did not finish its initial install.
The bounded retake passed all 12 daemon checks and its storage-boundary
trace checks. Original artifact 12110980651 is 15121562 bytes, SHA-256
`d49c99a41b5ff8fef4012064564fee1d0ee8740065d933ae0d5a57998637b505`.
Its held fixture retained 102 laptop objects and 21 backup objects, all
matching their sizes and SHA-256 keys; incomplete transfer was not silently
counted as protection. One bounded failed-shard retry and publication-only
retry kept the original tag and source. No deadline, safety verdict or gate
was waived. Both archives and the other Linux gates were retained.

## v0.6.8 observation

[v0.6.8](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.8)
published at 2026-10-17T19:33:28Z from `67db09aaa00eeef134d053b7b762fb5bf50071d1`. Independent
verification passed all six assets, four archive checksums, Linux installation,
the pinned annex, completions and latest redirect. Candidate lead was
**1080 seconds** and publication gap **2151 seconds**, meeting the one-hour
and two-hour targets. The collector retains 17 lead
and 16 completed-gap samples;
no percentile or sustained failure-rate improvement is inferred.

The original CI shard 3 passed 10 tests and timed out in held-file protection
and replacement installation. The held fixture's disk-wait headline and 102
valid laptop objects versus 36 backup objects show incomplete transfer, not
protection. The original artifact 12112021417 is 15261155 bytes, SHA-256
`73aae0e2ff0f8a6ff293a854cefe0ca118fef63d3397f40d49ace97c3960c73d`.
All present annex objects matched their sizes and SHA-256 keys. The original
logs do not contain pressure readings, so they do not establish the cause.

One bounded failed-shard retry passed all 12 daemon checks and the positive
and negative storage-trace checks. A publication-only retry reused both green
archives; the tag and source stayed immutable. No deadline, safety verdict or
required gate was waived. This repeats the same two fixture families that
blocked v0.6.7's first run. Pressure diagnostics in the next M6 follow-up target
that evidence gap; a reduction in failures remains unmeasured.

## v0.6.9 observation

[v0.6.9](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.9)
published at 2026-10-17T20:18:40Z from `cca9f52c289528fb83f256e4a0b2e6314d7b5482`. Six assets,
four archive checksums, native Linux installation, pinned annex, completions
and latest redirect passed independent verification. Candidate lead was
**1388 seconds**, publication gap **2712 seconds**; both targets were met.
There are 18 lead and
17 completed-gap samples. No
percentile or sustained failure-rate reduction is inferred.

All daemon CI shards passed their first attempt. The first core slice passed
S1/S2/S3/S4/S6/S7 but failed S8's absence assertion after a disk-full refusal.
Artifact 12113236151 is 79430 bytes, SHA-256
`70b7cb6fb96d511addba00899ddb73eec286a08ce7db0ab0e0e9e00032de4418`.
The retained stream shows a thinning move-aside and rename-in rather than
completed freeing. One bounded core-slice retry passed all seven scenarios,
including 15 S8 checks, and all filesystem/roster traces. A publication-only
retry reused both archives. No tag, deadline, model verdict or gate changed.
The next follow-up strengthens absence observations using journal boundaries;
its effect on recurrence remains unmeasured.

The actual v0.6.4 macOS background run 39051800511 completed successfully;
one v0.6.9 background run, 39066233321, is queued for the combined native
priority/repair changes. Neither is a measured resource budget or a seven-night
streak. The remaining four old v0.6.8 background S9 shards were canceled in
favor of v0.6.9; nine completed jobs were retained, not represented as a
completed full v0.6.8 model run. Active main cache producers saved before
obsolete main workflows were canceled.

## v0.6.10 observation

[v0.6.10](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.10)
published at 2026-10-17T21:10:31Z from
`510146fce732cbcd1dd3a35e4bb5f8ed3ea52319`. Independent verification
passed all six assets, four archive checksums, native Linux installation,
pinned annex, completions and latest redirect. Candidate lead was
**1200 seconds** and publication gap **3111 seconds**; both targets were met.
There are 19 lead and 18 completed-gap samples. No percentile or sustained
failure-rate improvement is inferred.

Original daemon shard 3 passed 11 of 12 tests; the initial capacity
assertion saw zero instead of 13 bytes. Artifact 12113894459 is 9641122
bytes, SHA-256
`b3a2359940016fd47319d71b7560102a87adef8d1c931e728203caedd0fe6d20`.
The one bounded original-shard retake passed all 12 checks in 203.441
seconds and the positive/negative storage-boundary trace checks. The
publish job had stopped at that failed CI gate after its installation
and real backup-service checks passed. One publication-job-only retry
reused both green archives. Source, tag, assertions, deadlines and gates
stayed unchanged. The next fixture checkpoint requires its actual named
paths before capacity observation; it does not infer the zero snapshot's
precise interleaving.

Main's stricter S8 observer passed all 15 checks in Linux namespaces, with
accepted filesystem and roster traces. The tag's complete slice also passed.
This is finite validation of the observation change, not a failure-rate
measurement or a real resource budget. The v0.6.9 Mac workspace separately
passed 690 of 692 checks: two telemetry fixtures correctly encountered
normal-build Mac backup refusal. They are corrected into the lab tier in
the next checkpoint; that run is not a passing Mac suite.
