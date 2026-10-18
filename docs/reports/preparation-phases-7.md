# Reference preparation phase evidence

> Mission: locate large-tree preparation costs before another long resource experiment.

## Problem and observations

The [full Fast diagnostic](reference-diagnostic-7.md#full-fast-preparation-observation)
expires its 5400-second preparation bound. Both catalogues eventually contain
100,000 paths, but the backup still has no complete object set. The final
RSS high-water readings exceed the reference limit; no window or budget
passes. A single final process snapshot cannot identify the earlier delay.
The hosted profile already builds production programs with `--release`.

A controlled private metadata fixture has 8192 synthetic, unverified
location claims. The existing 64-key query scheme starts 128 native
processes and takes 9.938 and 8.246 seconds in two local observations.
One persistent process with 64-key exchanges takes 5.070 and 5.026 seconds.
Every returned key and UUID matches the fixture. This is a narrow metadata
comparison, with no content or custody proof; it does not explain the
much longer full reference failure. Production query behavior stays the
same, including its 64-key bounds and reply checks.

## Phase counters and preparation timeline

A dedicated `pigeond::work_phases` debug target records ingestion progress,
independent byte/stat proof attempts and successes, protection-history read
time, location-claim queries and local-presence queries. The reference
harness explicitly enables this target. The fields are counts and elapsed
microseconds, without paths, keys, native arguments or credentials.
Ingestion reports each existing bounded batch; proof and query progress
report at most once per five seconds plus their final batch. Query counts
remain claims or presence observations, never positive custody proof.

The diagnostic reader keeps each phase's latest event alongside recent
scan/pressure events, within the existing overall 50-row envelope. A noisy
stream cannot evict every phase marker. Two-peer resource samples already
collected during preparation now also populate a timeline: at most one
snapshot every 30 seconds, with at most 256 rows. Snapshots are isolated
from later mutations. No additional resource polling or measurement-window
calls are introduced. The dataset, service settings, independent byte
proofs, ten-minute window and resource limits remain unchanged; diagnostic
Fast preparation still cannot certify an M6 pass.

Initial reduced runs complete but expose missing phase logging: the
profile enabled only candidate-scan debug events. The dedicated target
and its filter are corrected before the final evidence run. The fresh
32-file, 131072-byte Fast-to-Gentle smoke independently verifies both peers
and every object, records all twelve phase kinds, 32 successful byte/stat
observations, and the bounded preparation timeline. Both verdict fields
remain `not-measured`. Its changed worktree and lab programs are explicit;
it is a control-flow check, not a full reference measurement.

## Development password hashing

Development builds also compile Argon2 and Blake2 with optimization, as
already done for the signature and SHA-256 dependencies. Application code
keeps its development settings. Three hash-only CLI observations on clean
source `e5c7f7601647731c59dc374aa1610db95f535544` take 0.387–0.401
seconds with ordinary development code generation and 0.0246–0.0251
seconds with these dependency overrides. Both use Argon2id version 19,
`m=19456,t=2,p=1`. No password setting, salt policy or verifier changes.
These hash-only timings do not measure end-to-end sign-in or explain the
real Nextcloud gate: app requests use the existing token hash, not Argon2.

## Validation and next measurement

All 141 script contracts pass, including latest-phase retention, numeric
sanitization, the 50-row envelope, preparation cadence, backwards input,
snapshot isolation and the 256-row bound. All 26 bridge library cases and
28 selected daemon/installation capture, watch and location checks pass. Matching rebuilt
programs supply the final smoke evidence; all-target workspace Clippy has
zero warnings. Repository checks precede submission.

The next full-size diagnostic uses a clean integrated source and a bounded
600-second preparation observation. It retains the new phase/timeline
artifacts before any further experiment. It remains diagnostic and cannot
qualify M6, regardless of a passing numeric result. Physical-HDD and actual
elapsed qualification gates remain separate.
