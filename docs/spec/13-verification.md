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
| `Roster_fold` | `Roster` | `pass` (fast set) | issue #6: the roster of a commit is the fold of its non-merge ancestors' changes in canonical order (Kahn, smallest (time, device, id) first), each change atomic or skipped and reported, device removals always applied, a merge signed by a device of either parent's roster. Every roster keeps an admin (AdminRemains); a removed device stays removed (RemovalWins); each tree's roster is that fold, so merge order does not matter (TreeIsFold, Agreement); concurrent heads can always be merged (MergeAlwaysPossible); a change is skipped only for a concurrent one (SkipsNeedConcurrency). 3 persons (2 admins), 2 devices committing offline and merging any commit, a fresh key, all seven kinds of change, 5 commits: 608 563 states, 442 463 distinct, 11 s on 3 workers |
| `Roster_clock` | `Roster` | `pass` (full set) | the same with author times 0 or 1, so wrong clocks reorder the fold: 8 569 377 states, 6 224 785 distinct, 4 minutes on 2 workers |
| `Roster_fold3` | `Roster` | `pass` (full set) | three devices committing and merging, 5 commits: 10 321 071 states, 4 277 212 distinct, 5 minutes on 1 worker |
| `Roster_witness` | `Roster` | `violation:NoConcurrentRetirement` | a and b retire concurrently and a merge joins them: the first in canonical order applies, the other is skipped, b stays admin |
| `Roster_atomic` | `Roster` | `violation:RemovalWins` | skipping a rotation as a whole also skips its removal: a key added and removed on one branch makes the rotation to it fail on the other, and the old key comes back |
| `Roster_every` | `Roster` | `violation:MergeAlwaysPossible` | with rule 2 of [04 §4.4](04-roster-and-authority.md) (a signer in every parent's roster), branches that removed each other's devices have no common device and never merge |
| `Roster_person` | `Roster` | `violation:PersonRemovalWins` | a removes b while b removes a: both cannot apply without leaving no admin, so the later one in canonical order is skipped |
| `Roster_rebase` | `Roster` | `violation:Agreement` | replaying the other side's changes onto one's own roster depends on who merges: two merges of the same commits disagree |
| `Roster_table` | `Roster` | `violation:AdminRemains` | merging `roster.toml` per TOML table, a clean merge of the concurrent self-retirements leaves no admin |
| `Roster_adr0009` | `Roster` | `violation:MergeAlwaysPossible` | ADR 0009's rule (a merge of concurrent roster changes signed by an admin under both parents) leaves the concurrent self-retirements unmergeable: no person is admin on both sides |

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
| roster fold | R(c) depends only on the set of accepted ancestors (merge order, duplicate delivery, split and rejoin cycles); a linear history gives sequential application; every R(c) has an admin; a removed device never returns; a change applies at most once; every skipped change is listed (ADR 0051) | [04 §4.8](04-roster-and-authority.md) |
| catalogue merge | commutative, associative, idempotent; conflict names independent of arrival order | [05 §5.2](05-catalogue-and-names.md) |
| portable names | normalisation idempotent; injective within a folder; portable names unchanged | [05 §5.4](05-catalogue-and-names.md) |
| custody observations | latest-observation join per (custodian, key); *protected* monotone in presence observations; a custodian counts only with `has_commit` for the version (ADR 0046) | [06 §6.1](06-status.md) |
| headline | a pure function of the states; precedence total; never *Safe* while a version is not protected | [06 §6.5](06-status.md) |
| released keys | isolation: releasing one reference never releases a key another holds, and an uncovered reference removes its key; seal safety; monotone in the seals; independent of arrival order; never a current version | [08 §8.6](08-custody-loops.md), ADR 0042 |
| settings | expected git-annex settings are a function of the roster; applying twice changes nothing | [03 §3.5](03-repository-and-roles.md) |
| install journal | replay idempotent; each path ends old or new | [11 §11.5](11-local-state.md) |
| local files | round trip; newer formats refused | [11 §11.5](11-local-state.md) |
| compatibility | an older release's verdict is the newer one's or *unknown*; an unknown field or `format` is never ignored; an unreadable status report never raises a state (ADR 0048) | [04 §4.4](04-roster-and-authority.md), [06 §6.3](06-status.md) |
| append-only filter | never forwards a removal; forwards allowed messages unchanged; keeps `DATA` framing; no positive answer for a key before its sync (ADR 0046) | [09 §9.4](09-network-and-joining.md) |
| trace merge | Lamport merge associative and commutative; per-device order kept | [12 §12.3](12-observability.md) |
| work queue | order is a total preorder of the classes of ADR 0037; capture never starves; a key held in fewer failure domains never waits behind one held in more (ADR 0050) | [15 §15.2](15-resource-use.md) |
| failure domains | domains are the connected components of shared storage, host and declared `domain`; their number is at most the number of custodians; a fingerprint or declaration never raises a state; with one custodian per domain the state is ADR 0002's; a drop's chosen remotes lie in K distinct domains; domain repair is deterministic and wants nothing for a key that spans K domains (ADR 0049) | [06 §6.2](06-status.md), [08 §8.3](08-custody-loops.md) |
| capacity | the parts of the need sum to the total; every key not protected has a reason; admission never defers an endangered key for one that is not (ADR 0050) | [08 §8.9](08-custody-loops.md) |
| trees and names | every tree capture makes is well formed (rule 6); `package(p)` depends only on *p*'s name and is stable when other paths change; merging a package never mixes files of two sides; `ignored` is a pure function of the name | [07 §7.9](07-capture-and-install.md), [05 §5.2](05-catalogue-and-names.md) |
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
A subset runs on each device as the contract self-test when it first meets a
new version (ADR 0048).

**Format fixtures** (ADR 0048). `fixtures/formats/` holds golden documents and
small signed histories for every supported (`format`, `rules`) pair, each with
its expected verdicts, and fixtures of the next format that the current
release must judge *unknown*. Every release runs them. Two releases built
from two commits, in one group, are a prototype test (#17).

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
| E14 | purge by references with git and git-annex, two laptops and two bare custodians: two paths sharing a key, current content sharing a key with purged history and a suggestion sharing a key with accepted content keep their key (the suggestion through the seals' `held`); a restore that reached one custodian before its seal holds the purge back at the other until it merges that `main` (trusting the seals alone would release it); per-key transfers never fetch a released key although git-annex's own `wanted` still matches it; `drop --force --key` leaves no copy; a returning laptop drops its covered copy and its offline re-add comes back to both custodians; `matchexpression --key` agrees with git-annex's choices (`balanced` with `satisfy`, 11 keys split between the two; `sizebalanced` with `find --want-get`, which lists nothing in a bare repository); `get --auto --all` and `drop --auto --key` are refused and `satisfy --all` drops from a remote (asserts) | ADR 0042, [08 §8.6](08-custody-loops.md) |
| E12 | `remotedaemon` over git-annex's generic P2P transport (a lab network): on a change notice it fetches by the remote's refspecs and never moves `main`, `HEAD`, the index or the working tree; by default a peer pushes any ref into a custodian, `main` included, and into a laptop except its checked-out branch; a `pre-receive` hook refuses (asserts) | ADR 0043, [03 §3.6, §3.8](03-repository-and-roles.md) |
| E13 | `remotedaemon` serves `REMOVE` even with `GIT_ANNEX_SHELL_APPENDONLY`; append-only `git-annex-shell p2pstdio` refuses it but relays no git; pigeon's filter before `remotedaemon`, with random read sizes, passes reads, stores, fetches and change notices, refuses `REMOVE` on the custodian and any push, caps `VERSION`, closes on unknown or malformed input, passes payloads holding protocol lines intact; explicit `--numcopies` outweighs `.gitattributes` and a lowered branch setting, which `drop --key` otherwise obeys (asserts; 30 checks, about 90 s) | ADR 0043, [09 §9.4](09-network-and-joining.md) |
| E20 | what the folder may hold, on Linux and macOS (`lab.yml`, 53 checks each): links, nested repositories, FIFOs and sockets are never staged when capture takes paths from `git status` and checks `lstat` first, yet `git annex add` stages a link or a nested repository's file it is given; `git annex add` stages an executable file as 100644 and ignores a mode-only change, and `update-index --chmod` restores the bit; extended attributes do not travel; a thin object is hard-linked to one path only, and `fsck` does not notice an object changed through that link; unreadable paths are not reported deleted; a file rewritten while it is added never yields an object that mismatches its key; a person's `~/.gitconfig` (`annex.backend=WORM`, `annex.largefiles=nothing`, `core.excludesFile`) changes keys, tree and listing unless git runs isolated; `--ignored=traditional -uall` lists the files of ignored folders; `.git/info/attributes` overrides nested `.gitattributes`; `add --batch` fails on a name that is not UTF-8 (asserts) | ADRs 0044, 0045, [07 §7.9](07-capture-and-install.md), [03 §3.7](03-repository-and-roles.md) |
| E21 | the disaster drill (issue #11): content on K custodians before they have its commit is a key without a name once the laptop is lost; after losing every laptop and its key, a fresh machine recovers from custodians alone every name, hash, deleted version and suggestion, and every signature verifies against the roster at its parents; a recovery sheet's roster commit is accepted and nothing else it signs; `git clone` and `git annex get` read the files without pigeon, across partial custodians; a copied repository under a new UUID counts only after a hashing fsck; git's default `core.fsync` syncs neither loose objects nor refs (asserts) | ADRs 0046, 0047, [09 §9.9](09-network-and-joining.md), [06 §6.2](06-status.md) |
| E22 | failure domains and capacity (issues #12, #13), on Linux and macOS, 7 checks: fingerprints of disks and machines (one volume, one fingerprint; a loop mount or disk image follows its file to its disk; a tmpfs differs, and its mount source alone collides); git-annex counts two repositories on one disk as two copies, even one hard-linked inode; a drop that ignores all but one remote per domain refuses until another domain holds the key; `annex.diskreserve` refuses a get and an incoming copy, local and over `p2pstdio`, with a misleading message at the sender; `sizebalanced` with small `maxsize` leaves keys at one copy and the verified drop refuses to go lower; a real `ENOSPC` (another writer; git-annex alone stops at the free space) fails a get and a fetch cleanly, `git annex fsck` fails until space is freed, the retries succeed; a custodian killed during a copy counts only complete keys, and a deleted one is written into again and still claimed, which only the verified drop catches (asserts). Found on the way: `annex-bwlimit-download` is never read (`Types/GitConfig.hs` maps it to `bwlimit-upload`) | ADRs 0049, 0050, [08 §8.3, §8.9](08-custody-loops.md) |

**Pending:** E11 native Windows (v1.1). **New:** E16 `annex.bwlimit` over the P2P
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
- `lab` on every push: E20 on Linux and macOS, with the git-annex each
  installer uses (the standalone build, Homebrew); the other asserting
  experiments join it (issue #10).
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
| P1 Survival | E1, E5, E8, E9, E10 (lab, printing); E13 (asserting: pigeon's filter refuses remote `REMOVE`); E21 (asserting: recovery from custodians alone); E22 (asserting: one copy per failure domain, the reserve, disk full); `CustodyNaive`, `CustodyTokens` (evidence about hand-offs in general and about the earlier token protocol, not about git-annex) | `AnnexDrop`, `Repair` models; failure-domain and capacity laws; fault injection; asserting E9, E10 | independence pigeon cannot see (virtual machines, places not declared; ADR 0049); power loss on a custodian (outside the lab; ADR 0046 rests on the git and git-annex sources) |
| P2 Capture, no overwrite | E20 (asserting, Linux and macOS: what capture stages and what it never reads); E15b (asserting: no pointer during free and keep); E18 (asserting: offline in-place saves keep every version); E19 (asserting: racing saves and `kill -9` during install) | `CaptureInstall` model; install journal laws (hegel-rust); E19 on macOS | copy-on-write cost of pending history on APFS and btrfs (E18 there); saves from stale memory (16 §16.4) |
| P3 Reversible deletion | E2, E4 (lab, printing); `Purge` model (seals against restores, cancels, partitions, restarts and a returning laptop; ADR 0042); E14 (asserting: shared keys, a restore against the seals, per-key transfers, the forced drop, a returning laptop's new generation) | purge laws (hegel-rust); `Purge` trace validation | a custodian clock that jumps forward by more than the waiting period; erased names stay in history (16 §16.4) |
| P4 Authority | E7 (lab, printing); E12 (asserting: no pushed ref, `remotedaemon` only fetches); `Roster` model (roster forks, ADR 0051) | `Authority` model; roster fold laws; acceptance laws; decision-rights tests (ADR 0038) | format compatibility (#15) |
| P5 Convergence | E3 (lab, printing) | merge and name laws; three-way permutations | the merge algorithm (#8) |
| P6 Honest status | E20 (asserting: the kinds pigeon lists as not shared); E21 (asserting: content without its commit is not protected); E22 (asserting: two repositories on one disk are two copies to git-annex, so pigeon counts domains) | observation, domain and headline laws; usability test | — |

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
