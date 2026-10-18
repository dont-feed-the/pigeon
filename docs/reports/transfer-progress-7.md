# Progress-aware transfer deadlines

The native deadline previously counted all active time, including sustained
byte movement. Large files could hit the 600-second transfer limit while
making progress; excluding deliberate power/disk/shaper waits did not solve
this. Controlled one-shot and batch fixtures both reproduce the premature
timeout before correction. Four negative controls already pass there.

Keyed JSON get/copy calls now request annex's JSON progress. The driver
accepts only increasing byte counts whose nested action matches this
operation's command and key, and ignores counts beyond a declared size.
Progress renews active inactivity time; repeated counts, another key or
command, malformed records and final success/failure do not. Each batch key
has its own counter. Capture, fsck and metadata keep finite active budgets;
intentional scheduling/stream waits and process-group cleanup stay intact.
No progress record grants authority, durability or a successful result.

The source contract is git-annex 10.20261006 Messages/JSON.hs:164 and
Messages/Progress.hs:130; Get.hs/Copy.hs opt into jsonProgressOption. A real
32 MiB pinned-annex copy emits increasing byte-progress with the same key
and command. The ordinary final JSON and copied bytes survive managed
one-shot COPY and batch GET. Records are stripped before existing result
parsers; the extra one-shot line buffer is bounded and oversized/incomplete
output passes through without progress credit.

Initial correction validation passes seven controlled native fixtures,
fifteen Git-library checks (including the first 1000-case read-boundary
law), seven existing process-group/deadline fixtures and nine actual
Git/annex contracts. The later maximum-law and broader native observations
are recorded after running. All-target Clippy and structural checks pass
at that initial checkpoint. The full M6 queue, reference/HDD budgets and
elapsed M7 gates remain separate.

## Broader validation and stack correction

A second hegel law runs 1000 cases and checks that matching byte counts form
an order-independent maximum. All 42 Git-driver checks pass, including both
laws and the moving, wrong-key, repeated-count, stopped-progress, batch and
ordinary-scrub native controls.

The first broader daemon-library run passed 136 of 141 checks; five aborted
with stack overflows. The new inline 8 KiB read buffer enlarged nested async
Git futures even on metadata paths. This was a real unreleased regression.
The buffer now lives on the heap and is allocated only for progress-enabled
reads. No test concurrency or stack limit was increased. The normal-stack
retake passes all 42 Git checks and all 141 daemon-library checks, plus 19
health/resource/codec/format fixtures and the real annex transfer fixture
(65.952 seconds). S2 and S3 pass with required CaptureInstall and Roster
traces accepted by TLC. All-target Clippy and structural checks pass.

Retained local evidence includes the original moving-transfer timeouts in
`/tmp/pigeon-transfer-progress-before.log`, the five stack failures in
`/tmp/pigeon-transfer-progress-stack-regression.log`, and the corrected broad
run in `/tmp/pigeon-transfer-progress-validation.log`. These local paths are
continuation evidence, not checked-in CI artifacts. An additional complete
workspace attempt stopped before testing when its disk-headroom preflight
found 4,693,659,648 free bytes. Only stale generated test executables were
removed; the production five-billion-byte backup reserve is unchanged.
The complete parallel workspace is repeated below before committing.

## Complete parallel workspace

The complete normal-stack workspace run after the heap correction passes:

```text
Summary [ 572.300s] 812 tests run: 812 passed (9 slow), 1 skipped
```

All-target Clippy, formatting and repository structural checks pass after
that run. The original stack failures and preflight-only capacity stop
remain retained; this result does not establish the reference/HDD budgets.

## Maintenance rollup

The macOS maintenance-priority correction is rolled into this PR before CI.
The complete workspace result above precedes these maintenance-only files;
all affected checks, the native scrub and all-target Clippy run below after
current-main integration. See [native maintenance](macos-maintenance-7.md).

## Current-main validation

Current v0.6.15 main `c51af44c4e80fad8fd1a89e8ea8185b775028428` is integrated before PR validation.
Relative to the validated feature base, incoming changes are only workspace
versions, lockfile, release changelog and the preceding report footer. The
new progress entry alone remains Unreleased. The complete parallel
workspace evidence above precedes this version/report integration; all
affected selections and required native traces are repeated below.

All platform-layer and Git-driver tests, native corrupt-object scrub, daemon-library checks, health/throttled-transfer/codec/
format fixtures, real annex transfer and S2/S3 pass on current main. TLC
requires and accepts CaptureInstall and Roster coverage. All-target Clippy,
formatting and structural/ADR/spec checks pass before the integration commit.
Exact-head PR CI and publication remain separate.

## Release validation

PR #84, head `e250c57b919d4d2edc99bfd2c3ee707ba4674c8c`, passed exact-head CI
39079064463, lab 39079064432 and bridge
39079064434. Current main was integrated before validation.
The two parser laws, controlled moving/stalled/key-isolation tests, actual
pinned-annex format/bytes contracts and full-workspace observations remain
distinct from the original two moving-transfer timeouts and five stack
regressions retained above. Normal-stack retakes and required S2/S3
filesystem/roster traces precede this validation. Dependency and core
slice caches saved; active main producers remain protected from cancellation.
v0.6.15 was published and independently verified before this candidate.
M6 capture/native-preset/read-latency and measured hardware budgets, the
wider security audit and actual elapsed M7 gates remain open.

The original bridge custodian job 117532501244 in run 39079064434
failed when a regular peer offline save did not reach a client within the
existing wait, amid repeated 30-second metadata-fetch timeouts. Its original
logs are retained on GitHub; original CI discarded its temporary fixture.
Artifact 12118386718 (49,963,986 bytes, SHA-256
`ce011d5318e1c6a04d0606be14c1753a4474f2542d82804c19bfbe0da55ac4ef`)
contains the matching binaries; its source tree equals this PR head.
The retained local retake passes all four behavioral groups, and TLC accepts
one lifecycle stream and 40 DAV transactions. This does not reproduce or
explain the original interleaving. One bounded original-job CI retake
117534558589 passed at the same head; no wait, assertion or gate
was relaxed. Full final CI/lab/bridge checks are green before merging.
