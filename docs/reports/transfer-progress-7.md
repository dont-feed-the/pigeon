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
