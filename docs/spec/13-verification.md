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
| `AnnexDrop_design` | `AnnexDrop` | `pass` (fast set) | git-annex's verified drop as pigeon uses it (ADR 0046, [09 §9.4](09-network-and-joining.md)): each custodian drops only its own copy, under its removal lock, once it has locked, or seen present by a check that honours removal locks, K copies in K failure domains among those its location log lists; location logs go stale, partitions cut a drop off, a lock expires under a stalled drop, a custodian is marked `dead`, a new copy is made. The key stays in K domains (INV_K) and a locked copy stays (LockedStays); 3 custodians, K = 2: 406 880 distinct states, 11 s on 1 worker |
| `AnnexDrop_disk` | `AnnexDrop` | `pass` (fast set) | the same with custodians 1 and 2 on one disk: the drop counts domains, not repositories (E22.7) |
| `AnnexDrop_ranked` | `AnnexDrop` | `pass` (fast set) | with git-annex's P2P check, which ignores removal locks, a drop that counts only custodians ranked above its own is safe: the alternative [09 §9.4](09-network-and-joining.md) rejects, for liveness |
| `AnnexDrop_design4`, `AnnexDrop_gets2`, `AnnexDrop_ranked4` | `AnnexDrop` | `pass` (full set) | the design with 4 custodians, without expiry or new copies (4 518 880 distinct states, 4.5 minutes on 1 worker); with two new copies (1 259 520, 35 s); the ranked drop with 4 custodians (970 240, 44 s) |
| `AnnexDrop_p2p` | `AnnexDrop` | `violation:INV_K` | git-annex over its P2P protocol: `CHECKPRESENT` ignores the removal lock, so two custodians that drop at once each count the other's copy (E23) |
| `AnnexDrop_log` | `AnnexDrop` | `violation:INV_K` | counting the location log without checking: a stale log drops the last copies (E5) |
| `AnnexDrop_storage` | `AnnexDrop` | `violation:INV_K` | counting repositories, as git-annex does: two on one disk count twice (E22.2) |
| `AnnexDrop_trusted` | `AnnexDrop` | `violation:INV_K` | one custodian marked `trust`: the others count it unasked from a stale log (E23) |
| `Repair_design` | `Repair` | `pass` (fast set) | placement, repair and rebalancing on custodians (08 §8.3–8.4, §8.8–8.10; ADR 0046, [08 §8.9](08-custody-loops.md), [09 §9.4](09-network-and-joining.md)): custodians 1 and 2 hold key *a*; the joiner 3, with room for one key, comes in empty or on a restored disk; the laptop holds key *b*. Faults (a lost disk, a rotten copy, a removed custodian) stay within F, plus a crash or a partition. A key once protected keeps an intact copy on disk or on the laptop (Survival); no disk crosses its reserve (ReserveKept); no download takes the room of an endangered key (EndangeredFirst); a copy leaves only with its disk or its custodian, by a scrub, by a crash before its sync, or by a drop of an unwanted key backed by K lockable copies in K domains (INV_K, a property of steps). 355 939 distinct states, 27 s on 1 worker |
| `Repair_disk` | `Repair` | `pass` (fast set) | the same with the joiner on custodian 2's disk, one failure domain for both (ADR 0046): 303 067 distinct states |
| `Repair_live` | `Repair` | `pass` (fast set) | liveness: under fairness (transfers, syncs, scrubs and heals run; admins remove lost custodians and add the joiner) and with room for K copies of everything, an endangered key gets K domains back (Restores): 19 364 distinct states, 22 s |
| `Repair_live4` | `Repair` | `pass` (fast set) | four custodians, one key: the joiner is at its reserve yet first in sizebalanced's order, and domain repair chooses custodian 4 |
| `Repair_design3`, `Repair_design4`, `Repair_live2` | `Repair` | `pass` (full set) | three faults and two crashes or partitions (2 990 176 distinct states, 3.5 minutes); four custodians, two of them joining (515 705, 43 s); liveness with two faults (147 880, 2 minutes) |
| `Repair_lockonly` | `Repair` | `violation:Survival` | a lock that proves presence only ([09 §9.4](09-network-and-joining.md) alone): custodian 1's copy rots, custodian 2 drops *a* for the joiner and locks 1 and 3, then 3's disk dies. One fault in the epoch, and no intact copy is left ([09 §9.4](09-network-and-joining.md)) |
| `Repair_nosync` | `Repair` | `violation:Survival` | copies counted before their sync (ADR 0046): the laptop drops *b* while both custody copies are still only in memory |
| `Repair_nofsck` | `Repair` | `violation:Survival` | a restored disk counted before its fsck (09 §9.9): the laptop's drop leans on its bad copy, and the only other copy had rotted |
| `Repair_noprio` | `Repair` | `violation:EndangeredFirst` | admission without the endangered-first rule ([08 §8.9](08-custody-loops.md)): the joiner starts *b* with the room that *a* needs |
| `Repair_inflight` | `Repair` | `violation:ReserveKept` | a reserve check that ignores downloads in progress, which git-annex counts (`sizeOfDownloadsInProgress` in `checkDiskSpace'`, `Annex/Content/LowLevel.hs`): two downloads cross the reserve |
| `Repair_wantall` | `Repair` | `violation:temporal` | domain repair counting a member at its reserve as taking the key (08 §8.3 before ADR 0046 counted only members that can take the key): no other custodian is chosen, and the key stays endangered |
| `Evict_free` | `Evict` | `pass` (fast set) | issue #10, [08 §8.5](08-custody-loops.md), [07 §7.8](07-capture-and-install.md): one laptop, folders F = {a, b} and G = {c}, where c has a's key; F switched from kept to freed while the person saves (in place through a handle opened earlier, or by atomic replace), custodians lose a copy, status reports lag and `pigeond` crashes. No save or captured version is lost (NoLoss); no file is ever a pointer (NoPointer); a key is dropped only while K custodians hold it (DropsProtected); capture commits only what the person wrote (CaptureOnlySaves); no path is captured as deleted (NoDeletion): 52 944 distinct states, 5 s on 1 worker |
| `Evict_keep` | `Evict` | `pass` (fast set) | F switched from freed to kept while the group changes F's files (the index of hidden paths moves during the keep): 62 744 distinct states, 4 s |
| `Evict_live`, `Evict_livekeep` | `Evict` | `pass` (fast set) | liveness under fairness: a folder that stays freed ends hidden, nothing aside or staged, its keys gone unless G uses them (FreeLive); a folder that stays kept ends with every file present (KeepLive): 52 944 and 46 596 distinct states, about 20 s each |
| `Evict_flip`, `Evict_live2`, `Evict_livekeep2` | `Evict` | `pass` (full set) | F switched kept → freed → kept with a group change, a lost copy and a crash (10 612 600 distinct states, 9.5 minutes on 1 worker); FreeLive and KeepLive with two switches (466 952 distinct states, 3 minutes) and with a group change during a keep (1 136 988, 11 minutes) |
| `Evict_git` | `Evict` | `violation:NoLoss` | hiding with git's own unlink (08 §8.5 before [08 §8.5](08-custody-loops.md) hid a file as an install): an editor holds b open, the hide unlinks it, and the next write lands in no file (E24) |
| `Evict_guard` | `Evict` | `violation:NoPointer` | a free that drops a's key, which G's c also has: git-annex turns c into a pointer file (E24) |
| `Evict_target` | `Evict` | `violation:NoPointer` | the guard counting visible paths only: F is freed during a keep, after its staged files moved in with their bits set, and the drop turns them into pointers (E24) |
| `Evict_prune` | `Evict` | `violation:NoPointer` | the drop of old versions ([07 §7.8](07-capture-and-install.md)) without the guard: c's old version is a's current one |
| `Evict_lock` | `Evict` | `violation:DropsProtected` | a drop that trusts the status report instead of locking K copies when it runs: a custodian lost the key since |
| `Evict_stage` | `Evict` | `violation:NoPointer` | ADR 0031's keep order: showing with a checkout writes pointers for content not yet fetched (E15b) |
| `Evict_stale` | `Evict` | `violation:CaptureOnlySaves` | a keep that clears the bit of a moved-in file without comparing it with the index: main moved since the staging, and capture commits the old version as the person's save |
| `Evict_copy` | `Evict` | `violation:NoLoss` | capture with `annex.thin=true`: the object is the file's inode, and an in-place save destroys the captured version before any custodian has it ([07 §7.8](07-capture-and-install.md), E18) |
| `Evict_journal` | `Evict` | `violation:NoDeletion` | capture that does not skip a path whose file is aside: the hide's window is captured as a deletion |
| `Evict_lockfile` | `Evict` | `violation:temporal` | a stale `info/sparse-checkout.lock` kept after a crash: the hide never completes, and FreeLive fails (E15b) |
| `Purge_full` | `Purge` | `pass` (fast set) | [08 §8.6](08-custody-loops.md): a restore or cancel that reaches any custodian before its seal keeps the content (KeepWins); nothing is dropped before the waiting period (WaitingPeriod); a dropped copy returns only for an uncovered reference (NoResurrection); 2 custodians (one holds the key), 2 laptops (one offline with a copy), restarts, any message order |
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
| `Authority_decide` | `Authority` | `pass` (fast set) | issue #10: suggestions made, revised, adopted, rejected and withdrawn between the honest admin a and owner b, who also moves b's folder. Every decider can adopt; every decision is a decider's or the author's; every version entered through an allowed commit; an honest tree is the catalogue of its history; quiet devices agree (ADR 0051): 34 245 states, 20 829 distinct, 7 s |
| `Authority_faulty` | `Authority` | `pass` (fast set) | the same against a faulty member who signs any change, merge or decision on any parent: 370 794 states, 150 479 distinct, 55 s |
| `Authority_revoke` | `Authority` | `pass` (fast set) | c's laptop, once removed, signs on old parents and adds a key for c. Nothing it signed unseen, and no key it added, counts on any device (ADR 0051): 547 210 states, 188 525 distinct, 66 s |
| `Authority_live` | `Authority` | `pass` (fast set) | `Converge` under fairness of fetches and merges, with the removed laptop signing on old parents: 938 states, 399 distinct, 11 s |
| `Authority_decide4`, `Authority_faulty2`, `Authority_revoke2` | `Authority` | `pass` (full set) | four changes; two faulty commits, with transfers; saves and the removed laptop's merges: 1 178 047, 1 360 807 and 1 645 961 distinct states, 10, 7 and 6 minutes |
| `Authority_proposal` | `Authority` | `violation:DecidersCanAdopt` | deciders under the roster at the proposal's parents (ADR 0051's words). a suggests a change in b's folder, then gives the folder to himself. b still decides, but rule 3 rejects b's adoption (3 states) |
| `Authority_mixed` | `Authority` | `violation:DecidersCanAdopt` | the proposal's own diff under today's roster. a adds the root file; c suggests adding it and a file in F. b decides, but adopting keeps both versions of the root file, which modifies it, and only the admins may |
| `Authority_noforce` | `Authority` | `violation:DecidersCanAdopt` | a suggestion that nobody may make falls to the admins, whose adoption folder rules reject without Force |
| `Authority_fakeadopt` | `Authority` | `violation:AdoptionHoldsContent` | without rule 7, c's clean merge of `main` with a's suggestion marks it accepted with none of its content |
| `Authority_base` | `Authority` | `violation:AuthorisedContent` | proposals counted in a merge's base, without rule 7: a suggestion's content enters with no decider |
| `Authority_first` | `Authority` | `violation:AuthorisedContent` | rule 3 checked against the first accepted parent only: c's merge changes F, which only one parent's roster lets c change |
| `Authority_anyone` | `Authority` | `violation:DecisionsByDeciders` | any member may write a decision: c rejects a's suggestion, which only b may decide |
| `Authority_none` | `Authority` | `violation:RemovedSilenced` | no revocation (04 §4.4 and ADR 0051 alone). After a removes c's laptop, the laptop changes the root file on the genesis, and a's device merges it (5 states) |
| `Authority_tip` | `Authority` | `violation:RemovedSilenced` | 09 §9.7's policy, no merge of a head signed outside `main`'s roster. The removed laptop signs the merge of its change with the removal itself, and b's device, which has not yet seen the removal, fast-forwards to it (6 states) |
| `Authority_tipquiet`, `Authority_tiplive` | `Authority` | `violation:ConvergedWhenQuiet`, `violation:temporal` | the same policy leaves an accepted commit out of every `main` for good |
| `Authority_contentonly` | `Authority` | `violation:RemovalRevokesAdded` | revoking the content but not the keys: the key that the removed laptop added for c stays in the roster |
| `TraceSample_good` | `TraceSample` | `trace:accepted` (fast set) | the pattern of every trace module (§13.3) on a toy save-then-capture spec: a log whose values carry quotes, backslashes, nested tuples and records is explained line by line, and its `ASSUME`s check that TLC reads back what the NDJSON held |
| `TraceSample_bad` | `TraceSample` | `trace:rejected:2` | a capture of a path never saved: no behaviour explains line 2, and the verdict names it |
| `RosterTrace_good` | `RosterTrace` | `trace:accepted` (fast set) | a run of pigeon-core's roster fold, written by `crates/pigeon-core/tests/roster_trace.rs` (seed 1): each logged change is one of `Roster`'s `Ops`, and each roster the code computed, at commits and merges, is the model's (§13.3) |
| `RosterTrace_bad` | `RosterTrace` | `trace:rejected:7` | the same run with the roster of its last checked step altered: the verdict names line 7 |
| `CaptureInstall_own` | `CaptureInstall` | `pass` (fast set) | issue #1, [07 §7.5](07-capture-and-install.md): capture and install of one path while a person writes in place, writes through a handle opened earlier, replaces atomically, creates and deletes at any instant, also while `pigeond` is down; the group changes or deletes the path; one crash. Nothing saved is lost (NoLoss); when idle, the path holds what the index says or the person's content (Journal); the index never calls a changed file clean (IndexHonest); what install moves away was captured (CaptureFirst), is the file it expected, and is written by nobody afterwards (NoOverwrite); a deletion the person made stays (DeletionKept); under fairness every save is captured (SavesCaptured) and the path ends at `current(p)` with no divergence and no kept aside (Converges). Own folder, two person actions: 122 560 states, 61 698 distinct, 9 s on 1 worker |
| `CaptureInstall_other` | `CaptureInstall` | `pass` (fast set) | the same in another person's folder: saves become suggestions, which are accepted or rejected, then resolved (*Take the group's version*, *Suggest again*), also while new saves race the choice: 1 240 787 states, 612 281 distinct, 6 s |
| `CaptureInstall_crash2` | `CaptureInstall` | `pass` (fast set) | two crashes, one during recovery: a second replay decides as the first (ADR 0036's law): 360 940 states, 171 214 distinct, 4 s |
| `CaptureInstall_hidden` | `CaptureInstall` | `pass` (fast set) | writers that escape the writer scan and close within the keep period: nothing is lost, though NoOverwrite does not hold for them ([07 §7.5](07-capture-and-install.md)) |
| `CaptureInstall_own3`, `CaptureInstall_other3`, `CaptureInstall_otherlive` | `CaptureInstall` | `pass` (full set) | three person actions in the own folder (1 048 763 distinct states, 9 s); three in another's folder with escaping writers (21 968 788 distinct states, 3 minutes); liveness in another's folder (612 281 distinct states, 72 s) |
| `CaptureInstall_rec07` | `CaptureInstall` | `violation:DeletionKept` | the recovery table of 07 before [07 §7.5](07-capture-and-install.md) journalled the proof, which decides from *p* alone: the person deletes the new file after step 7, `pigeond` is killed before step 8, and recovery puts the old file back |
| `CaptureInstall_rec07del` | `CaptureInstall` | `violation:Journal` | the same table: a deletion killed after step 8 gets its old file back while the index says deleted, so capture would add it again |
| `CaptureInstall_nocheck` | `CaptureInstall` | `violation:CaptureFirst` | without the check of step 6, a save made after the clean test is moved away before capture has it |
| `CaptureInstall_noscan` | `CaptureInstall` | `violation:NoOverwrite` | without the writer scan, a write through a handle opened earlier lands in the proved aside |
| `CaptureInstall_racy` | `CaptureInstall` | `violation:IndexHonest` | trusting a racily clean index entry: a save in the same timestamp tick keeps the stat, and the path counts as clean |
| `CaptureInstall_nojournal` | `CaptureInstall` | `violation:Journal` | without the journal, a crash between steps 4 and 7 leaves the path absent, and capture would delete it for everyone |
| `CaptureInstall_noreplace` | `CaptureInstall` | `violation:NoLoss` | step 7 with a plain `rename`: a file an application created during the gap is replaced and lost |
| `CaptureInstall_nohash` | `CaptureInstall` | `violation:NoLoss` | a kept aside released without hashing: a racing save kept there is unlinked |
| `CaptureInstall_quiet` | `CaptureInstall` | `violation:CaptureFirst` | capture of a file written in the current tick: its `mine_stat` is racy, and *Take the group's version* moves away a save of the same tick |
| `CaptureInstall_keep` | `CaptureInstall` | `violation:NoLoss` | a writer that escapes the scan and writes after the release: the write goes to a file with no name |
| `CaptureInstall_escape` | `CaptureInstall` | `violation:NoOverwrite` | a writer that escapes the scan can write into a proved aside; `CaptureInstall_hidden` shows that the write is still kept |

The violations and the deadlock are negative tests: a model edit that hides
the failure is caught (ADR 0026). `CustodyNaive` and `CustodyTokens` argue
why custody hand-off needs locking and fresh identifiers, and why naive designs
fail. In v2 the mechanism is git-annex's lock-based verified drop (E1, E5,
E23), not pigeon's own tokens; `AnnexDrop` states what pigeon relies on from
it, and its header cites the git-annex 10.20261006 source of each action.

**How the `AnnexDrop` traces match the lab.** `Check = "safe"` is a drop over
a local path, which checks presence with `inAnnexSafe` (`Remote/Git.hs`), as
in E1: its 40 runs never lost the last copy, and the design passes.
`Check = "p2p"` is a drop over git-annex's P2P protocol, pigeon's transport
([09 §9.4](09-network-and-joining.md)). There the server answers `CHECKPRESENT` with `inAnnex`
(`P2P/Annex.hs`), which ignores the removal lock, and a drop whose
`LOCKCONTENT` fails counts that answer (`Annex/NumCopies.hs`). The 8-step
counterexample of `AnnexDrop_p2p` (two custodians begin, each `LOCKCONTENT`
meets the other's removal lock, each `CHECKPRESENT` says present, both remove)
is E23's race with real git-annex: with numcopies 1 no copy is left, with
numcopies = mincopies = 2 on three custodians one is. Behind pigeon's probe
([09 §9.4](09-network-and-joining.md)) the check is `"safe"` again, and E23's same races keep every copy.
`AnnexDrop_log` is E5: counting a stale log drops the last copies, while the
design checks each copy, as git-annex does (E5, and E23's stale log).
`AnnexDrop_trusted` is E23's `trust`: a trusted custodian counts unasked from
the log, so pigeon never sets `trust` ([03 §3.5](03-repository-and-roles.md)).
`AnnexDrop_storage` is E22.2 and `AnnexDrop_disk` E22.7: git-annex counts two
repositories on one disk twice, and pigeon's drop, which ignores all remotes
but one per domain, refuses until another domain holds the key. Laptops are
not modelled: they are untrusted and never counted, and their drops leave the
custody copies alone. Nor are remote drops (pigeon's filter refuses `REMOVE`,
E13), or the forced drops of purge and Drop, which skip the check (purge has
the `Purge` model).

**`CaptureInstall`** ([07 §7.7](07-capture-and-install.md)) models one path on
one laptop. Every write makes a new content, so a content names the save that
made it. A file's stat is its inode and mtime, and a write in the same clock
tick keeps the mtime, as in racy git. Install's steps 1 to 3 are one action;
steps 4 to 9, `Crash`, `Recover` (forward or back, [07 §7.5](07-capture-and-install.md)) and its end are
one action each. Capture takes a quiet path, unchanged since the last tick.
The person acts at any instant, pigeond being up or down; the group
(`NewVersion`, `Decide`) changes `current(p)` and judges suggestions.
Each negative configuration removes one safeguard (`Off`) and must fail. The
model found that the recovery table, which decided from *p* alone, undoes a
finished deletion and a deletion the person made after step 7; [07 §7.5](07-capture-and-install.md)
journals the proof and rolls proved entries forward. Its first repair, which
decided from the staged file, failed the double replay of
`CaptureInstall_crash2`. Not modelled: several paths and packages, thin hard
links and their link count, the resolution of a recovered version (it is
captured; the model checks only that it is kept), and the scan's retry period
(a deferred path is retried once its writer has closed).

**`Repair`** ([08 §8.4](08-custody-loops.md)) takes keys of one unit each.
Each custodian has room above its reserve and a failure domain, and the
order in which `sizebalanced` and domain repair rank it. A get copies only
an intact object, since git-annex verifies it on arrival. A get counts as a
copy only once synced (ADR 0046). A verified drop is one step: `AnnexDrop`
shows the drop itself is safe behind the probe of [09 §9.4](09-network-and-joining.md). Assumption F
(02 §2.2) bounds the faults: within a repair epoch fewer than K domains lose
or corrupt a key. The model counts the epoch on intact copies, so a rot opens
it when it happens. Each negative configuration removes one safeguard
(`Off`) and must fail. The model found two flaws, fixed by ADR 0046, [09 §9.4](09-network-and-joining.md). A
rotten copy can prove a custodian's drop, since git-annex's lock checks only
presence. A custodian at its reserve still wants its share, so domain
repair, counting it as taking the key, never chose another domain. Not
modelled: keys of different sizes; `maxsize` apart from real room; stale
status reports ([08 §8.9](08-custody-loops.md) shows that they cost space, never safety); the
seven days before an admin removes a silent custodian; `min_sites`.

**`Evict`** ([08 §8.5](08-custody-loops.md)) models one laptop with two
folders: F = {a, b}, which the person switches between kept and freed, and
G = {c}, always kept, where c has a's key. A content is its key. Files,
asides, staged copies and annex objects are inodes, so a write through a
handle changes its inode, wherever it is linked, or nowhere. The person
saves (in place through a handle opened earlier, or by atomic replace),
opens and closes files at any instant, `pigeond` being up or down; the
group changes F's files; a custodian loses a copy; status reports lag; a
crash may leave `info/sparse-checkout.lock`. From git-annex 10.20261006 the
model takes the drop: the object goes, and every unmodified file of the key
becomes a pointer (`removeAnnex` in `Annex/Content.hs`); it is verified
when it runs ([09 §9.4](09-network-and-joining.md)). From git it takes that `git sparse-checkout set`
unlinks a hidden file that matches the index, and that git clears the bit
of a file present on disk, so capture sees a file made in a hidden folder.
Each negative configuration removes one safeguard (`Off`) and must fail. The
model and E24 found that hiding with git's unlink loses a write through an
earlier handle, that a drop's guard must count staged and moved-in files and
cover the drop of old versions, and that a keep can show a version main has
replaced; [08 §8.5](08-custody-loops.md) fixes all three. Not modelled: install's own steps (the
`CaptureInstall` model), packages, divergences and conflicts, several
custodians per domain (`AnnexDrop`, `Repair`), and a writer that escapes the
scan.

**`Authority`** ([04 §4.3–4.5](04-roster-and-authority.md),
[09 §9.7](09-network-and-joining.md)) models acceptance with three persons:
the admin a, b who owns a folder F, and the member c. There are two paths: a
file in F, and a file at the root (add-only, the admins are its stewards).
The devices of a and b are honest `pigeond`s: they capture, revise, adopt,
reject and withdraw suggestions, give F to someone else, remove c's devices,
fetch and merge. c's devices belong to an attacker who signs any change,
merge or decision on any parent it knows, and may add a key for c. That is a
faulty member, or c's stolen laptop once a has removed it.

The verdict of each commit is computed by the rules of 04 §4.4 (1–4 and 7)
when the commit is written. A path's value is the set of versions that the
newest changes of that path wrote: a conflict keeps several. This is the
abstraction of the catalogue that `models/catalogue` checks in full
([05 §5.2](05-catalogue-and-names.md)). Increasing commit ids give the canonical order; `Roster` checks
the fold's order and its edits.

The model found two things:

- **A removed device keeps signing.** On an old parent, the removed laptop's
  commits are valid. 09 §9.7's policy of not merging them lets them in
  through a device that has not yet seen the removal (`Authority_tip`). It
  also keeps accepted commits out of `main` (`Authority_tipquiet`). ADR 0051
  revokes them instead.
- **Deciders who cannot decide.** ADR 0051 judges deciders on the
  proposal's parents, or on its own diff, so some deciders cannot adopt
  (`Authority_proposal`, `Authority_mixed`). Admins cannot adopt without
  Force (`Authority_noforce`). And a merge that adopts nothing marks a
  proposal accepted (`Authority_fakeadopt`). ADR 0051 judges deciders at the
  decision and adds rule 7.

Convergence is checked in two ways. `ConvergedWhenQuiet` is a safety
property: every behaviour is finite, so under fairness each one ends where
no honest device can fetch or merge, and there the trees must be the
catalogue of all known commits. It holds in every design configuration.
`Converge` is the temporal property; TLC checks it only in the small
`Authority_live`, since fairness over these actions is slow to check.

Not modelled: several devices per person, persons joining or leaving,
key rotation, rules 5 and 6 (names, well-formed trees), Drop, and the
contents of a conflict copy.

### Planned, before or with the code (ADR 0008)

None remain: `Authority`, `Evict` and `Repair`, the last three planned for
issue #10, are in the table above. Bounds start at N ≤ 4 devices, K ≤ 2, a few paths and keys, as now
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
- **Reading the log**. `scripts/trace-to-tla` writes the log as a
  TLA+ module that defines it as a tuple of records, and the trace module
  extends it; the pinned TLC 1.7.4 reads it (ADR 0026). Values it cannot
  encode exactly (`null`, fractions, control or non-ASCII characters, keys
  that are not identifiers) and lines out of Lamport order are refused, never
  approximated. The CommunityModules' `ndJsonDeserialize` needs TLC 1.8,
  which has only a rolling nightly; the choice is revisited when 1.8 is
  released.
- **Verdict.** A trace configuration checks the invariant `TraceNotConsumed`
  (`l <= Len(log)`), violated exactly when a behaviour explains every line:
  `scripts/run-tlc` reports `trace:accepted`. Otherwise TLC finds no error,
  and since every step consumes one line, the depth of its search is the
  first line that no behaviour explains: `trace:rejected:N`, which fails the
  build with that line. `TraceSample` checks the pipeline in CI (13 §13.2).
- **Runs of the code**. Before the drivers exist, a test runs
  pigeon-core's own functions inside a simulated environment that takes the
  model's steps (`crates/pigeon-core/tests/common/roster_sim.rs` for
  `Roster`), logs each step with what the code computed, and
  `scripts/check-code-traces` hands every log to TLC: 20 random runs in `ci`,
  200 in `models-full`. A mutant of each run, one logged roster altered,
  must be rejected, so a check that accepts everything fails. A mutation of
  the canonical order (signer before time) passes every hegel law of the
  fold and is caught here.
- Every fault-injection run in CI is trace-validated; a mismatch fails the
  build and reports the first log line no behaviour of the model explains.
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
| catalogue merge | a function of the contributions and of which saw which, never of contact order, duplicate deliveries or merge commits; commutative, associative, idempotent; `tree(c) = catalogue(anc*(c))`; a key leaves only through a contribution whose base showed it; a superseded version never comes back; conflict names independent of arrival order and injective after folding; each copy allowed to its author ([05 §5.2](05-catalogue-and-names.md)) | [05 §5.1–5.3](05-catalogue-and-names.md), `models/catalogue` |
| portable names | normalisation idempotent; injective within a folder; portable names unchanged | [05 §5.4](05-catalogue-and-names.md) |
| custody observations | latest-observation join per (custodian, key); *protected* monotone in presence observations; a custodian counts only with `has_commit` for the version (ADR 0046) | [06 §6.1](06-status.md) |
| headline | a pure function of the states; precedence total; never *Safe* while a version is not protected | [06 §6.5](06-status.md) |
| released keys | isolation: releasing one reference never releases a key another holds, and an uncovered reference removes its key; seal safety; monotone in the seals; independent of arrival order; never a current version | [08 §8.6](08-custody-loops.md), [08 §8.6](08-custody-loops.md) |
| settings | expected git-annex settings are a function of the roster; applying twice changes nothing | [03 §3.5](03-repository-and-roles.md) |
| install journal | replay idempotent; each path ends old or new | [11 §11.5](11-local-state.md) |
| local files | round trip; newer formats refused | [11 §11.5](11-local-state.md) |
| compatibility | an older release's verdict is the newer one's or *unknown*; an unknown field or `format` is never ignored; an unreadable status report never raises a state (ADR 0048) | [04 §4.4](04-roster-and-authority.md), [06 §6.3](06-status.md) |
| append-only filter | never forwards a removal; forwards allowed messages unchanged, but a custodian's `CHECKPRESENT`; keeps `DATA` framing; no positive answer for a key before its sync (ADR 0046); a positive `CHECKPRESENT` only for a copy it could lock ([09 §9.4](09-network-and-joining.md)); to a custodian, a positive `LOCKCONTENT` or `CHECKPRESENT` only for a copy whose hash matched ([09 §9.4](09-network-and-joining.md)) | [09 §9.4](09-network-and-joining.md) |
| trace merge | Lamport merge associative and commutative; per-device order kept | [12 §12.3](12-observability.md) |
| work queue | order is a total preorder of the classes of ADR 0037; capture never starves; a key held in fewer failure domains never waits behind one held in more ([08 §8.9](08-custody-loops.md)) | [15 §15.2](15-resource-use.md) |
| failure domains | domains are the connected components of shared storage, host and declared `domain`; their number is at most the number of custodians; a fingerprint or declaration never raises a state; with one custodian per domain the state is ADR 0002's; a drop's chosen remotes lie in K distinct domains; domain repair is deterministic and wants nothing for a key that spans K domains (ADR 0046) | [06 §6.2](06-status.md), [08 §8.3](08-custody-loops.md) |
| capacity | the parts of the need sum to the total; every key not protected has a reason; admission never defers an endangered key for one that is not ([08 §8.9](08-custody-loops.md)) | [08 §8.9](08-custody-loops.md) |
| trees and names | every tree capture makes is well formed (rule 6); `package(p)` depends only on *p*'s name and is stable when other paths change; merging a package never mixes files of two sides; `ignored` is a pure function of the name | [07 §7.9](07-capture-and-install.md), [05 §5.2](05-catalogue-and-names.md) |
| encodings | `parse(print(x)) = x` for roster TOML, NDJSON, git objects; our parser agrees with git; TLC reads from `scripts/trace-to-tla`'s module what the NDJSON held, or the line is refused (§13.3) | everywhere |

Example tests remain for regressions found by properties (with the shrunk
input) and for the fixed scenarios of this specification.

**Reference model of the catalogue** ([05 §5.2](05-catalogue-and-names.md)). `models/catalogue/catalogue.py`,
in plain Python, defines `catalogue(S)` and runs every schedule of
contributions and pulls between three devices, for eleven cases: add and add,
edit and edit (with two copies of one minute), edit and delete, rename and
edit, a case collision, a Unicode collision of folders, a package, a save after
a conflict, *Keep mine* and *Keep both* racing new edits, and a decision
merge. Schedules cover orders, pairings, fast-forwards, duplicate merges and
deliveries, and repeated split and rejoin. The reachable states are finite,
and the model visits them all: up to 264 500 for one case. On each state it
checks the laws of the *catalogue merge* row above. It also replays fixed
histories with their expected trees and open conflicts. It runs the rule
before [05 §5.2](05-catalogue-and-names.md) (`old`: merge commits are changes) and its obvious repair
(`naive`: copies derived from the heads) up to five pulls, and expects their
failures, so a model change that hides them fails. CI runs it in about 30
seconds. The core's hegel properties must agree with it on the same cases.

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
They live in `crates/pigeon-git/tests/contract.rs`; CI sets
`PIGEON_REQUIRE_GIT_ANNEX=1`, so they fail rather than skip when git-annex is
missing (03 §3.7). A subset runs on each device as the contract self-test when it first meets a
new version (ADR 0048).

**Format fixtures** (ADR 0048). `fixtures/formats/` holds golden documents and
small signed histories for every supported (`format`, `rules`) pair, each with
its expected verdicts, and fixtures of the next format that the current
release must judge *unknown*. Every release runs them. Two releases built
from two commits, in one group, are a prototype test (#17).

## 13.7 The lab (`lab/git-annex/`)

| Experiment | Result | Used by |
|---|---|---|
| E1, E5 | concurrent drops of the last two copies over local paths: 40/40 left one (over P2P they do not: E23); a stale location log is not trusted | INV-K, [08 §8.3](08-custody-loops.md) |
| E2–E4, E6 | deleted content kept until `dropunused`; a propagated `rm -rf` undone; concurrent edits keep two variants; absent content is a pointer file | [05](05-catalogue-and-names.md), [07](07-capture-and-install.md) |
| E7 | a pull filter merges an allowed contribution and keeps a disallowed edit as a proposal | [04](04-roster-and-authority.md) |
| E8 | `annex.thin`: an in-place edit corrupts the laptop's copy; others refuse to count it | ADR 0002, [07 §7.5](07-capture-and-install.md) |
| E9 | roles: untrusted laptop, two custodians, K = 2; `--all` needed for history | [03](03-repository-and-roles.md), [08](08-custody-loops.md) |
| E10 | partial custodians: `sizebalanced` is heuristic (6/10 where 8/10 fit); repair within capacity | ADR 0003, [08 §8.9](08-custody-loops.md) |
| E15 | freeing per folder: sparse checkout, drop and get by key, `smudge --update` (asserts) | ADR 0031, [08 §8.5](08-custody-loops.md) |
| E15b | ADR 0031's order shows pointer files (44 887 and 1 306 observations for 100 files); hiding before dropping and staging before showing show none, also under `kill -9`; a `kill -9` inside `git sparse-checkout set` leaves `info/sparse-checkout.lock`, which blocks every later free until it is removed (asserts) | [08 §8.5](08-custody-loops.md), [08 §8.5](08-custody-loops.md) |
| E18 | five offline saves of one file, failed uploads, reconnection: with `annex.thin` in-place saves lose 4 of 5 versions; capturing a copy keeps all 5, and protection, drop by key and re-thinning return to about one copy; `drop --auto --all` is refused (asserts) | [07 §7.8](07-capture-and-install.md), [07 §7.8](07-capture-and-install.md) |
| E19 | install against an application acting at every boundary (in place, atomic replace, create, delete, old handle, shared mapping), thin and not thin, 21 `kill -9` points recovered twice, a fuzzing writer: no save lost, the index never calls a changed file clean, recovery idempotent; without the writer scan 9 of 49 writes reach the aside (asserts). On macOS the `libproc` scan sees write handles and mappings and costs 30–50 ms, so the name is absent for a median of 32–50 ms per install (0.5 ms on Linux); a write through a shared mapping leaves the stat unchanged until `msync`, so git calls the file clean, before any install (`SKIP (platform)`; no save is lost) | [07 §7.5](07-capture-and-install.md), [07 §7.5](07-capture-and-install.md) |
| E14 | purge by references with git and git-annex, two laptops and two bare custodians: two paths sharing a key, current content sharing a key with purged history and a suggestion sharing a key with accepted content keep their key (the suggestion through the seals' `held`); a restore that reached one custodian before its seal holds the purge back at the other until it merges that `main` (trusting the seals alone would release it); per-key transfers never fetch a released key although git-annex's own `wanted` still matches it; `drop --force --key` leaves no copy; a returning laptop drops its covered copy and its offline re-add comes back to both custodians; `matchexpression --key` agrees with git-annex's choices (`balanced` with `satisfy`, 11 keys split between the two; `sizebalanced` with `find --want-get`, which lists nothing in a bare repository); `get --auto --all` and `drop --auto --key` are refused and `satisfy --all` drops from a remote (asserts) | [08 §8.6](08-custody-loops.md), [08 §8.6](08-custody-loops.md) |
| E12 | `remotedaemon` over git-annex's generic P2P transport (a lab network): on a change notice it fetches by the remote's refspecs and never moves `main`, `HEAD`, the index or the working tree; by default a peer pushes any ref into a custodian, `main` included, and into a laptop except its checked-out branch; a `pre-receive` hook refuses (asserts). Homebrew's git-annex has no `git-remote-p2p-annex`, which pigeon links on macOS; a fetch or push over P2P can hang when git-annex's relay drops git's last output, which E12 retries and prints as a note (16 §16.4) | [09 §9.4](09-network-and-joining.md), [03 §3.6, §3.8](03-repository-and-roles.md) |
| E13 | `remotedaemon` serves `REMOVE` even with `GIT_ANNEX_SHELL_APPENDONLY`; append-only `git-annex-shell p2pstdio` refuses it but relays no git; pigeon's filter before `remotedaemon`, with random read sizes, passes reads, stores, fetches and change notices, refuses `REMOVE` on the custodian and any push, caps `VERSION`, closes on unknown or malformed input, passes payloads holding protocol lines intact; explicit `--numcopies` outweighs `.gitattributes` and a lowered branch setting, which `drop --key` otherwise obeys (asserts; 30 checks, about 90 s) | [09 §9.4](09-network-and-joining.md), [09 §9.4](09-network-and-joining.md) |
| E20 | what the folder may hold, on Linux and macOS (`lab.yml`, 53 checks each): links, nested repositories, FIFOs and sockets are never staged when capture takes paths from `git status` and checks `lstat` first, yet `git annex add` stages a link or a nested repository's file it is given; `git annex add` stages an executable file as 100644 and ignores a mode-only change, and `update-index --chmod` restores the bit; extended attributes do not travel; a thin object is hard-linked to one path only, and `fsck` does not notice an object changed through that link; unreadable paths are not reported deleted; a file rewritten while it is added never yields an object that mismatches its key; a person's `~/.gitconfig` (`annex.backend=WORM`, `annex.largefiles=nothing`, `core.excludesFile`) changes keys, tree and listing unless git runs isolated; `--ignored=traditional -uall` lists the files of ignored folders; `.git/info/attributes` overrides nested `.gitattributes`; `add --batch` fails on a name that is not UTF-8 (asserts) | [07 §7.9](07-capture-and-install.md), [03 §3.7](03-repository-and-roles.md), [07 §7.9](07-capture-and-install.md), [03 §3.7](03-repository-and-roles.md) |
| E21 | the disaster drill (issue #11): content on K custodians before they have its commit is a key without a name once the laptop is lost; after losing every laptop and its key, a fresh machine recovers from custodians alone every name, hash, deleted version and suggestion, and every signature verifies against the roster at its parents; a recovery sheet's roster commit is accepted and nothing else it signs; `git clone` and `git annex get` read the files without pigeon, across partial custodians; a copied repository under a new UUID counts only after a hashing fsck; git's default `core.fsync` syncs neither loose objects nor refs (asserts) | ADRs 0046, 0047, [09 §9.9](09-network-and-joining.md), [06 §6.2](06-status.md) |
| E22 | failure domains and capacity (issues #12, #13), on Linux and macOS, 7 checks: fingerprints of disks and machines (one volume, one fingerprint; a loop mount or disk image follows its file to its disk; a tmpfs differs, and its mount source alone collides); git-annex counts two repositories on one disk as two copies, even one hard-linked inode; a drop that ignores all but one remote per domain refuses until another domain holds the key; `annex.diskreserve` refuses a get and an incoming copy, local and over `p2pstdio`, with a misleading message at the sender; `sizebalanced` with small `maxsize` leaves keys at one copy and the verified drop refuses to go lower; a real `ENOSPC` (another writer; git-annex alone stops at the free space) fails a get and a fetch cleanly, `git annex fsck` fails until space is freed, the retries succeed; a custodian killed during a copy counts only complete keys, and a deleted one is written into again and still claimed, which only the verified drop catches (asserts). Found on the way: `annex-bwlimit-download` is never read (`Types/GitConfig.hs` maps it to `bwlimit-upload`) | ADR 0046, [08 §8.9](08-custody-loops.md), [08 §8.3, §8.9](08-custody-loops.md) |
| E23 | git-annex's verified drop over its P2P protocol (lab network, 12 checks, about 15 s): a drop counts a copy that another custodian holds locked for its own removal, since `CHECKPRESENT` answers with `inAnnex`; over a local path it refuses; two concurrent drops leave no copy with numcopies 1, and one of three with numcopies = mincopies = 2; behind a filter that answers `CHECKPRESENT` by `LOCKCONTENT` both races keep every copy and a lone drop still succeeds; a stale log is not counted, but is for a custodian marked `trust`; a `dead` one never counts (asserts) | [09 §9.4](09-network-and-joining.md), `AnnexDrop`, [08 §8.3](08-custody-loops.md), [09 §9.4](09-network-and-joining.md) |
| E24 | what freeing a folder may lose or show, on Linux and macOS: `git sparse-checkout set` unlinks a clean file although a writer holds it open, so the write lands in no file (with `annex.thin`, inside the object, which git-annex then calls absent); a hide that renames the file aside first keeps it; `git annex drop --key` turns every unmodified file of its key into a pointer file with the old mtime, in another kept folder and in a staged, moved-in copy, and `git status` stays clean (asserts) | [08 §8.5](08-custody-loops.md), [08 §8.5](08-custody-loops.md), `Evict` |

**Pending:** E11 native Windows (v1.1). **New:** E16 `annex.bwlimit` over the P2P
transport; E17 the HDD video-editing acceptance test ([15 §15.8](15-resource-use.md)).
E9 and E10 must be rerun with bare custodian repositories
([03](03-repository-and-roles.md)). Each experiment joins CI once its script
asserts its results (ADR 0026); every asserting one runs in `lab` on Linux and
macOS (§13.8).

## 13.8 Continuous integration (ADR 0026)

- `ci` on every push and pull request: the repository rules
  (`scripts/check-file-length-and-header`, `scripts/check-adr-index`), the fast
  model set; with the first crate, `cargo fmt --check`, `clippy` (with
  `unwrap_used` and `expect_used` denied, ADR 0023), tests with the fast
  property and fault sets, the Fluent completeness test (ADR 0028), the
  licence check (`cargo deny`, ADR 0019), E15 and every asserting experiment.
- `lab` on every push: every asserting experiment (E12–E15b, E18–E23) on
  Linux and macOS, with the git-annex each installer uses (the standalone
  build, Homebrew), in four parallel groups, 6 to 11 minutes. A platform
  difference is printed as `SKIP (platform)` with its reason, never a dropped
  check, and the script still asserts what it can. Every script isolates git
  from the person's configuration and sets a lab identity, as `pigeond` runs
  git (03 §3.7).
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
| P1 Survival | E1, E5, E8, E9, E10 (lab, printing); E13 (asserting: pigeon's filter refuses remote `REMOVE`); E21 (asserting: recovery from custodians alone); E22 (asserting: one copy per failure domain, the reserve, disk full); E23 (asserting: over P2P git-annex's verified drop counts a copy locked for removal, two concurrent drops lose the key, pigeon's probe keeps it); `AnnexDrop` model (git-annex's verified drop: safe over a local path or behind the probe, not over P2P; [09 §9.4](09-network-and-joining.md)); `Repair` model (placement, repair and rebalancing against lost disks, removals, rotten and restored copies, crashes; a rotten copy proving a drop and a full custodian stalling domain repair, ADR 0046, [09 §9.4](09-network-and-joining.md)); `Evict` model (a laptop drops only keys K custodians hold, verified when the drop runs; [08 §8.5](08-custody-loops.md)); `CustodyNaive`, `CustodyTokens` (evidence about hand-offs in general and about the earlier token protocol, not about git-annex) | failure-domain and capacity laws; the filter's hash law ([09 §9.4](09-network-and-joining.md)); fault injection; asserting E9, E10 | independence pigeon cannot see (virtual machines, places not declared; ADR 0046); power loss on a custodian (outside the lab; ADR 0046 rests on the git and git-annex sources) |
| P2 Capture, no overwrite | E20 (asserting, Linux and macOS: what capture stages and what it never reads); E15b (asserting: no pointer during free and keep); E24 (asserting, Linux and macOS: git's hide loses a write through an earlier handle, a drop turns other files of its key into pointers; [08 §8.5](08-custody-loops.md)); E18 (asserting: offline in-place saves keep every version); E19 (asserting, Linux and macOS: racing saves and `kill -9` during install); `CaptureInstall` model (racing saves, escaping writers, crashes during install and recovery, resolutions; [07 §7.5](07-capture-and-install.md)); `Evict` model (free and keep against saves through earlier handles, group changes, lost copies, crashes and switches: no save lost, no pointer shown, nothing captured that the person did not write; [08 §8.5](08-custody-loops.md)) | install journal laws (hegel-rust), checked against the model | copy-on-write cost of pending history on APFS and btrfs (E18 there); saves from stale memory, and on macOS saves made only through a shared mapping until they are synced (16 §16.4) |
| P3 Reversible deletion | E2, E4 (lab, printing); `Purge` model (seals against restores, cancels, partitions, restarts and a returning laptop; [08 §8.6](08-custody-loops.md)); E14 (asserting: shared keys, a restore against the seals, per-key transfers, the forced drop, a returning laptop's new generation) | purge laws (hegel-rust); `Purge` trace validation | a custodian clock that jumps forward by more than the waiting period; erased names stay in history (16 §16.4) |
| P4 Authority | E7 (lab, printing); E12 (asserting: no pushed ref, `remotedaemon` only fetches); `Roster` model (roster forks, ADR 0051); `Authority` model (acceptance, proposals and decisions against a faulty member, a removed laptop signing on old parents; ADR 0051) | roster fold laws; acceptance and revocation laws; decision-rights tests (ADR 0051); `Authority` trace validation | format compatibility (#15) |
| P5 Convergence | E3 (lab, printing); `models/catalogue` (every schedule of three devices over eleven cases; the earlier merge rule fails as expected) | merge and name laws (hegel-rust), checked against the reference model | rename tracking (16 §16.4) |
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

## 13.12 The first milestone: the safety slice

The first code of v2 is a slice that carries the promises and nothing else
(issue #17). Its group has one person, who is its admin, one laptop (Linux on
ext4, or macOS on APFS) and two Linux custodians on ext4, with K = 2. It has
one repository and two folders.

| Part | In the slice | After it passes |
|---|---|---|
| crates ([14 §14.6](14-platforms-and-packaging.md)) | `pigeon-core`, `pigeon-proto`, `pigeon-i18n` (the English bundle), `pigeon-git`, `pigeon-sys`, `pigeond`, `pigeon`, `git-annex-p2p-iroh` | `pigeon-web`, `pigeon-telemetry`, the French bundle (ADR 0028) |
| loops ([08 §8.1](08-custody-loops.md)) | capture, accept, install with its journal ([07 §7.5](07-capture-and-install.md)), place, repair (started by a removed custodian), free and keep, settings check; the filter with its `CHECKPRESENT` probe ([09 §9.4](09-network-and-joining.md)) | scrub, purge |
| commands ([10](10-interfaces.md)) | `create`, `invite --backup-machine`, `join --backup-machine`, `status`, `machines`, `history`, `restore`, `free`, `keep`, `remove-computer`, `recovery-sheet`, `recover`, `doctor`, `debug bundle` | the rest answer `not-in-this-build` |
| content | regular files and folders, the executable bit, portable names ([07 §7.9](07-capture-and-install.md)) | other file systems: `create` and `join` name the one they refuse (`file-system-not-in-this-build`, 10 §10.5) |
| network | iroh with direct addresses and mDNS | relays and discovery (ADR 0016) |
| diagnostics | JSON logs (ADR 0022), trace logs (ADR 0008) | telemetry transport and dashboards (ADR 0025) |

Also after it: more people and laptops (invitations of people, proposals,
conflicts between devices, stewards and decisions), purge, erase and the
scrub, notifications, packaging and service units, adaptive limits (the slice
keeps only low CPU and I/O priority), the tuning of large and hot files, and
Windows. A deferred command answers `not-in-this-build`, and the slice holds
no partial code for it. The deferrals concern the order of work only: v1
still meets every accepted ADR. **[no ADR: milestone]**

**The first build** (14 October 2026) has `pigeond`'s main, the loops of the
table, the commands and an integration test of three daemons on one host
(`crates/pigeond/tests/slice.rs`: *local*, then *published* with one backup
machine, *protected* with two, `free`, and `keep` giving back the same
bytes). Until the slice passes, it differs from this specification as
follows, each to close before S1–S11 run **[no ADR: first build]**:

- `restore`, `recovery-sheet` and `recover` answer `not-in-this-build`;
- roster and capture commits are not signed ([04 §4.4](04-roster-and-authority.md)),
  and accept only fast-forwards `main`, on custodians: a laptop installs no
  incoming version, so the group has one laptop and no merge;
- place copies every key from the laptop to every custodian, and each
  custodian gets every key, without the queue order or capacity admission
  ([08 §8.9](08-custody-loops.md)); repair marks a removed device's UUID `dead`, after which place
  copies again;
- `heard` times and the trace clock are kept in memory, not in `clock.json`
  ([11 §11.3](11-local-state.md)); `history` takes the state of older
  versions from their copy counts;
- the device key is kept as hex, not in OpenSSH format ([11 §11.2](11-local-state.md));
  an invitation is base32 of JSON, not of a compact binary encoding, and
  also carries the group's name, K and the inviter's name for messages;
- a custodian's fingerprints and host are kept in `roster.toml` beside its
  device, with no status reports yet ([06 §6.3](06-status.md));
- the person's ID is taken from `$USER`, and a custodian is named
  `<host name>-<6 hex digits of its device ID>`;
- the CLI parses its arguments by hand, without `clap`, completions or
  colour ([10 §10.2](10-interfaces.md));
- one round runs, per group in turn, the settings check, capture, accept,
  place and repair, every `[loops] interval_seconds` and at once after a
  change; `status` on a laptop captures first.

**Setup** (`crates/pigeon-lab`). The lab build (`--features lab`) adds three
seams to `pigeon-sys`, and nothing else:

- fingerprints read from a file named by `PIGEON_LAB_FINGERPRINTS`
  (`{"storage": [...], "host": "...", "virtual": ""}`, `null` for unknown),
  since one host is one failure domain (ADR 0046); in the slice macOS reports
  its host (`IOPlatformUUID`) and no storage;
- named crash points (`PIGEON_LAB_CRASH=point:n`, checked at start): the n-th
  crossing sends `SIGKILL` to the process itself, so no destructor runs and
  no buffer is flushed, as with `kill -9`;
- a root for a backup machine's system directories
  (`PIGEON_LAB_SERVICE_ROOT`), so that custodians run without mount
  namespaces on the macOS runners and on a developer's machine.

A release build ignores the three variables.

It says so in `doctor`, and its status reports carry `lab = true`, which a
release build never counts. On one Linux host, each custodian runs in its own
mount namespace on its own loop-mounted ext4 image, connected over
127.0.0.1, with direct addresses only. On GitHub's hosted runners the scenarios run on `ubuntu-24.04`
(this setup, with the runner's `sudo`) and on `macos-latest`, where each
device's disk is a sparse APFS image attached with `hdiutil` and custodians
use the service-root seam (lab only: macOS is no backup machine, ADR 0032). These two runners replace a
run on real hardware (Guillaume, chat, 14 October 2026). Real fingerprints
reaching *protected* are then shown only by E22, and network faults beyond
loopback (NAT, relays, Wi-Fi drops) by no scenario; the ledger marks what
rests on them *conditional*.

**Scenarios.** Each is a program of pigeon-lab's typed EDSL
(`crates/pigeon-lab/src/slice.rs`), not a script: a world of typed device
handles (a custodian has no `free`), steps, conditions with deadlines and
expected outcomes, as data that prints, round-trips through JSON and is
generated and shrunk by hegel. Two interpreters fold the same steps:

- the **model**, over `pigeon-core` (states, headline, the install journal
  and its recovery), in `cargo test`: every scenario must pass there, so its
  expectations are what the specification predicts;
- the **executor**, with the lab build of `pigeond` and `pigeon --json` per
  device, supervised as systemd or launchd would (a custodian runs
  `pigeond --service`); it records disk use, and merges the devices' trace
  logs, where each device's events must contain its part of the scenario's
  projection in its own order, and which `scripts/check-lab-traces` hands
  to TLC for every specification with a `*Trace.tla` (the others are
  listed as not checked yet). A device's paths stay short (`/tmp/lab`, a
  hashed directory per scenario): unix sockets live under them, and past
  104 bytes a join fails with `inviter-offline`.

Laws (`crates/pigeon-lab/tests/laws.rs`): steps form a monoid; validation
and the trace projection are compositional; a prefix runs the same whatever
follows; settling the model is idempotent; a crash at any named point, then
a restart, changes no outcome (1000 model schedules per push). The workflow
`slice.yml` runs them, then S1–S11 and 50 random S9 schedules against the
real binaries on both runners; `models-full.yml` runs 1000. A scenario that
does not pass yet against the first build is listed in `slice::BLOCKED`
with what its last runs showed; per push it is reported as blocked, and
`models-full.yml` runs it (`pigeon-lab run --with-blocked`) without
failing. On 14 October 2026 every scenario is blocked: on both runners
the laptop stays at *published* after the second custodian accepts a
version (S1, S4, S6, S7; S1 passes now and then on one host), `free`
hides nothing (S2, S3), no partial object shows during S5's transfer, a
full custodian says nothing (S8), S9's crash points of `free` and `keep`
fall inside the request where the model frees in its loops, there is no
recovery sheet yet (S10), and the laptop's disk holds each file twice
(S11, 2.06 × the content kept).

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

S7's repair order and S10's list of what recovery needed are read from the
run's trace and report, not yet asserted by a condition.

**Laws first.** The parts of `pigeon-core` that the slice uses have their
hegel-rust laws before their drivers: states and headline, domain counting,
the install journal, the queue order and the roster fold.

**Passing and the ledger.** The slice passes when every scenario asserts on
both runners, no protected version is lost in any
schedule, and laptop disk use in the steady state is at most 1.1 × the
content kept (§13.11). The peak is reported with its causes.
`docs/reports/slice-1.md` marks each promise (P1–P6) and each Proposed ADR it
exercises as *established* (with its scenarios and traces), *conditional*
(with the assumption it rests on) or *untested*. Evidence from pigeon's own
code enters the "now" column of §13.10 only through this ledger. Issue #17
closes when the ledger is published with the slice passing.

## 13.13 The laws in code (ADR 0012)

`pigeon-core` states each law in the documentation of its module and checks
it with a `hegeltest` property in `crates/pigeon-core/tests/` (at least 1000
cases each; a law that needs a rare input gets a targeted generator and an
example test). The laws of §13.4 not listed here wait for their module.

| Law | Module | Test | Model mirrored |
|---|---|---|---|
| domains partition the custodians, ignore input order, join custodians that share a fact; a new fact only merges; no shared fact, one domain each | `domain` | `tests/domain.rs` | — |
| more presence observations never lower a state; a new fact never raises one; custodian order is irrelevant; one custodian per domain is ADR 0002's rule; a new holder never lowers a state | `state` | `tests/state.rs` | — |
| the join is a semilattice with *Safe* as identity and *Needs you* absorbing; the headline of a union is the join; adding an item never lowers it; a version only this computer holds is *Not safe yet* after the grace period, even unreachable | `headline` | `tests/headline.rs` | — |
| the queue order is total; a higher class goes first; arrival order is irrelevant; a key in fewer failure domains never waits behind one in more | `queue` | `tests/queue.rs` | — |
| recovery ends every entry and a second does nothing; a crash during recovery ends as one recovery; a person's save is never lost; a deletion after install stays; without a person, the path ends old or new and the index agrees (INV-Journal); pigeon never renames over a name | `journal` | `tests/journal.rs` | `CaptureInstall` (`NoLoss`, `Journal`, `NoOverwrite`, `DeletionKept`) |
| every tree is the fold of its history; the fold depends only on the set of changes; every fold is valid and removals win, even of a device not yet added; skips need concurrency; a linear history skips nothing; the canonical order is topological; pigeon-core lets a device sign every change of the model's `Ops` | `roster` | `tests/roster.rs` | `Roster` (`TreeIsFold`, `Agreement`, `AdminRemains`, `RemovalWins`, `SkipsNeedConcurrency`), and `RosterTrace` on runs |
| a trace line parses back to its event; the Lamport clock orders a receive after its send; every action name is an operator of its `.tla` | `trace` | `tests/trace.rs`, `tests/roster_trace.rs` | every `*Trace.tla` |
