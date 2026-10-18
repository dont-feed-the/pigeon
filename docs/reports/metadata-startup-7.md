# M6: bound native metadata startup work

> Mission: reduce helper starts while preserving complete key binding and fresh local observations.

## Original full-size diagnostic

[Run 39136137286](https://github.com/dont-feed-the/pigeon/actions/runs/39136137286),
source `e5f0b1bd57079daa03f0f7cc76664fa40af90c31`, uses production builds
and a 5400-second Fast preparation bound. It fails preparation before the
measured window. Its artifact 12141449856 is 13097 bytes, SHA-256
`4c3ded207a77b00322eb5a7094ead1e6075a729f45b9ad1e859398e11693a031`.
The complete finished-job log and original report are retained before changes.

The source ingests all 100000 files in 2792.052 seconds; its subsequent
capture-proof event verifies 100000 entries in 91.488 seconds. Both peers
accept all 100000 names. The last preparation poll counts 3188 backup
objects by expected digest names and sizes, **without** independent content
hashing. A later diagnostic counts 3206 physical object files, also unverified.
The harness never reaches its final independent hashing of both peers.

| Observed preparation phase | Laptop | Backup |
| --- | --- | --- |
| Protection history reading | 95.675 s | 36.761 s |
| Location-claim sweep, 100000 keys | 352.498 s | 262.373 s |
| Initial local-presence sweep | skipped | 78.520 s |
| Later full local-presence sweep | 198.371 s | 117.528 s |

Preparation RSS high-water marks are 482373632 bytes for the laptop and
773599232 for the backup, above the reference limit of 300 MB. These are
preparation observations, not a completed budget window. The final snapshot
also shows a stopped native backup get group while metadata queries run;
that snapshot alone does not establish one cause of poor transfer progress.
Fast preparation remains explicitly nonqualifying. No idle CPU, memory,
ten-minute scan or HDD budget pass is inferred.

## Bounded bulk queries

Both location claims and content locations previously start a native child
for every 64 keys. Complete metadata reads now use at most 256 keys and
64 KiB of ordinary input per command. An individually oversized identity
stands alone so the partition still progresses. No identity is copied,
dropped or reordered by partitioning. Existing complete-response checks,
key binding, encoded-key fallback and actual-file checks remain required.
Location claims still supply neither byte proofs nor deletion authority.

A controlled native pipe fixture returns complete replies for 1025 keys.
The original implementation starts 34 children for the two sweeps and
fails the target of ten. With bounded batching it starts ten, preserving
every claimed identity and every absent answer. Four laws run 1000 generated
cases each: ordered coverage, count/UTF-8-byte bounds, greedy packing and
progress past an oversized identity.

A separate pinned-annex experiment uses 4096 synthetic absent keys in one
real repository. Three alternating trials compare sizes 64 and 256:

| Native query | Median at 64 | Median at 256 | Starts per sweep |
| --- | --- | --- | --- |
| whereis | 1.913703 s | 1.033553 s | 64 → 16 |
| contentlocation | 0.608004 s | 0.357967 s | 64 → 16 |

Every result is checked against all requested keys or blank absent answers.
This isolates native metadata overhead in a small repository; it establishes
no whole-daemon speedup, full reference result or hardware budget.

## Post-transfer lookups

The backup worker also previously starts a scalar `contentlocation` child
after each received key. It now reuses a worker-owned batch process for at
most 256 queries or 64 KiB of ordinary input, then rotates it. Only the
process is reused: presence and path results are never cached. The native
answer is reobserved each time, bound to its key, and checked for an actual
regular object. Encoded-key answers retain the existing scalar fallback.

A real annex fixture observes its object, moves that private fixture object
aside, sees absence, restores the exact bytes and sees it again. The scalar
prototype starts three children and fails its one-process target. The final
fixture also crosses 257 queries and requires exactly two child starts;
a separate UTF-8 pipe fixture requires rotation at the byte bound.
Transfer authority, key revalidation, synchronization and custody byte proofs
remain unchanged. The process belongs to the existing worker and stops or
drops before its pending-protection registration is released.

## Validation and next measurements

All 215 daemon library checks pass, followed by eleven native history,
backup-rejoin, health, purge-format and slice cases. These include normal
protection, freeing/keeping and a backup returning after a stop. A native
300-object fixture independently compares every returned object's bytes,
checks all holder identities and includes a missing key. Its first setup
attempt omits the driver's required `--no-check-gitignore` flag and is
refused before adding files; the corrected fixture passes. The original
212-pass/one-setup-failure log remains retained, not called a complete pass.

The focused macOS capture profile also selects these metadata regressions.
[Run 39146843909](https://github.com/dont-feed-the/pigeon/actions/runs/39146843909),
job 117732323777 at source
`474fa4b11ece1904f21615ce78a4ca1dfdc5a8e8`, passes all 15 selected native
checks in 88.002 seconds. The other 202 checks are filtered out, not executed.
The original finished log is retained with SHA-256
`f61ee58b31d7cafa19ae8a3a9d750b3c2245fa84a4e82471b44b17ddaf0aaba5`.
This includes native metadata freshness/rotation and capture/free/restore
checks, not the full macOS suite or production backup/resource qualification.
The previously successful APFS people profile stays separate.

`reference-budget --smoke --smoke-files N` accepts 1–4096 files for a larger
reduced fixture crossing metadata boundaries. It never certifies budgets;
custom counts are refused without `--smoke`. The full profile stays fixed
at 100000 files, 50 GB and a ten-minute window. The default smoke stays
32 files. Script contracts check these boundaries and nonqualification.

The full resource/HDD measurements, remaining global queue/native scheduling,
catalogue-memory work, security/law audit and elapsed M7 gates remain open.
A fresh full-size diagnostic must use the changed source and retain its own
results; the failed original is not retried unchanged.

## Extended native smoke

Clean source `1df86d380165bcd675c39411248592fbdd56341a` is built once
with the lab feature and all three programs frozen read-only. A 1024-file,
4096-byte-per-file probe runs two real local peers on an ordinary filesystem.
Both accept all 1024 names and retain all 1024 unique objects; the harness
independently hashes both peers' complete trees and every unique object.
All frozen program hashes match again after the run. This crosses multiple
bulk-query batches and worker-session rotations.

The report completes with `budget_verdict = not-measured`. Five seconds of
smoke sampling, lab filesystem seams and small content do not establish the
100000-file/50GB reference, CPU/RSS, physical-HDD or elapsed gates. The
final report-only checkpoint does not change the runtime or probe code.

## Integrated release validation

PR #114, head `474fa4b11ece1904f21615ce78a4ca1dfdc5a8e8`, integrates main `31e86654127e9733229026d2a7b8637c1bc8eee3` before validation.
CI 39146844152, lab 39146844129 and bridge
39146844237 pass. v0.6.44 is independently published
and verified first. Local validation passes 215 daemon library checks,
eleven native history/health/slice cases and 162 script contracts. Four new
metadata partition laws run 1000 cases each. Frozen lab source
`1df86d380165bcd675c39411248592fbdd56341a` independently verifies both peers'
complete 1024-file trees and objects; final head changes only report text
after that smoke. All-target workspace Clippy, actionlint and repository
rules pass. Original native regressions and fixture refusal remain retained.
Fresh v0.6.45 tag gates and independent publication verification are required.
The changed-source full-size Fast diagnostic and focused Mac check have
separate background results; no resource, HDD, elapsed or final-RC gate is
closed by the finite local checks or by this patch publication.
