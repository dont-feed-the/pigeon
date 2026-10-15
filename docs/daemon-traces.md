# Checking daemon traces

Build with `--features pigeond/tla-trace` and set `PIGEON_TRACE=1` to
record durable NDJSON observations. Production builds leave tracing off.
`PIGEON_TRACE_DIR` optionally archives logs outside a disposable test
worktree, under a hash of the canonical original trace directory.

Run the mandatory implementation-to-model regressions with:

```sh
scripts/check-driver-traces --out /tmp/pigeon-driver-traces
scripts/check-driver-traces --check-only --out /tmp/pigeon-driver-traces
```

This runs the real drivers, checks their observations with TLC and then
corrupts boundaries to ensure the checker rejects them. It requires the
same Rust, Git, git-annex and Java tools as CI. Omit `--out` for temporary
logs. `scripts/check-lab-traces OUT` checks a lab report directory; use
`--require SPEC` to require a particular family as well as checking every
family present. Unknown families, empty logs, malformed observations,
missing pairs and gaps in a device's local event clock fail the check.
The lab merger preserves read/parse errors as failing trace records.

For full daemon scenarios, use the release binaries built by `slice.yml`.
Repeated content hashing in a debug build can lengthen rename windows
enough to change timing-sensitive scenario observations.

| Emitted family | Checked behavior |
| --- | --- |
| `CaptureInstall` | capture, incoming install, retained asides, recovery |
| `Evict` | free and keep, including skip-worktree changes |
| `Thin` | hard-link replacement and unindexed completion |
| `Restore` | explicit recreation followed by capture |
| `Resolve` | replacement, withdrawal and divergence-table updates |
| `Roster` | accepted history, authority, membership fold and loaded roster |

The filesystem families share `CaptureInstallTrace.tla`, which uses
`DaemonDriver.tla` and the original `CaptureInstall.tla` actions.
`RosterDaemonTrace.tla` uses `Roster.tla`'s change and history fold. These
adapters check observed runs; the existing bounded model configurations
continue to check their original design properties.

## Observation contract

Each filesystem operation records before and after states. These include
actual inode identities, content hashes, writer observations, path/aside/
stage/kept locations, index pointers, sparse bits, signed reachable hashes,
journal flags and the journal's expected stat. Stat equality must justify
the rename proof. Positive clean-cache observations constrain the model;
a conservative dirty observation need not imply changed bytes.

The observer reads Git index formats 2, 3 and 4 directly: even `ls-files`
can refresh skip-worktree state. Unsupported required index extensions
fail explicitly. Git history and authority are checkpointed before the
critical rename window; synchronous observations in that window refresh
files, journal and the raw index, and reject a changed pointer. With tracing
off, these observations perform no filesystem or Git reads.

Writer facts inside the rename window come from the driver's actual scan
input. Other critical edges use `writers: null` (unobserved); TLC leaves
those handles unconstrained. Repeated process-wide scans at every edge
would lengthen the missing-path window. The adapter refuses a `Scan` event
without its actual boolean input. Outside that window, snapshots scan for
writers independently.

The encoder renames inode/content identities while preserving equality.
It projects signed history onto hashes seen so far in that path, including
an explicit restore request, so another path's batch capture cannot become
this path's capture. Permission and recent-install facts are observed
inputs. Driver actions cannot be inserted to fill a missing trace step.

A killed process may leave a before record without an after record. Only
a real checkpoint from a different process run can close that prefix. TLC
must explain a completed or ineffective prefix followed by recovery.
Closed journal entries with missing completion observations also produce
a recovery checkpoint. Event clocks resume from the durable append log.
A trailing unmatched record, failed observation or unexplained transition
fails rather than becoming an unchecked family.

CI requires normal/concurrent/recent captures (including a recent save by
someone who cannot modify the original name), all six filesystem modes,
changed-aside release, stale presence races, roster forks, index formats
and eight real incoming-install SIGKILL cases, including gaps after pointer and journal
writes but before their after observations, plus a restore of a deleted
path interrupted after proof. Negative checks require TLC
counterexamples for wrong safety outputs, omitted proof/index steps and
bad roster folds; schema and pipeline checks reject missing fields/logs.

## Limits of this evidence

This checks finite executions from their first observed checkpoint, with
at most four environment steps between operations. It is not a universal
refinement proof. Unobserved writes that disappear entirely cannot be
reconstructed. Arbitrary partial I/O failures and finer interleavings may
need a richer adapter; unstable or unexplained observations currently fail.

The roster projection checks people, administrators, device ownership and
retirements. Raw signed changes and roster TOML are retained, but names,
folders, UUIDs and settings are outside this projection. Conflicts in
those fields that affect atomic edit acceptance need additional modeling.

The filesystem adapter checks inode/content preservation, journal protocol
and signed reachable references. It abstracts catalogue winner selection,
which has separate core tests. It does not establish physical annex-object
durability, custody/protection, remote availability, drop/repair safety,
wall-clock retention duration, liveness or power-loss durability. Existing
models and storage regressions remain necessary for those properties.
