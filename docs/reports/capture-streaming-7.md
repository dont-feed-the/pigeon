# Capture progress and independent streamed verification

Capture ingestion and final verification each used an unmetered native
command with a fixed two-minute active limit. A long hash could therefore
fail, or leave an already captured file without a verified unchanged-file
observation. This checkpoint addresses these two paths; it does not complete
M6 or measure the reference/HDD budgets.

## Controlled failures and correction

A real capture fixture intercepts only the subsequent `calckey` helper and
holds it beyond the unchanged 120-second deadline. The real pinned annex
still ingests the file and commits it. The original run fails in 120.343
seconds because the exact staged entry has no verified observation. Its
fixture is retained at `/workspace/pigeon-fs-fixtures/.tmpPzB9gL`; the original
log is `/tmp/pigeon-capture-read-original.log`. Two preceding fixture build
attempts supplied a String where GitEnv needs OsString; neither supplies a
native verdict. An unrelated write-line edit did not fix that first error;
the second build retained the same error before correcting the OS string.

Final verification now streams bounded 64 KiB reads with native page-cache
advice. It shares the repository's Capture control and preserves the actual
kernel timestamp fence, regular-file and executable-mode checks, exact
opened-descriptor and final-path observations, and exact staged Entry.
The existing `export::matches` law supplies SHA256/SHA256E digest/size
validation. A filename extension does not change content identity. The
cache format and its authority remain unchanged: it proves neither custody
nor permission to overwrite, transfer or erase.

Only advancing read bytes renew the existing two-minute active inactivity
budget; intentional resource waits spend none. Cancellation releases the
Capture registration and signals the read worker. A late blocking read may
finish in the kernel, but it cannot publish an observation; the worker exits
at its next cancellation check. This is not a claim that kernel I/O itself
is interruptible. Five controlled read fixtures pass for moving bytes,
pauses, cancellation, genuine stalls and opened-inode mismatch. The original
capture fixture passes in 0.432 seconds after correction.

A separate controlled native NUL-batch Add fixture originally passes three
negative controls but times out the moving two-file capture at 800 ms.
After correction all four pass, with moving capture completing in 3.235
seconds. Progress must name `add` and a planned file in that bounded batch;
only a larger count for that file adds activity. Another file or command,
repeated counts, invalid declared sizes and final answers cannot renew it.
Final JSON passes unchanged to the existing exact answer-count/name/key/
success checks. Keyed GET/COPY keep their operation-specific counters.

## Native contracts and initial checks

Pinned git-annex 10.20261006-pigeon1 `Command/Add.hs` declares
`jsonProgressOption` and meters ingestion at line 264. A real 32 MiB raw
Add emits three records, from 32,752 to 33,554,432 bytes, whose action names
`add` and the planned file; the key appears in the final answer. By contrast,
`Command/CalcKey.hs` uses `nullMeterUpdate`, so its lifetime or output is not
used as evidence of hash progress. Source/schema evidence is retained in
`/tmp/pigeon-capture-add-native-format.json`.

All 50 Git-driver checks pass, including two new 1000-case laws: the
saturating sum of per-file maxima is order-independent, and arbitrary read
boundaries preserve all final answers. A real 32 MiB managed Add contract
checks canonical native keys, exact final answers and bytes, including a
literal newline in the second filename. The five existing export laws and
all 30 platform checks pass. Whole-workspace all-target Clippy and structural
checks pass at this initial checkpoint. The broader workspace and current-
main observations follow after running.

## Keeping future bridge failures inspectable

PR #84's original custodian bridge CI discarded its temporary fixture after
an offline save missed convergence amid metadata-fetch timeouts. That miss,
matching-binary local retake and bounded CI retake are recorded in the
transfer-progress release report. Future custodian jobs retain their fixture
and traces on failure, separated by run and attempt. The short `/tmp/pgc-…`
root preserves Unix socket path headroom; a long runner-temp directory could
break those sockets. No wait or assertion changes, and successful jobs do
not upload these disposable test fixtures.

## Background Mac allocation

The older Mac run 39077215598 at source
`323a2c8a82401f05291ad48ad282418b48981ee1` passed its CI and lab stages:
735 workspace tests and all 53 native daemon cases, including the corrected
backup-refounding prerequisite. Its two fixed and two random slice jobs
then overlapped v0.6.16's archive build while the Mac pinned-dependency job
was queued. This superseded, non-main run was cancelled with its completed
evidence retained; it is not a passing complete background workflow. The
release dependency job subsequently started at 23:57:36 UTC.

Future background slices keep two fixed jobs and run one random shard at
a time, for a three-job peak. Linux concurrency, every scenario, both random
shards, per-job workers and publication gates are unchanged. The existing
automation dispatches current main after v0.6.16 is independently verified
and the older background run finishes stopping. Runtime of the new complete
background workflow and sustained publication gains remain to be measured.

## Complete workspace and final open-race hardening

The complete parallel workspace passes **826/826 tests in 546.845 seconds**
(eight slow, one skipped), before the final open-race guard below. S2 passes
11 checks in 39 seconds and S3 passes 14 in 29 seconds; both require and pass
CaptureInstall and Roster TLC coverage. All 25 CI-selection and two partition
checks pass, followed by all-target Clippy and structural checks.

Review then identified an opening race: a regular file could become a pipe
before the blocking open. The exact-inode reader now opens nonblocking with
no-follow, checks the opened stat, and refuses replacements before reading.
Native pipe and final-symlink fixtures cover those cases; the pipe fixture
releases a regressed blocking reader before failing so it cannot hang the
test process. The ordinary streaming reader keeps its existing cache advice.

After this guard, **32 platform, 146 daemon-library and 54 install tests pass**.
S2/S3 again pass with required filesystem/roster TLC. Whole-workspace all-target
Clippy and structural checks pass, and Apple Silicon platform all-target
Clippy passes separately. That Apple result is compile-time evidence, not a
new complete native Mac run or the unavailable broader daemon cross-check.
Logs: `/tmp/pigeon-capture-workspace.log` and
`/tmp/pigeon-capture-open-hardening.log`. No stack, assertion, production
reserve or existing stalled-operation budget was relaxed.
