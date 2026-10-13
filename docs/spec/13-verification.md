# 13. Verification

> Mission: specify how pigeon shows that it keeps its guarantees: the TLA+
> models and what each must prove, trace validation of real runs, the laws
> and property-based tests, fault injection, contract tests against git and
> git-annex, the lab, CI, and the criteria (technical and human) for going on.

## 13.1 Layers of evidence

| Layer | Checks | Decided by |
|---|---|---|
| TLA+ models (§13.2) | the protocols, for all interleavings within bounds | ADR 0008 |
| Trace validation (§13.3) | that real runs are behaviours of those models | ADR 0008 |
| Laws (§13.4) | the pure core, for all generated inputs | ADR 0012 |
| Stateful tests and fault injection (§13.5) | the shell with real git and git-annex, under faults | ADR 0012 |
| Contract tests (§13.6) | that git and git-annex behave and print as pigeon assumes | ADR 0004, ADR 0020 |
| The lab (§13.7) | git-annex's own behaviour on which the design rests | ADR 0001 |
| Budgets (§13.9) | that background work stays out of the way | ADR 0037 |
| Usability test (§13.10) | that people understand the states | ux.md §9 |

## 13.2 TLA+ models

### Existing (`models/tla/`)

| Configuration | Model | Expected verdict | Shows |
|---|---|---|---|
| `Naive_FALSE` | `CustodyNaive` | `violation:NoLoss` | a naive "ask, then drop" hand-off loses the last copy |
| `Naive_TRUE` | `CustodyNaive` | `deadlock` | the same with locks deadlocks |
| `T_idem` | `CustodyTokens` | `violation:TokensNeverLost` | idempotent acknowledgements lose tokens |
| `T_nononce` | `CustodyTokens` | `violation:TokensNeverLost` | offers without nonces lose tokens |
| `T_nonce` | `CustodyTokens` | `pass` | nonces make the hand-off safe |
| `T_n42o3`, `T_n31o5`, `T_n32o5` | `CustodyTokens` | `pass` (fast set) | N, K and offers varied |
| `T_n42o4` | `CustodyTokens` | `pass` (full set) | N = 4, K = 2, 4 offers |
| `Purge_full` | `Purge` | `pass` (fast set) | ADR 0042: a restore or cancel that reaches any custodian before its seal keeps the content (KeepWins); nothing is dropped before the waiting period (WaitingPeriod); a dropped copy returns only for an uncovered reference (NoResurrection); 2 custodians (one holds the key), 2 laptops (one offline with a copy), restarts, any message order |
| `Purge_full3` | `Purge` | `pass` (full set) | 3 custodians, 2 of them holding the key, and the deciding laptop with a copy: 12.4 million states, 2 minutes |
| `Purge_seals` | `Purge` | `violation:KeepWins` | accepting every seal without merging the sealers' `main` loses a cancel |
| `Purge_none` | `Purge` | `violation:KeepWins` | dropping when one's own waiting period ends (ADR 0018's first mechanism) loses a restore that reached another custodian |

The violations and the deadlock are negative tests: a model edit that hides
the failure is caught (ADR 0026). These models argue why custody hand-off
needs locking and fresh identifiers, and why naive designs fail. In v2 the
mechanism is git-annex's lock-based verified drop (E1, E5), not pigeon's own
tokens; the planned `AnnexDrop` model below states what pigeon relies on.

### Planned, before or with the code (ADR 0008)

| Model | Actions | Safety | Liveness (under fairness) |
|---|---|---|---|
| `CaptureInstall` ([07 §7.7](07-capture-and-install.md)) | `Save`, `Scan`, `Capture`, `InstallStep`, `Crash`, `Recover`, `Decide`, `Resolve` | INV-Capture, INV-NoOverwrite, INV-Journal | every save is captured; every clean path reaches `current(p)` |
| `Authority` ([04](04-roster-and-authority.md)) | `Receive`, `Validate`, `Merge`, `Propose`, `Adopt`, `Reject`, roster changes | INV-Valid, INV-Merge: devices with the same commits have the same catalogue | connected devices converge |
| `AnnexDrop` | lock, check, drop, location-log updates, stale logs, `dead`, partitions | no key below K custody copies except by purge or Drop (INV-K) | — (a model of git-annex, checked against E1, E5) |
| `Evict` ([08 §8.5](08-custody-loops.md)) | free, keep, verified drop, sparse hide and show | a laptop drops only protected keys; a kept folder never shows pointers | a freed folder's space is freed once its files are protected |
| `Repair` ([08 §8.4](08-custody-loops.md)) | custodian loss, `dead`, want, get, scrub failure | no protected version is lost while one custody copy remains | K copies are restored when capacity allows |

Bounds start at N ≤ 4 devices, K ≤ 2, a few paths and keys, as now
(ADR 0008); larger bounds run in the full set. Each model has a negative
configuration that removes one safeguard and must fail.

**Safety and liveness are stated apart.** Safety properties (the invariants of
[02 §2.5](02-guarantees.md)) must hold in every behaviour, without fairness.
Liveness properties hold only under explicit assumptions, written as fairness
conditions in each model: every pair of devices that stays online is eventually
connected; a custodian that is not removed is eventually online; capacity is at
least K × the content wanted ([08 §8.9](08-custody-loops.md)); a person
eventually answers what *Needs you*. A liveness failure without these
assumptions is expected and is not a bug.

## 13.3 Trace validation (ADR 0008)

- Each model has a trace module `*Trace.tla` that reads the merged NDJSON
  events ([12 §12.3](12-observability.md)) and constrains `Next` to the
  logged step; variables not traced stay non-deterministic (the method of
  Cirstea, Kuppe, Merz and others, arXiv 2404.16075).
- Every fault-injection run in CI is trace-validated; a mismatch fails the
  build and reports the first log line no behaviour of the model explains.
- **TLC version.** ADR 0008 assumes TLC reads NDJSON with `ndJsonDeserialize`
  from the `Json` module. The pinned TLC 1.7.4 (ADR 0026) does not ship it;
  ADR 0026 foresees pinning, by SHA-256, a build that has it (a TLC 1.8 build,
  or the CommunityModules jar beside 1.7.4). That choice needs its own ADR
  before the first trace module ([16](16-open-questions.md)).
- Trace validation shows that observed runs conform; it is not a proof of
  the code.

## 13.4 Laws and property tests (ADR 0012)

The core is pure functions over values; git, git-annex, the file system and
iroh sit behind traits in a thin shell. Every core structure states its laws,
and each law is a `hegeltest` property (pinned to 0.49, a dev-dependency).

| Structure | Laws | Where |
|---|---|---|
| fetched history | join-semilattice: associative, commutative, idempotent union | [08 §8.2](08-custody-loops.md) |
| acceptance | `accepted(S ∪ T) = accepted(S) ∪ accepted(T)`; verdicts depend only on ancestors; a clean merge has Δ = ∅ | [04 §4.4](04-roster-and-authority.md) |
| catalogue merge | commutative, associative, idempotent; conflict names independent of arrival order | [05 §5.2](05-catalogue-and-names.md) |
| portable names | normalisation idempotent; injective within a folder; portable names unchanged | [05 §5.4](05-catalogue-and-names.md) |
| custody observations | latest-observation join per (custodian, key); *protected* monotone in presence observations | [06 §6.1](06-status.md) |
| headline | a pure function of the states; precedence total; never *Safe* while a version is not protected | [06 §6.5](06-status.md) |
| released keys | isolation: releasing one reference never releases a key another holds, and an uncovered reference removes its key; seal safety; monotone in the seals; independent of arrival order; never a current version | [08 §8.6](08-custody-loops.md), ADR 0042 |
| settings | expected git-annex settings are a function of the roster; applying twice changes nothing | [03 §3.5](03-repository-and-roles.md) |
| install journal | replay idempotent; each path ends old or new | [11 §11.5](11-local-state.md) |
| local files | round trip; newer formats refused | [11 §11.5](11-local-state.md) |
| append-only filter | never forwards a removal; forwards allowed messages unchanged; keeps `DATA` framing | [09 §9.4](09-network-and-joining.md) |
| trace merge | Lamport merge associative and commutative; per-device order kept | [12 §12.3](12-observability.md) |
| work queue | order is a total preorder of the classes of ADR 0037; capture never starves | [15 §15.2](15-resource-use.md) |
| encodings | `parse(print(x)) = x` for roster TOML, NDJSON, git objects; our parser agrees with git | everywhere |

Example tests remain for regressions found by properties (with the shrunk
input) and for the fixed scenarios of this specification.

## 13.5 Stateful tests and fault injection

hegel's state-machine tests drive real repositories (git and git-annex in
temporary directories, several devices in one process or in containers)
through random operations, compare them with a model, and validate the trace:

- **Operations:** saves through every save pattern (write in place, write a
  temporary file and rename, delete and recreate, write through an old
  handle), renames, deletions, mass deletions, edits in another person's
  folder, decisions, invitations, roster changes, freeing and keeping folders,
  purges, Drop.
- **Faults:** `kill -9` of `pigeond` or a git-annex child at any step; disk full;
  an unplugged drive; network partitions and reconnection; clock skew; a
  custodian lost and replaced; a relay down.
- **Scale:** at least 1000 randomised schedules per CI run of the full set
  (proposal §7); the fast set runs fewer on every push.
- **Oracles:** INV-K (no protected version lost, except by purge or Drop), the
  invariants of [02 §2.5](02-guarantees.md), no file a person saved is
  overwritten, and trace validation.

## 13.6 Contract tests against git and git-annex

Every command pigeon runs ([03 §3.7](03-repository-and-roles.md)) has tests
that run it against the minimum supported git (2.38) and git-annex
(10.20261006), and the newest, and check the JSON and porcelain output pigeon
parses, the exit status, and the behaviour pigeon relies on (for example that
`drop` refuses without K live custody copies). A new git-annex version is
supported only once they pass ([14 §14.3](14-platforms-and-packaging.md)).

## 13.7 The lab (`lab/git-annex/`)

| Experiment | Result | Used by |
|---|---|---|
| E1, E5 | concurrent drops of the last two copies: 40/40 left one; a stale location log is not trusted | INV-K, [08 §8.3](08-custody-loops.md) |
| E2–E4, E6 | deleted content kept until `dropunused`; a propagated `rm -rf` undone; concurrent edits keep two variants; absent content is a pointer file | [05](05-catalogue-and-names.md), [07](07-capture-and-install.md) |
| E7 | a pull filter merges an allowed contribution and keeps a disallowed edit as a proposal | [04](04-roster-and-authority.md) |
| E8 | `annex.thin`: an in-place edit corrupts the laptop's copy; others refuse to count it | ADR 0002, [07 §7.5](07-capture-and-install.md) |
| E9 | roles: untrusted laptop, two custodians, K = 2; `--all` needed for history | [03](03-repository-and-roles.md), [08](08-custody-loops.md) |
| E10 | partial custodians: `sizebalanced` is heuristic (6/10 where 8/10 fit); repair within capacity | ADR 0003, [08 §8.9](08-custody-loops.md) |
| E15 | freeing per folder: sparse checkout, drop and get by key, `smudge --update` (asserts) | ADR 0031, [08 §8.5](08-custody-loops.md) |
| E15b | ADR 0031's order shows pointer files (44 887 and 1 306 observations for 100 files); hiding before dropping and staging before showing show none, also under `kill -9` (asserts) | ADR 0039, [08 §8.5](08-custody-loops.md) |
| E18 | five offline saves of one file, failed uploads, reconnection: with `annex.thin` in-place saves lose 4 of 5 versions; capturing a copy keeps all 5, and protection, drop by key and re-thinning return to about one copy; `drop --auto --all` is refused (asserts) | ADR 0041, [07 §7.8](07-capture-and-install.md) |
| E19 | install against an application acting at every boundary (in place, atomic replace, create, delete, old handle, shared mapping), thin and not thin, 21 `kill -9` points recovered twice, a fuzzing writer: no save lost, the index never calls a changed file clean, recovery idempotent; without the writer scan 9 of 49 writes reach the aside (asserts) | ADR 0040, [07 §7.5](07-capture-and-install.md) |

**Pending:** E11 native Windows (v1.1); E12 `remotedaemon` only fetches,
never merges; E13 `ServeAppendOnly` over P2P (ADR 0014); E14 purge with git-annex:
per-key transfers skip released keys, the forced drop, a new generation uploaded
again, `matchexpression --key` agreeing with `wanted` for `sizebalanced` (ADR 0042). **New:** E16 `annex.bwlimit` over the P2P
transport; E17 the HDD video-editing acceptance test ([15 §15.8](15-resource-use.md)).
E9 and E10 must be rerun with bare custodian repositories
([03](03-repository-and-roles.md)). Each experiment joins CI once its script
asserts its results, as E15 does (ADR 0026).

## 13.8 Continuous integration (ADR 0026)

- `ci` on every push and pull request: the repository rules
  (`scripts/check-file-length-and-header`, `scripts/check-adr-index`), the fast
  model set; with the first crate, `cargo fmt --check`, `clippy` (with
  `unwrap_used` and `expect_used` denied, ADR 0023), tests with the fast
  property and fault sets, the Fluent completeness test (ADR 0028), the
  licence check (`cargo deny`, ADR 0019), E15 and every asserting experiment.
- `models-full` when a model or the runner changes, weekly, and by hand: every
  configuration, the full fault-injection set with trace validation, E17 on a
  self-hosted runner with an HDD **[no ADR: runner]**.
- TLC pinned by SHA-256; actions pinned by commit; read-only permissions.
- Fault-injection runs keep their OTLP-JSON and trace files as artifacts
  (ADR 0025).

## 13.9 Resource budgets (ADR 0037)

CI measures `pigeond` on a reference tree: idle CPU below 1 % of one core over
10 minutes, bounded memory, and the throughput and latency budgets of E17
([15 §15.8](15-resource-use.md)). A regression fails the build.

## 13.10 Coverage map: from guarantees to evidence

What carries each guarantee today, and what is still missing. "Planned" items
are specified here but not built; the gaps are tracked as GitHub issues
([16 §16.5](16-open-questions.md)). Passing CI today is evidence only for the
"now" column.

| Guarantee | Now | Planned | Gaps (issues) |
|---|---|---|---|
| P1 Survival | E1, E5, E8, E9, E10 (lab, printing); `CustodyNaive`, `CustodyTokens` (evidence about hand-offs in general and about the earlier token protocol, not about git-annex) | `AnnexDrop`, `Repair` models; fault injection; E13; asserting E9, E10 | append-only serving unproven (#9); failure domains (#12); capacity admission (#13); recovery on fresh machines (#11) |
| P2 Capture, no overwrite | E15b (asserting: no pointer during free and keep); E18 (asserting: offline in-place saves keep every version); E19 (asserting: racing saves and `kill -9` during install) | `CaptureInstall` model; install journal laws (hegel-rust); E19 on macOS | copy-on-write cost of pending history on APFS and btrfs (E18 there); file-system scope (#14); saves from stale memory (16 §16.4) |
| P3 Reversible deletion | E2, E4 (lab, printing); `Purge` model (seals against restores, cancels, partitions, restarts and a returning laptop; ADR 0042) | E14; purge laws (hegel-rust); `Purge` trace validation | a custodian clock that jumps forward by more than the waiting period; erased names stay in history (16 §16.4) |
| P4 Authority | E7 (lab, printing) | `Authority` model; acceptance laws; decision-rights tests (ADR 0038) | roster forks (#6); format compatibility (#15) |
| P5 Convergence | E3 (lab, printing) | merge and name laws; three-way permutations | the merge algorithm (#8) |
| P6 Honest status | — | observation and headline laws; usability test | — |

## 13.11 Going on: technical and human criteria

**Go** for v1 if all hold (proposal §7, restricted to the v1 platforms of
ADR 0005): zero loss of protected versions in every fault schedule and in
every model; laptop disk use at most 1.1 × the content of the folders kept;
test users explain *only on this computer*, *shared, not safe yet* and *safe*
correctly. Otherwise, requirements are reviewed before any engine is built.
The Windows criteria of the proposal move to v1.1.

**Usability test** (ux.md §9), before code with the mockups
(`docs/design/mockups/`), then with the real system: five participants per
round, not technical, plus two "Pauls" for setup; moderated think-aloud,
30 minutes, seven tasks. Pass: at least 4 of 5 explain the three states
correctly; at least 4 of 5 complete tasks 4 and 5 (an old version, a deleted
folder) without help; **no** participant believes that a file which is not
safe is safe. The thresholds of [06 §6.7](06-status.md) are tuned from it.
