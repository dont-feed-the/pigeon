# M7: persistent native soak runner

> Mission: accumulate actual elapsed evidence with explicit interruption rules.

`scripts/lab-soak` builds a clean source into a new frozen binary directory,
recording source revision, lab features and all three executable checksums.
The runner requires a new fixture root and keeps the same laptop and backup
across atomic saves and deliberate backup crashes/restarts. Every cycle checks
the accepted pointer trees and independently hashes every current and retained
annex object on both peers. PID/start-time checks detect unexpected daemon
replacement. A boot check and at most thirty seconds between heartbeat
observations prevent a suspended harness from silently earning elapsed credit.

The exercise clock starts after initial capture and backup verification.
Normal runs require 72 actual hours, multiple cycles and backup fault coverage.
A signal, missing peer, changed boot, byte-proof deadline or cleanup failure
fails the observation. There is no resume credit. Checkpoints and fsynced cycle
records remain in the private fixture directory on every exit. No invitation,
device key or signed URL is written to the public report.

The short smoke mode exercises at least two cycles and one backup crash. It
always reports `not-measured` for the elapsed gate, regardless of its clock.
It is not a passing soak. CPU/RSS samples are diagnostic; no M6 resource budget
is inferred. This Linux runner uses `/proc` identity checks and laboratory
backup-role support; macOS periodic evidence remains separate.

Build, smoke-check, then start on a host that will remain available:

```sh
scripts/lab-soak prepare --bin /absolute/new/soak-bin
scripts/lab-soak run --smoke --bin /absolute/new/soak-bin \
  --root /absolute/new/smoke --report /absolute/smoke.json
scripts/lab-soak run --bin /absolute/new/soak-bin \
  --root /absolute/new/soak --report /absolute/soak.json
```

An initial smoke on the preceding watcher source passes independent byte
checks but finishes after one long cycle without exercising the restart. The
smoke completion condition is corrected to require at least two cycles, and
the rerun covers the backup restart. Neither observation qualifies 72 hours.
The actual long run and final verdict must be recorded separately.

The corrected initial smoke completes two cycles and one backup restart in
56.856 seconds, retaining 24 independently hashed versions on both peers and
a maximum heartbeat gap of 1.120 seconds. Its elapsed verdict remains
`not-measured`. It uses lab source
`7fb385656da6e2c37d2f3136f028d84fea42d42d`; the subsequent held-decision
correction is validated separately before beginning a longer observation.

A corrected-source smoke on
`01da565d1a719c07b2817a96ae031ac29700793a` passes two cycles and one backup
restart in 56.843 seconds. The actual 72-hour observation on those frozen lab
binaries starts its exercise at 08:49:13.805554 UTC on 18 October, after setup
and independent initial byte proofs. Its earliest completion is 21 October
at that time. The observation instead fails at 09:36:54 UTC; it supplies no
72-hour credit. Later source changes do not rewrite its provenance.

## First actual observation: hot-path deadline mismatch

The initial run completes seven cycles and one deliberate backup restart,
then fails its eighth cycle after 2,860.205 seconds of exercise. The harness
allowed only 300 seconds for each accepted-tree/object proof. It edits the
same four paths every five minutes, so those paths exceed six captures per
hour. [15 §15.5](../spec/15-resource-use.md#155-large-and-hot-files) then
intentionally permits one capture per ten minutes. The source logs show
its seventh and eighth captures at 09:26:24 and 09:36:24 UTC, exactly that
interval. The backup catalogue is one capture behind when the deadline
expires; independent post-run hashing verifies all 48 generated objects on
both peers, and the source catalogue matches the eighth save. This is a
harness deadline mismatch, with no lost content observed. It does not prove
that the backup would have accepted the catalogue after the stopped run.

The private original fixture, report and logs are retained together in an
archive of 378,434 bytes, SHA-256
`16d42d3d9ea46e3dcdbd7cd5e366bbbb1f512f3e2095658bf420bcdec852d649`.
No unchanged-source retry is used and none of this elapsed time is resumed.

Cycle proofs now have a bounded fifteen minutes: the specified ten-minute
hot-path interval plus five minutes for settling, history propagation and
independent proofs. Initial setup keeps its five-minute deadline. Reports
record the cycle deadline and the in-flight cycle before each save. The
72-hour requirement, heartbeat fences, actual byte checks and restart
coverage are unchanged. A deterministic observer test reproduces the old
300-second rejection and admits a catalogue arriving after the valid hot
wait. A new actual observation and its final verdict remain required.
