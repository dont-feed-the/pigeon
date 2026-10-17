# Logging and conformance checkpoint

The [conformance inventory](../conformance.md) maps all 43 currently accepted
ADRs to implementation and verification entry points. It records open work,
not a blanket conformance verdict. Proposed records keep their own status.

## Diagnostics leave daemon work available

Review of accepted ADR 0022 found that the prior logger synchronously wrote
daily JSON from the event callback, had no seven-file retention, and did not
install the specified human stderr/EnvFilter layer. The follow-up uses
tracing-appender's bounded lossy queues for file and stderr workers. The
daemon owns both worker guards until shutdown. Its diagnostic callbacks
perform no file or stderr I/O. The durable trace proof channel is unchanged.

JSON keeps the existing top-level fields and `pigeond-YYYY-MM-DD.ndjson`
names used by debug bundles. A JSON event over 16 KiB becomes valid metadata
stating its original byte count. Each queue holds at most 128 entries; excess
diagnostics can be dropped instead of waiting for the destination. This is
diagnostic behavior, not permission to drop trace evidence or safety state.

The file worker retains its active file and the six newest other matching
daily files, including across a backwards wall-clock correction. It ignores
unrelated names, invalid calendar dates, directories and symlinks. Existing
file names remain compatible; new files are private to their owner.

## Validation

Five native contract checks passed: the sink is deliberately held blocked
while additional writes and queue overflow complete; worker shutdown drains
queued JSON; target filters and typed top-level fields remain intact;
oversized JSON remains bounded and valid; and daily files preserve names,
private permissions and the retention selection. File-date fixtures test
selection only; no real days of retention or resource budget are claimed.

Extending the retention fixture with invalid date-shaped names exposed an
initial matcher failure. The shared RFC3339 parser normalizes some invalid
calendar dates, so the file matcher now requires a round trip to the exact
original UTC date. The original failed run is retained separately from the
passing five-check retake. This patch does not change restore-time parsing.

All-target Clippy including lab, trace and tokio-console features passed at
the initial checkpoint. The new dependency set passed advisories, bans,
licenses and sources without new exceptions. Final current-main native
integration checks and exact-head CI precede merging and publication.

The current-version parallel workspace run passed 752 of 753 checks. Its
single failure was a restart fixture assuming it could always create a new
metadata ref lock after stopping its fetch. The retained failed directory
contained an empty lock and no remaining fixture fetch. The fixture now
retains an existing obstruction without modifying it and asserts both inode
and bytes across rejoining; it creates a fresh fixture lock only if absent.
Both restart scenarios then passed together through their four durable crash
points. The original 753-check run remains recorded as failed, separately
from the two-check retake; no timeout or production lock policy was relaxed.

Final current-version native S2 passed all 11 checks in 40 seconds and S3
passed all 14 in 18 seconds. TLC accepted both filesystem and roster streams
for each scenario. Final all-target Clippy including tokio-console, formatting
and structural checks passed again after the fixture change. Exact-head PR
CI remains required before merge.

## Remaining gates

Complete secret-type Debug redaction and instrumentation coverage still
need the joining/signing security and conformance review. In particular,
logging's dedicated checks are not substituted by telemetry exporter tests.
The inventory also records the open Authority refinement, upgrade matrix,
M6 scheduling and measured budgets, and real elapsed M7 gates.
