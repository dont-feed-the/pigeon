# Bounded capture pointer proofs

> Mission: reduce capture subprocess overhead while retaining exact native proofs.

## Observation and change

On source `c298375d0dac14017dfdc2222f83539cf51137d6`, a 129-file
fixture starts 129 Git processes while verifying already staged annex
pointers. The outer loop already groups 64 paths, but each pointer is read
with a separate `cat-file` process. Capture now resolves those immutable
pointer objects with one existing batch read per group of at most 64.

Every path keeps its own original stat and index entry. Missing files keep
their reply slots; incomplete reply sets provide no verified observations.
Only blob replies containing a valid annex pointer proceed. Executable
mode, timestamp fencing, streamed file bytes, SHA-256, size and full stat
matching remain required before a cache entry is accepted. A concurrent
same-size save still fails its proof. This changes no authority policy or
core algebra.

## Controlled native observation

The same 129 unique files, including executable files, are checked before
and after the change with real Git and the required patched git-annex. An
isolated wrapper counts `cat-file` invocations; it does not replace object
contents. All 129 byte/stat proofs pass on the original source, then the
new process-count assertion fails: 129 instead of three.

| Source | Git processes | Check duration, two local observations |
| --- | --- | --- |
| Original | 129 | 352,589 and 360,586 microseconds |
| Batched | 3 | 25,523 and 22,265 microseconds |

These are narrow fixture observations, not a complete-workload speed claim
or an M6 resource-budget result. The deterministic process bound is the
regression assertion; elapsed timings are diagnostic only. Both intentional
original-source failures are retained as controlled RED evidence, not CI
retries.

Four native regressions cover the process bound, a missing first file,
empty/incomplete pointer replies and a same-size mutation during pointer
resolution. Together with the two existing timestamp-fence cases they pass
in 2.370 seconds. Broader validation passes 191 daemon library tests and
63 native install, held-change, watch, history and suggestion tests.
Matching rebuilt programs pass S2 (11 checks, 39 seconds) and S3
(14 checks, 18 seconds); TLC accepts both filesystem and roster traces.
All-target workspace Clippy has zero warnings.

## Background macOS evidence

The periodic v0.6.35 run 39120772097 fails three assertions: transfer
completion after capture preemption, the free/keep fixture's deleted-file
view, and the time-based restore fixture's deletion capture. Workspace
tests otherwise pass 834 cases; native shards 1 and 2 and packaging pass.
The two failed native fixtures retain their previous catalogue paths,
with pending deletions and valid historical objects; event-based creation
tests pass in those same jobs. These observations do not identify the
original mechanisms, and this batching change does not claim to fix them.
Original logs and both private fixture archives are preserved before any
retake. No unchanged-source retry is used. macOS remains periodic evidence
under the user's release policy.

The actual 72-hour soak and reference-resource diagnostics continue
separately with their frozen source revisions. No elapsed or resource gate
is inferred from these finite native checks.
