# 13. Verification

> Mission: the layers of evidence and what each must show, the coverage of
> P1–P6, the criteria for going on, the safety slice, and the laws in code.

## 13.1 Layers of evidence

| Layer | Checks | Decided by |
|---|---|---|
| TLA+ models (§13.2) | the protocols, for all interleavings within bounds | ADR 0008 |
| Trace validation (§13.3) | that real runs are behaviours of those models | ADR 0008 |
| Laws (§13.4, §13.13) | the pure core, for all generated inputs | ADR 0012 |
| Stateful tests and fault injection (§13.5) | the shell with real git and git-annex, under faults | ADR 0012 |
| Contract tests (§13.6) | that git and git-annex behave and print as pigeon assumes | ADRs 0004, 0069 |
| The lab (§13.7) | git-annex's own behaviour on which the design rests | ADR 0001 |
| Budgets (§13.9) | that background work stays out of the way | ADR 0037 |
| Usability test (§13.11) | that people understand the states | ux.md §9 |
| The slice (§13.12) | that the first build keeps the promises end to end | issue #17 |

## 13.2 TLA+ models

Each protocol has a model in `models/tla/` (ADR 0008).
`models/tla/expected.tsv` lists every configuration with its set (fast in
`ci`, full in `models-full`) and its expected verdict; `scripts/run-tlc`
runs them; `models/tla/README.md` says what each shows and leaves out. The
invariants this specification cites are normative.

| Model | Invariants; liveness | Specified in |
|---|---|---|
| `CaptureInstall` | `NoLoss`, `Journal`, `IndexHonest`, `CaptureFirst`, `NoOverwrite`, `DeletionKept`; `SavesCaptured`, `Converges` | [07 §7.5](07-capture-and-install.md), [07 §7.7](07-capture-and-install.md) |
| `Evict` | `NoLoss`, `NoPointer`, `DropsProtected`, `CaptureOnlySaves`, `NoDeletion`; `FreeLive`, `KeepLive` | [08 §8.5](08-custody-loops.md) |
| `Repair` | `Survival`, `INV_K` (of steps), `ReserveKept`, `EndangeredFirst`; `Restores` | [08 §8.3–8.4](08-custody-loops.md), [08 §8.9](08-custody-loops.md) |
| `CaptureTransaction`, `RecoveredNames`, `Retention` | `CapturedBeforeDisplacement`, `EverySaveKept`, `Complete` | capture's partial-failure states, recovery names across batches, the retained-key input to repair |
| `Admission` | `ReserveKept`, `EndangeredFirst`, `DeferredStays` | [08 §8.9](08-custody-loops.md) |
| `AnnexDrop` | `INV_K`, `LockedStays` | [08 §8.3](08-custody-loops.md), [09 §9.4](09-network-and-joining.md) |
| `Purge` | `KeepWins`, `WaitingPeriod`, `NoResurrection` | [08 §8.6](08-custody-loops.md) |
| `Roster` | `AdminRemains`, `RemovalWins`, `PersonRemovalWins`, `TreeIsFold`, `Agreement`, `MergeAlwaysPossible`, `SkipsNeedConcurrency`; `NoConcurrentRetirement` is a witness, violated on purpose | [04 §4.8](04-roster-and-authority.md) |
| `Authority` | `AuthorisedContent`, `AdoptionHoldsContent`, `DecidersCanAdopt`, `DecisionsByDeciders`, `HonestMergesAccepted`, `TreeIsCatalogue`, `RemovalRevokesAdded`, `RemovedSilenced`, `ConvergedWhenQuiet`; `Converge` | [04 §4.3–4.5](04-roster-and-authority.md), [09 §9.7](09-network-and-joining.md) |
| `CustodyNaive`, `CustodyTokens` | `NoLoss`; `TokensNeverLost`, `TokenHolderHasContent`, `KCopies`, `AtMostOneToken` | evidence about hand-offs in general, not about git-annex |
| `Catalogue` | none: the merge of the catalogue over abstract slots, which `CatalogueTrace` replays | [05 §5.2](05-catalogue-and-names.md) |
| `TraceSample`, `RosterTrace`, `AdmitTrace`, `AuthorityTrace`, `CatalogueTrace` | `TraceNotConsumed` (§13.3) | [12 §12.3](12-observability.md) |

Per safeguard, each model has a configuration without it (`Off`) that must
fail with the verdict `expected.tsv` names (ADR 0026). Bounds start at
N ≤ 4 devices, K ≤ 2, a few paths and keys; larger ones run in the full
set. Safety (the invariants of [02 §2.5](02-guarantees.md)) holds in every
behaviour, without fairness. Liveness holds under the fairness each model
writes: devices that stay online eventually connect; a custodian not
removed is eventually online; capacity is at least K × the content wanted
([08 §8.9](08-custody-loops.md)); a person eventually answers what *Needs
you*. A liveness failure without them is not a bug.

## 13.3 Trace validation (ADR 0008)

Each model with events has a `*Trace.tla` that reads the merged NDJSON
events ([12 §12.3](12-observability.md)) and constrains `Next` to each
logged step, leaving untraced variables free (arXiv 2404.16075).
`scripts/trace-to-tla` writes the log as a module for the pinned TLC
(ADR 0026), refusing values it cannot encode exactly and lines out of
Lamport order. `TraceNotConsumed` is violated exactly when a behaviour
explains every line (`trace:accepted`); otherwise the depth reached names
the first unexplained line (`trace:rejected:N`), which fails the build.
`scripts/check-code-traces` checks pigeon-core's functions (the roster
fold, admission, the authority rule, the catalogue merge) run in a simulated environment taking
the model's steps (20 runs in `ci`, 200 in
`models-full`; a mutant of each must be rejected), and
`scripts/check-lab-traces` the registered parts of the slice's runs (§13.12).
An unregistered specification or missing fields are reported as unchecked;
`--require SPEC` makes missing or partial coverage fail, even when another
run of that specification passed. The daemon's full `CaptureInstall` trace
is not yet registered, so its action names alone are not conformance
evidence. Checked observations show those runs conform; they are not a
proof of the code.

`scripts/check-storage-traces` is mandatory in the Rust CI job. Real Git/
git-annex tests observe live files, kept/recovered content, and retained
keys, and check those observations against `StorageTrace` with required
coverage. Every boundary has a corrupted observation that must be rejected;
missing logs, actions and fields must fail too. The independent retained-key
oracle unions the fixture's historical trees. These are targeted boundary
checks, not full `CaptureInstall` refinement **[no ADR]**.

## 13.4 Laws and property tests (ADR 0012)

The core is pure functions over values; git, git-annex, the file system and
iroh sit behind traits in a thin shell. Every core structure states its
laws, each a `hegeltest` property. The laws coded so far are those of
§13.13; these are required before or with their module:

| Structure | Laws still to code | Where |
|---|---|---|
| fetched history | a join-semilattice (associative, commutative, idempotent union) | [08 §8.2](08-custody-loops.md) |
| acceptance | `accepted(S ∪ T) = accepted(S) ∪ accepted(T)`; verdicts depend only on ancestors; a clean merge has Δ = ∅ | [04 §4.4](04-roster-and-authority.md) |
| custody observations | latest-observation join per (custodian, key); a custodian counts only with `has_commit` (ADR 0046) | [06 §6.1](06-status.md) |
| released keys | releasing one reference never releases a key another holds; an uncovered reference keeps its key; monotone in the seals; independent of arrival order; never a current version | [08 §8.6](08-custody-loops.md) |
| settings | expected git-annex settings are a function of the roster; applying twice changes nothing | [03 §3.5](03-repository-and-roles.md) |
| local files | round trip; newer formats refused | [11 §11.5](11-local-state.md) |
| compatibility | an older release's verdict is the newer one's or *unknown*; an unknown field or `format` is never ignored; an unreadable status report never raises a state (ADR 0048) | [04 §4.4](04-roster-and-authority.md), [06 §6.3](06-status.md) |
| append-only filter | a positive `CHECKPRESENT` only for a copy it could lock; to a custodian, a positive `LOCKCONTENT` or `CHECKPRESENT` only for a copy whose hash matched | [09 §9.4](09-network-and-joining.md) |
| capacity | the parts of the need sum to the total; every key not protected has a reason; admission never defers an endangered key for one that is not | [08 §8.9](08-custody-loops.md) |
| trees and names | every captured tree is well formed (rule 6); `package(p)` depends only on *p*'s name; merging a package never mixes two sides; `ignored` is a pure function of the name | [07 §7.9](07-capture-and-install.md), [05 §5.2](05-catalogue-and-names.md) |
| encodings | `parse(print(x)) = x` for roster TOML, NDJSON and git objects; our parser agrees with git | everywhere |

Example tests remain for regressions found by properties (with the shrunk
input) and for the fixed scenarios of this specification.

**Reference model of the catalogue**: `models/catalogue/catalogue.py` runs
every schedule of three devices for each case of [05 §5.2](05-catalogue-and-names.md)
and checks its laws there (`order`, `tree`, `kept`, `seen`, `names`,
`authority`) on each reachable state. The earlier rules (`old`, `naive`)
must fail; the core's laws must agree with it. Its `--dump DIR` writes
every commit it reaches; `pigeon-core/tests/catalogue_oracle.rs` judges
them again (about 140 000 over the eleven cases) and must agree on every
verdict, tree and open conflict.

## 13.5 Stateful tests and fault injection

Not yet coded beyond the slice (§13.12). hegel state-machine tests drive
real repositories of several devices through random operations (saves,
renames, mass deletions, decisions, invitations, roster changes, free and
keep, purges, Drop) and faults (`kill -9` of `pigeond` or a git-annex child,
disk full, an unplugged drive, partitions, clock skew, a custodian replaced,
a relay down), at least 1000 schedules per full run, against
[02 §2.5](02-guarantees.md) and trace validation.

## 13.6 Contract tests against git and git-annex

Every command pigeon runs ([03 §3.7](03-repository-and-roles.md)) is tested
against the minimum git (2.38) and git-annex (10.20261006) and the newest:
the output pigeon parses, the exit status, and the behaviour it relies on
(`crates/pigeon-git/tests/contract.rs`; CI makes them fail rather than
skip). A new git-annex is supported only once they pass
([14 §14.3](14-platforms-and-packaging.md)). A subset runs on each device
as a self-test when it first meets a new version (ADR 0048).
**Format fixtures** (ADR 0048): `fixtures/formats/` holds golden documents
and signed histories for every supported (`format`, `rules`) pair with
their verdicts, and next-format fixtures that must be judged *unknown*.
`crates/pigeond/tests/formats.rs` reads them on every CI run on both
runners: rosters, status reports and `clock.json` read as their directory
(`known`, `unknown`, `malformed`) says, and each signed commit of
`commits/` gets its verdict under its parent's roster. The cases are single
steps for now; histories of several commits, with merges and proposals, come
with the catalogue merge (M2). `fixtures/formats/README.md` states the rules:
a known file is never edited, and the generator only adds missing files.

## 13.7 The lab (`lab/git-annex/`)

The experiments E1–E25 test git-annex's own behaviour;
`lab/git-annex/README.md` lists each script, whether it asserts, what it
found and what rests on it. An E*n* cited in this specification is one of
them. Each joins CI once its script asserts (ADR 0026). Pending: E11 native
Windows (v1.1); new: E16 `annex.bwlimit` over P2P, E17 the HDD
video-editing test ([15 §15.8](15-resource-use.md)); E9 and E10 must be
rerun with bare custodian repositories.

## 13.8 Continuous integration (ADR 0026)

Each workflow of `.github/workflows/` says in its header what it runs:
`ci.yml` (the `scripts/check-*` rules, clippy, the Fluent check, `cargo
deny`), `lab.yml` and `slice.yml` (§13.12) on every push; `models-full.yml`
when a model changes, weekly and by hand, with E17 on a self-hosted HDD
runner **[no ADR: runner]**. A lab
platform difference prints `SKIP (platform)` with its reason, never a
dropped check. TLC is pinned by SHA-256, actions by commit; permissions are
read-only; fault-injection runs keep their OTLP-JSON and trace files
(ADR 0025).

## 13.9 Resource budgets (ADR 0037)

CI measures `pigeond` on a reference tree: idle CPU below 1 % of one core
over 10 minutes, bounded memory, and the budgets of E17
([15 §15.8](15-resource-use.md)). A regression fails the build.

## 13.10 Coverage map: from guarantees to evidence

What carries each guarantee today; gaps are GitHub issues
([16 §16.5](16-open-questions.md)). Evidence from pigeon's own code enters
"Now" only through the slice's ledger (§13.12).

| Guarantee | Now | Planned | Gaps |
|---|---|---|---|
| P1 Survival | E13, E21, E22, E23 (asserting); E1, E5, E8–E10 (printing); `AnnexDrop`, `Repair`, `Evict` | capacity laws; the filter's hash law; fault injection; asserting E9, E10 | independence pigeon cannot see (ADR 0046); power loss on a custodian |
| P2 Capture, no overwrite | E15b, E18, E19, E20, E24 (asserting); `CaptureInstall`, `Evict` | journal laws against the model (§13.13) | copy-on-write cost on APFS and btrfs; saves from stale memory or a macOS shared mapping (16 §16.4) |
| P3 Reversible deletion | E14 (asserting); E2, E4 (printing); `Purge` | purge laws; `Purge` trace validation | a custodian clock jumping beyond the waiting period; erased names stay in history (16 §16.4) |
| P4 Authority | E12 (asserting); E7 (printing); `Roster`, `Authority` | acceptance and revocation laws; `Authority` trace validation | format compatibility (#15) |
| P5 Convergence | E3 (printing); `models/catalogue`, `Catalogue` | merges by `pigeond`; the scenarios of two people and three laptops (M2); renames in `Catalogue` traces (ADR 0072) | renames with a large edit, or of files beyond 1 MiB, are a deletion and an add (ADR 0072) |
| P6 Honest status | E20, E21 (asserting) | observation laws; usability test | — |

## 13.11 Going on: technical and human criteria

**Go** for v1 if all hold (on the platforms of ADR 0005): no
loss of a protected version in any fault schedule or model; laptop disk use
at most 1.1 × the content kept; test users explain *only on this computer*,
*shared, not safe yet* and *safe* correctly. Otherwise the requirements are
reviewed before any engine is built. Windows criteria move to v1.1.

**Usability test** (protocol in ux.md §9), with the mockups, then the real
system. Pass: at least 4 of 5 explain the three states; at least 4 of 5 do tasks 4 and 5 (an old version, a deleted
folder) without help; **no** participant believes a file that is not safe
is safe. It tunes the thresholds of [06 §6.7](06-status.md).

## 13.12 The first milestone: the safety slice

The first code of v2 is a slice that carries the promises and nothing else
(issue #17): one person, its admin, with one laptop (Linux on ext4, or
macOS on APFS) and two Linux custodians on ext4, K = 2, one repository and
two folders.

| Part | In the slice | After it passes |
|---|---|---|
| crates ([14 §14.6](14-platforms-and-packaging.md)) | `pigeon-core`, `pigeon-proto`, `pigeon-i18n` (the English bundle), `pigeon-git`, `pigeon-sys`, `pigeond`, `pigeon`, `git-annex-p2p-iroh` | `pigeon-web`, `pigeon-telemetry`, the French bundle (ADR 0028) |
| loops ([08 §8.1](08-custody-loops.md)) | capture, accept, install with its journal ([07 §7.5](07-capture-and-install.md)), place, repair (started by a removed custodian), free and keep, settings check; the filter with its `CHECKPRESENT` probe ([09 §9.4](09-network-and-joining.md)) | scrub, purge |
| commands ([10](10-interfaces.md)) | `create`, `invite` (a person's laptop, or `--backup-machine`), `join`, `status`, `machines`, `history`, `restore`, `free`, `keep`, `remove-computer`, `recovery-sheet`, `recover`, `doctor`, `debug bundle` | the rest answer `not-in-this-build` |
| content | regular files and folders, the executable bit, portable names ([07 §7.9](07-capture-and-install.md)) | other file systems: `create` and `join` name the one they refuse (`file-system-not-in-this-build`, 10 §10.5) |
| network | iroh with direct addresses and mDNS | relays and discovery (ADR 0016) |
| diagnostics | JSON logs (ADR 0022), trace logs (ADR 0008) | telemetry transport and dashboards (ADR 0025) |

Also after it: more people and laptops (invitations, proposals, conflicts,
stewards and decisions), purge, erase and the scrub, notifications,
packaging and service units, adaptive limits (the slice keeps only low CPU
and I/O priority), large and hot files, and Windows. A deferred command
answers `not-in-this-build`, with no partial code. The deferrals concern
the order of work only: v1 still meets every accepted ADR. **[no ADR:
milestone]**

**The first build** (14 October 2026) has `pigeond`'s main, the loops of
the table, the commands, and an integration test of three daemons on one
host (`crates/pigeond/tests/slice.rs`). Until the slice passes it differs
from this specification as follows, each to close before S1–S11 run
**[no ADR: first build]**:

- `restore --at TIME` answers `not-in-this-build`; `recover` joins the new
  machine on `pigeon/join/1` through a backup machine of the sheet, dialled
  with the sheet's key, which writes and signs the roster commit once it
  has checked that the recovery device may add the machine (`may_sign`):
  the others accept it while the backup machine's person may add it (an
  admin in the slice), where ADR 0047 has the sheet key sign;
- every commit is signed with the device key, and a custodian moves `main`
  only as far as the authority rule accepts each commit against the
  catalogue of its accepted parents ([04 §4.4](04-roster-and-authority.md)),
  merging two accepted heads ([05 §5.2](05-catalogue-and-names.md)), roster
  forks by their fold ([04 §4.8](04-roster-and-authority.md)), and an
  adoption by rule 7; a laptop moves `main` the
  same way, then installs each clean path's new version through the journal
  ([07 §7.5](07-capture-and-install.md)); a save racing an install becomes
  a contribution concurrent with `main`'s version, merged into `main`, and
  the laptop whose version lost the name keeps its file, a `concurrent`
  divergence in `conflicts.json` that install leaves alone
  ([07 §7.6](07-capture-and-install.md)); a save that does not merge clean
  stays out of `main`, its content in the annex;
- a change its person may not make becomes a proposal, relayed by every
  device, its content placed like `main`'s, and a waiting `suggestion` on
  the laptop; a later save stacks a proposal on it
  ([04 §4.5](04-roster-and-authority.md), [07 §7.6](07-capture-and-install.md));
  `crates/pigeond/tests/suggestions.rs` runs it, with an admin accepting
  one suggestion, which reaches every device and ends the author's, and
  declining the next, a conflict on the author's laptop;
  `crates/pigeond/tests/resolve.rs` resolves divergences each way of
  [07 §7.6](07-capture-and-install.md), the first withdrawing a waiting
  proposal. *Keep mine*, *Keep Paul's*, *Keep both* and `pigeon view` are
  not in it, and a refused commit on a peer's `main` is not kept as a
  proposal ([08 §8.2](08-custody-loops.md) step 4);
- laptops join by invitation, of a new person or of a person of the
  roster ([09 §9.6](09-network-and-joining.md)), and keep every folder:
  choosing them at the join, and the safety words, are not in it;
  `crates/pigeond/tests/people.rs` runs two people, three laptops and two
  backup machines on one host, through a concurrent edit;
- place follows the queue order with admission
  ([08 §8.9](08-custody-loops.md)) but reads no introduction time; repair
  marks a removed device's UUID `dead`, after which place copies again;
- `heard` times and the trace clock are kept in `clock.json`
  ([11 §11.3](11-local-state.md)), but the clock travels in no stream
  header or join message yet ([12 §12.3](12-observability.md)); `history`
  takes older versions' states from their copy counts;
- the device key is hex, not OpenSSH ([11 §11.2](11-local-state.md)); an
  invitation is base32 of JSON, and also carries the group's name, K and the
  inviter's name;
- status reports are published, fetched, counted and relayed
  ([06 §6.3](06-status.md)), but a custodian neither refuses `tmpfs` nor
  stops serving once its repository is lost, and reports no proposals,
  scrub or purge;
- the creator's person ID is `$USER`, and a joining device is named by
  `join --name`, else `<host name>-<6 hex digits of its device ID>`;
- the CLI parses its arguments by hand, without completions or colour
  ([10 §10.2](10-interfaces.md));
- one round runs, per group, the settings check, free and keep toward each
  folder's target, capture, accept, install (laptops), place, re-thinning
  and repair, every
  `[loops] interval_seconds` and at once after a request or a peer's
  notice (remotedaemon's `CONNECTED` and `DONESYNCING`); `free` and `keep`
  only record the target; `status` on a laptop captures first;
- a peer is dialled at the direct addresses it last gave (an opener's in its
  stream header; the inviter's known peers' in a join's welcome; iroh still
  authenticates by key), and a device keeps
  the port it picked across restarts (`<state>/port`); a custodian that
  answers but serves the group no more (`unknown-group`) counts for nothing
  until it is removed.

**Setup** (`crates/pigeon-lab`; its module docs and `slice.yml`'s header
give the details). The lab build (`--features lab`) adds named crash points
(`PIGEON_LAB_CRASH=point:n`, as `kill -9`) and a root for a backup
machine's system directories, which a release build ignores; it says so in
`doctor`, and its status reports carry `lab = true`, which a release build
never counts. On `ubuntu-24.04` each custodian has its own mount namespace
and loop-mounted ext4 image, over 127.0.0.1 with direct addresses only; on
`macos-latest` each device's disk is a sparse APFS image (custodians on
macOS in the lab only, ADR 0032). These runners replace a run on real
hardware (Guillaume, chat, 14 October 2026). Network faults beyond loopback
are shown by no scenario; the ledger marks what rests on them
*conditional*.

**Scenarios** are typed programs of pigeon-lab's EDSL
(`crates/pigeon-lab/src/slice.rs`) that print, round-trip through JSON and
are generated and shrunk by hegel. Two interpreters fold the same steps:
the **model** over `pigeon-core`, where every scenario must pass, so its
expectations are what the specification predicts; and the **executor**,
which runs the lab build per device, records disk use and merges the trace
logs: each device's events must contain its part of the scenario in order,
and `scripts/check-lab-traces` hands them to TLC (§13.3).

Laws (`crates/pigeon-lab/tests/laws.rs`): steps form a monoid; validation
and the trace projection are compositional; a prefix runs the same whatever
follows; settling the model is idempotent; a crash at any named point, then
a restart, changes no outcome (1000 model schedules per push). The workflow
`slice.yml` runs them, then S1–S16 and 50 random S9 schedules against the
real binaries on both runners; `models-full.yml` runs 1000. A scenario that
does not pass yet against the first build is listed in `slice::BLOCKED`
with what its last runs showed; per push it is reported as blocked, and
`models-full.yml` runs it on both runners (`pigeon-lab run
--with-blocked`, with pigeond's debug log) without failing. A device's
disk is sized above the `annex.diskreserve` pigeon sets (03 §3.4): 4 GiB
for a laptop, 8 GiB (sparse) for a custodian; below it git-annex refuses
every transfer, which kept the first runs at *published*. In `slice.yml`
run 38660637841 (15 October 2026, at f5dddd6) S1–S13, every S9 point and
S9's 50 random schedules pass on both runners, and `slice::BLOCKED` is
empty. The random schedules run in a job of their own, two shards of 25
per runner, as 50 at about 10 s (Linux) to 60 s (macOS) each do not fit
beside the scenarios. Their last failure, one macOS case of 50, was a stale
`.git/index.lock` left by a stopped daemon's git; pigeond now removes stale
locks (11 §11.4).

| Scenario | Asserts |
|---|---|
| S1 | *local* → *published* → *protected*, each step from a recorded observation (ADR 0046); a symbolic link listed as not shared |
| S2 | `free` leaves a version that is not protected in place, then frees it without a pointer ([08 §8.5](08-custody-loops.md)) |
| S3 | `keep` and `restore` give back the same bytes (SHA-256) |
| S4 | an offline custodian: *published* only; *protected* without action when it returns |
| S5 | a 1 GiB transfer killed on either side resumes; no partial object counts |
| S6 | a save racing `keep`'s install wins ([07 §7.5](07-capture-and-install.md)) |
| S7 | a custodian's image destroyed: not safe; `remove-computer`, a new custodian, repair in [08 §8.9](08-custody-loops.md)'s order |
| S8 | a full custodian refuses admission and says why; a full laptop refuses a restore before it starts |
| S9 | `pigeond` crashed at each named point of capture, install, place and free: nothing lost or duplicated; 50 random schedules per push, 1000 in `models-full` |
| S10 | a fresh laptop recovers from the custodians with the recovery sheet (ADR 0047); the keys, metadata and authority it needed are listed |
| S11 | steady and peak disk use per device: folders, `.git`, annex objects, `.git/annex/tmp`, pending history ([07 §7.8](07-capture-and-install.md)), pigeon's state, temporary files |
| S12 | a nearly full custodian takes what fits in its room above the reserve, says `no-capacity` without being full, and so does the laptop from its status report ([06 §6.3](06-status.md)); it takes the rest once room returns ([08 §8.9](08-custody-loops.md), issue #13) |
| S13 | a commit signed outside the group, put on the laptop's `main` beside pigeond, is refused by both custodians with `commit-unknown-signer`, with every commit after it; what came before stays protected ([04 §4.4](04-roster-and-authority.md), ADR 0007) |
| S14 | two people, three laptops (M2): what each saves reaches every laptop; Bob changes his own file at once (ADR 0070), and his change of Ana's file is a suggestion, which he may not accept and she does ([04 §4.5](04-roster-and-authority.md)) |
| S15 | a declined suggestion stays its author's file alone until `pigeon resolve --take-group` ([07 §7.6](07-capture-and-install.md)); edits made apart on two laptops both stay, one under the name and one as its conflict copy, and neither laptop's file is overwritten (ADR 0034) |
| S16 | two laptops each admit a new person while they cannot reach each other: the roster forks, then every laptop lists every device and the newcomers' files reach everyone (ADR 0051) |

S14–S16 (`pigeon-lab/src/people.rs`) give laptops persons: a spare laptop
joins with `pigeon invite PERSON` and `pigeon join … --name`. The model
follows the default root rule (an admin or the file's only author changes
a file, anyone adds one, another change is a suggestion); conflict copies
are observed by the executor only, as `Catalogue.tla` and `Authority.tla`
model them. A person saves a file they received only after 12 seconds,
since a save sooner after an install goes beside it (ADR 0071), which the
model does not follow; `pigeond/tests/install.rs` checks that rule.

S7's repair order and S10's list of what recovery needed are read from the
run's trace and report, not yet asserted by a condition.

**Laws first.** The parts of `pigeon-core` that the slice uses have their
hegel-rust laws before their drivers: states and headline, the install
journal, the queue order and the roster fold.

**Passing and the ledger.** The slice passes when every scenario asserts on
both runners, no protected version is lost in any
schedule, and laptop disk use in the steady state is at most 1.1 × the
content kept (§13.11). The peak is reported with its causes.
`docs/reports/slice-1.md` marks each promise (P1–P6) and each ADR it
exercises as *established* (with its scenarios and traces), *conditional*
(with the assumption it rests on) or *untested*. Evidence from pigeon's own
code enters the "now" column of §13.10 only through this ledger. Issue #17
closes when the ledger is published with the slice passing.

## 13.13 The laws in code (ADR 0012)

`pigeon-core` states each law in the documentation of its module and checks
it with a `hegeltest` property in `crates/pigeon-core/tests/` (at least 1000
cases each; a law that needs a rare input gets a targeted generator and an
example test). These laws, and the model invariants they mirror, are
normative; the laws of §13.4 wait for their module. Other crates state laws
the same way: `pigeond/tests/codecs.rs` and `net_laws.rs`,
`pigeon-proto/tests/proto_laws.rs`, and `pigeon-lab/tests/laws.rs` (§13.12).

| Law | Module | Test | Model mirrored |
|---|---|---|---|
| more presence observations never lower a state; a second entry for one custodian changes nothing; custodian order is irrelevant; protected exactly when K distinct custodians count (ADRs 0002, 0046); a new holder never lowers a state | `state` | `tests/state.rs` | — |
| the join is a semilattice with *Safe* as identity and *Needs you* absorbing; the headline of a union is the join; adding an item never lowers it; a version only this computer holds is *Not safe yet* after the grace period, even unreachable | `headline` | `tests/headline.rs` | — |
| a signed commit verifies to its signer; changing any byte of it never does; signing then cutting the signature off gives the commit back | `sig` | `tests/sig.rs` | — |
| a commit signed by a device of R(p) that changes only what its person may is accepted; one unsigned, forged or signed outside R(p) is refused; a path's change is allowed exactly as §4.3's tables say; a roster change is accepted iff `may_sign` allows it and it applies whole; a proposal's deciders are those allowed every change of it, else the admins with Force, and exactly their devices may sign its adoption; capture's split of a batch loses nothing, its commit on `main` is accepted and each proposal is refused for folder rules alone; a new decision is accepted iff a decider declines or the author withdraws, a changed one never; an adoption holds its proposal's change, keeps what `main` changed meanwhile, adds only conflict copies, and is the proposal on an unchanged `main` (`tests/proposal.rs`); `diff` is a right inverse of `apply` and says every change of one edit; a roster of a newer `format`, or with a key, role or folder rule this release does not know, is *unknown*, never refused, and so is every commit under `rules` it does not support (ADR 0048) | `authority` | `tests/authority.rs`; `pigeond/tests/codecs.rs`, and `pigeond/tests/formats.rs` on the fixtures of §13.6; `tests/authority_trace.rs` | `Authority` (`AuthorisedContent`, `DecisionsByDeciders`); each verdict of `commit::judge`, on commits, decisions and adoptions, is `Judge`'s (`AuthorityTrace`) |
| the latest report of each device is a semilattice; the order of arrival does not matter; an older report never replaces a newer; only a report signed by the roster device it names, known and not from the future, counts; relaying what counted changes nothing | `report` | `tests/report.rs` | — |
| the queue order is total; a higher class goes first; arrival order is irrelevant; a key on fewer custodians never waits behind one on more | `queue` | `tests/queue.rs` | — |
| admitted bytes never exceed the room; an endangered key is never deferred for one that is not; the plan does not depend on arrival and keeps queue order, each item once; a deferred item says the bytes it misses, and that much more room admits everything; the reserve is a floor or a share; the parts of the need sum to K times each distinct key; capacity counts at most one copy per custodian | `capacity` | `tests/capacity.rs`, `tests/admit_trace.rs` | `Admission` (`ReserveKept`, `EndangeredFirst`) |
| recovery ends every entry and a second does nothing; a crash during recovery ends as one recovery; a person's save is never lost; a deletion after install stays; without a person, the path ends old or new and the index agrees (INV-Journal); pigeon never renames over a name | `journal` | `tests/journal.rs` | `CaptureInstall` (`NoLoss`, `Journal`, `NoOverwrite`, `DeletionKept`) |
| every tree is the fold of its history; the fold depends only on the set of changes; every fold is valid and removals win, even of a device not yet added; skips need concurrency; a linear history skips nothing; the canonical order is topological; pigeon-core lets a device sign every change of the model's `Ops` | `roster` | `tests/roster.rs` | `Roster` (`TreeIsFold`, `Agreement`, `AdminRemains`, `RemovalWins`, `SkipsNeedConcurrency`), and `RosterTrace` on runs |
| allowed streams pass unchanged; chunking does not matter; custodians never forward removals or pushes; malformed input closes; a custodian answers only after the sync; an unattributed success closes; `AUTH` is rewritten and checked | `filter` | `tests/filter_laws.rs` | — |
| the merge's laws of 05 §5.2 (`order`, `tree`, `kept`, `seen`, `names`, `algebra`, `authority`) on random schedules of three devices; judging the same commits in another order gives the same verdicts, catalogues and open conflicts; every commit the oracle reaches gets the oracle's verdict, tree and open conflicts | `catalogue` | `tests/catalogue.rs`, `tests/catalogue_oracle.rs`, `tests/catalogue_trace.rs` | `models/catalogue` (mode `new`); each commit is `Record`'s, with `View`'s catalogue (`CatalogueTrace`) |
| folding is idempotent; every conflict name of a portable path is portable and recognised as its copy; a suggested name is portable and differs from every sibling; names that do not collide never change; the cases of 05 §5.4 | `names` | `tests/names.rs` | — |
| what goes and what is held split a round's deletions; nothing held goes unless released; what goes unasked stays within the limit and touches no removed top folder; a larger round is held whenever a smaller one is (07 §7.4) | `held` | `tests/held.rs` | — |
| a manifest reads back as written; content matches a key exactly when its size and SHA-256 are the key's; a safe path stays below the export directory; a pointer gives back its key; a free name is never a tree path nor its folder (09 §9.9) | `export` | `tests/export.rs` | — |
| a trace line parses back to its event; the Lamport clock orders a receive after its send; every action name is an operator of its `.tla` | `trace` | `tests/trace.rs`, `tests/roster_trace.rs` | every `*Trace.tla` |
