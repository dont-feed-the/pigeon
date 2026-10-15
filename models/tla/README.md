# The TLA+ models

What each configuration of `expected.tsv` shows, and what each model leaves
out. This is evidence; what the models must prove is stated in spec
[13 §13.2](../../docs/spec/13-verification.md), and the invariants it cites
are normative. `scripts/run-tlc` runs the configurations: the fast set in
`ci`, the full set in `models-full` (ADR 0026). State counts are those of
the run that last changed a model.

## Configurations

| Configuration | Model | Expected verdict | Shows |
|---|---|---|---|
| `Naive_FALSE` | `CustodyNaive` | `violation:NoLoss` | a naive "ask, then drop" hand-off loses the last copy |
| `Naive_TRUE` | `CustodyNaive` | `deadlock` | the same with locks deadlocks |
| `T_idem` | `CustodyTokens` | `violation:TokensNeverLost` | idempotent acknowledgements lose tokens |
| `T_nononce` | `CustodyTokens` | `violation:TokensNeverLost` | offers without nonces lose tokens |
| `T_nonce` | `CustodyTokens` | `pass` | nonces make the hand-off safe |
| `T_n42o3`, `T_n31o5`, `T_n32o5` | `CustodyTokens` | `pass` (fast set) | N, K and offers varied |
| `T_n42o4` | `CustodyTokens` | `pass` (full set) | N = 4, K = 2, 4 offers |
| `AnnexDrop_design` | `AnnexDrop` | `pass` (fast set) | git-annex's verified drop as pigeon uses it (ADR 0046, [09 §9.4](../../docs/spec/09-network-and-joining.md)): each custodian drops only its own copy, under its removal lock, once it has locked, or seen present by a check that honours removal locks, K copies among those its location log lists; location logs go stale, partitions cut a drop off, a lock expires under a stalled drop, a custodian is marked `dead`, a new copy is made. The key stays on K custodians (INV_K) and a locked copy stays (LockedStays); 3 custodians, K = 2: 406 880 distinct states, 11 s on 1 worker |
| `AnnexDrop_ranked` | `AnnexDrop` | `pass` (fast set) | with git-annex's P2P check, which ignores removal locks, a drop that counts only custodians ranked above its own is safe: the alternative [09 §9.4](../../docs/spec/09-network-and-joining.md) rejects, for liveness |
| `AnnexDrop_design4`, `AnnexDrop_gets2`, `AnnexDrop_ranked4` | `AnnexDrop` | `pass` (full set) | the design with 4 custodians, without expiry or new copies (4 518 880 distinct states, 4.5 minutes on 1 worker); with two new copies (1 259 520, 35 s); the ranked drop with 4 custodians (970 240, 44 s) |
| `AnnexDrop_p2p` | `AnnexDrop` | `violation:INV_K` | git-annex over its P2P protocol: `CHECKPRESENT` ignores the removal lock, so two custodians that drop at once each count the other's copy (E23) |
| `AnnexDrop_log` | `AnnexDrop` | `violation:INV_K` | counting the location log without checking: a stale log drops the last copies (E5) |
| `AnnexDrop_trusted` | `AnnexDrop` | `violation:INV_K` | one custodian marked `trust`: the others count it unasked from a stale log (E23) |
| `Repair_design` | `Repair` | `pass` (fast set) | placement, repair and rebalancing on custodians (08 §8.3–8.4, §8.8–8.10; ADR 0046, [08 §8.9](../../docs/spec/08-custody-loops.md), [09 §9.4](../../docs/spec/09-network-and-joining.md)): custodians 1 and 2 hold key *a*; the joiner 3, with room for one key, comes in empty or on a restored disk; the laptop holds key *b*. Faults (a lost disk, a rotten copy, a removed custodian) stay within F, plus a crash or a partition. A key once protected keeps an intact copy on disk or on the laptop (Survival); no disk crosses its reserve (ReserveKept); no download takes the room of an endangered key (EndangeredFirst); a copy leaves only with its disk or its custodian, by a scrub, by a crash before its sync, or by a drop of an unwanted key backed by K lockable copies (INV_K, a property of steps). 355 939 distinct states, 27 s on 1 worker |
| `Repair_live` | `Repair` | `pass` (fast set) | liveness: under fairness (transfers, syncs, scrubs and heals run; admins remove lost custodians and add the joiner) and with room for K copies of everything, an endangered key gets K custodians back (Restores): 19 364 distinct states, 22 s |
| `Repair_live4` | `Repair` | `pass` (fast set) | four custodians, one key: the joiner is at its reserve yet first in sizebalanced's order, and overflow chooses custodian 4 |
| `Repair_design3`, `Repair_design4`, `Repair_live2` | `Repair` | `pass` (full set) | three faults and two crashes or partitions (2 990 176 distinct states, 3.5 minutes); four custodians, two of them joining (515 705, 43 s); liveness with two faults (147 880, 2 minutes) |
| `Repair_lockonly` | `Repair` | `violation:Survival` | a lock that proves presence only ([09 §9.4](../../docs/spec/09-network-and-joining.md) alone): custodian 1's copy rots, custodian 2 drops *a* for the joiner and locks 1 and 3, then 3's disk dies. One fault in the epoch, and no intact copy is left ([09 §9.4](../../docs/spec/09-network-and-joining.md)) |
| `Repair_nosync` | `Repair` | `violation:Survival` | copies counted before their sync (ADR 0046): the laptop drops *b* while both custody copies are still only in memory |
| `Repair_nofsck` | `Repair` | `violation:Survival` | a restored disk counted before its fsck (09 §9.9): the laptop's drop leans on its bad copy, and the only other copy had rotted |
| `Repair_noprio` | `Repair` | `violation:EndangeredFirst` | admission without the endangered-first rule ([08 §8.9](../../docs/spec/08-custody-loops.md)): the joiner starts *b* with the room that *a* needs |
| `Repair_inflight` | `Repair` | `violation:ReserveKept` | a reserve check that ignores downloads in progress, which git-annex counts (`sizeOfDownloadsInProgress` in `checkDiskSpace'`, `Annex/Content/LowLevel.hs`): two downloads cross the reserve |
| `Repair_wantall` | `Repair` | `violation:temporal` | overflow counting a member at its reserve as taking the key ([08 §8.3](../../docs/spec/08-custody-loops.md) counts only members that can take the key): no other custodian is chosen, and the key stays endangered |
| `Admission_design` | `Admission` | `pass` (fast set) | [08 §8.9](../../docs/spec/08-custody-loops.md), issue #13: admission on one custodian, every instance of four keys of sizes 0..3, some endangered, and a room of 0..7, decided in any order: what is admitted fits in the room (`ReserveKept`), an endangered key's room never goes to another (`EndangeredFirst`), a deferral is final (`DeferredStays`); `Repair`'s `StartGet` guard with sizes |
| `Admission_noprio` | `Admission` | `violation:EndangeredFirst` | admission without the room kept for endangered keys: a key that is not endangered takes it |
| `Admission_noused` | `Admission` | `violation:ReserveKept` | admission that compares each key with the whole room, not what is left of it: two keys cross the reserve |
| `Evict_free` | `Evict` | `pass` (fast set) | issue #10, [08 §8.5](../../docs/spec/08-custody-loops.md), [07 §7.8](../../docs/spec/07-capture-and-install.md): one laptop, folders F = {a, b} and G = {c}, where c has a's key; F switched from kept to freed while the person saves (in place through a handle opened earlier, or by atomic replace), custodians lose a copy, status reports lag and `pigeond` crashes. No save or captured version is lost (NoLoss); no file is ever a pointer (NoPointer); a key is dropped only while K custodians hold it (DropsProtected); capture commits only what the person wrote (CaptureOnlySaves); no path is captured as deleted (NoDeletion): 52 944 distinct states, 5 s on 1 worker |
| `Evict_keep` | `Evict` | `pass` (fast set) | F switched from freed to kept while the group changes F's files (the index of hidden paths moves during the keep): 62 744 distinct states, 4 s |
| `Evict_live`, `Evict_livekeep` | `Evict` | `pass` (fast set) | liveness under fairness: a folder that stays freed ends hidden, nothing aside or staged, its keys gone unless G uses them (FreeLive); a folder that stays kept ends with every file present (KeepLive): 52 944 and 46 596 distinct states, about 20 s each |
| `Evict_flip`, `Evict_live2`, `Evict_livekeep2` | `Evict` | `pass` (full set) | F switched kept → freed → kept with a group change, a lost copy and a crash (10 612 600 distinct states, 9.5 minutes on 1 worker); FreeLive and KeepLive with two switches (466 952 distinct states, 3 minutes) and with a group change during a keep (1 136 988, 11 minutes) |
| `Evict_git` | `Evict` | `violation:NoLoss` | hiding with git's own unlink (08 §8.5 before [08 §8.5](../../docs/spec/08-custody-loops.md) hid a file as an install): an editor holds b open, the hide unlinks it, and the next write lands in no file (E24) |
| `Evict_guard` | `Evict` | `violation:NoPointer` | a free that drops a's key, which G's c also has: git-annex turns c into a pointer file (E24) |
| `Evict_target` | `Evict` | `violation:NoPointer` | the guard counting visible paths only: F is freed during a keep, after its staged files moved in with their bits set, and the drop turns them into pointers (E24) |
| `Evict_prune` | `Evict` | `violation:NoPointer` | the drop of old versions ([07 §7.8](../../docs/spec/07-capture-and-install.md)) without the guard: c's old version is a's current one |
| `Evict_lock` | `Evict` | `violation:DropsProtected` | a drop that trusts the status report instead of locking K copies when it runs: a custodian lost the key since |
| `Evict_stage` | `Evict` | `violation:NoPointer` | ADR 0031's keep order: showing with a checkout writes pointers for content not yet fetched (E15b) |
| `Evict_stale` | `Evict` | `violation:CaptureOnlySaves` | a keep that clears the bit of a moved-in file without comparing it with the index: main moved since the staging, and capture commits the old version as the person's save |
| `Evict_copy` | `Evict` | `violation:NoLoss` | capture with `annex.thin=true`: the object is the file's inode, and an in-place save destroys the captured version before any custodian has it ([07 §7.8](../../docs/spec/07-capture-and-install.md), E18) |
| `Evict_journal` | `Evict` | `violation:NoDeletion` | capture that does not skip a path whose file is aside: the hide's window is captured as a deletion |
| `Evict_lockfile` | `Evict` | `violation:temporal` | a stale `info/sparse-checkout.lock` kept after a crash: the hide never completes, and FreeLive fails (E15b) |
| `Purge_full` | `Purge` | `pass` (fast set) | [08 §8.6](../../docs/spec/08-custody-loops.md): a restore or cancel that reaches any custodian before its seal keeps the content (KeepWins); nothing is dropped before the waiting period (WaitingPeriod); a dropped copy returns only for an uncovered reference (NoResurrection); 2 custodians (one holds the key), 2 laptops (one offline with a copy), restarts, any message order |
| `Purge_full3` | `Purge` | `pass` (full set) | 3 custodians, 2 of them holding the key, and the deciding laptop with a copy: 12.4 million states, 2 minutes |
| `Purge_seals` | `Purge` | `violation:KeepWins` | accepting every seal without merging the sealers' `main` loses a cancel |
| `Purge_none` | `Purge` | `violation:KeepWins` | dropping when one's own waiting period ends (ADR 0018's first mechanism) loses a restore that reached another custodian |
| `Roster_fold` | `Roster` | `pass` (fast set) | issue #6: the roster of a commit is the fold of its non-merge ancestors' changes in canonical order (Kahn, smallest (time, device, id) first), each change atomic or skipped and reported, device removals always applied, a merge signed by a device of either parent's roster. Every roster keeps an admin (AdminRemains); a removed device stays removed (RemovalWins); each tree's roster is that fold, so merge order does not matter (TreeIsFold, Agreement); concurrent heads can always be merged (MergeAlwaysPossible); a change is skipped only for a concurrent one (SkipsNeedConcurrency). 3 persons (2 admins), 2 devices committing offline and merging any commit, a fresh key, all seven kinds of change, 5 commits: 608 563 states, 442 463 distinct, 11 s on 3 workers |
| `Roster_clock` | `Roster` | `pass` (full set) | the same with author times 0 or 1, so wrong clocks reorder the fold: 8 569 377 states, 6 224 785 distinct, 4 minutes on 2 workers |
| `Roster_fold3` | `Roster` | `pass` (full set) | three devices committing and merging, 5 commits: 10 321 071 states, 4 277 212 distinct, 5 minutes on 1 worker |
| `Roster_witness` | `Roster` | `violation:NoConcurrentRetirement` | a and b retire concurrently and a merge joins them: the first in canonical order applies, the other is skipped, b stays admin |
| `Roster_atomic` | `Roster` | `violation:RemovalWins` | skipping a rotation as a whole also skips its removal: a key added and removed on one branch makes the rotation to it fail on the other, and the old key comes back |
| `Roster_every` | `Roster` | `violation:MergeAlwaysPossible` | with rule 2 of [04 §4.4](../../docs/spec/04-roster-and-authority.md) (a signer in every parent's roster), branches that removed each other's devices have no common device and never merge |
| `Roster_person` | `Roster` | `violation:PersonRemovalWins` | a removes b while b removes a: both cannot apply without leaving no admin, so the later one in canonical order is skipped |
| `Roster_rebase` | `Roster` | `violation:Agreement` | replaying the other side's changes onto one's own roster depends on who merges: two merges of the same commits disagree |
| `Roster_table` | `Roster` | `violation:AdminRemains` | merging `roster.toml` per TOML table, a clean merge of the concurrent self-retirements leaves no admin |
| `Roster_adr0009` | `Roster` | `violation:MergeAlwaysPossible` | ADR 0009's rule (a merge of concurrent roster changes signed by an admin under both parents) leaves the concurrent self-retirements unmergeable: no person is admin on both sides |
| `Authority_decide` | `Authority` | `pass` (fast set) | issue #10: suggestions made, revised, adopted, rejected and withdrawn between the honest admin a and owner b, who also moves b's folder. Every decider can adopt; every decision is a decider's or the author's; every version entered through an allowed commit; an honest tree is the catalogue of its history; quiet devices agree (ADR 0051): 34 245 states, 20 829 distinct, 7 s |
| `Authority_faulty` | `Authority` | `pass` (fast set) | the same against a faulty member who signs any change, merge or decision on any parent: 371 002 states, 150 579 distinct, 55 s |
| `Authority_revoke` | `Authority` | `pass` (fast set) | c's laptop, once removed, signs on old parents and adds a key for c. Nothing it signed unseen, and no key it added, counts on any device (ADR 0051): 547 210 states, 188 525 distinct, 66 s |
| `Authority_live` | `Authority` | `pass` (fast set) | `Converge` under fairness of fetches and merges, with the removed laptop signing on old parents: 938 states, 399 distinct, 11 s |
| `Authority_decide4`, `Authority_faulty2`, `Authority_revoke2` | `Authority` | `pass` (full set) | four changes; two faulty commits, with transfers; saves and the removed laptop's merges: 1 178 047, 1 360 807 and 1 645 961 distinct states, 10, 7 and 6 minutes |
| `Authority_proposal` | `Authority` | `violation:DecidersCanAdopt` | deciders under the roster at the proposal's parents (ADR 0051's words). a suggests a change in b's folder, then gives the folder to himself. b still decides, but rule 3 rejects b's adoption (3 states) |
| `Authority_mixed` | `Authority` | `pass` (fast set) | the proposal's own diff under today's roster. It used to fail: a adds the root file, c suggests adding it and a file in F, b decides, and adopting keeps both versions of the root file, which the model took for a change only the admins may make. A version beside the old ones is a conflict copy, a new file that anyone may add at the root, as the code judges it (`AuthorityTrace`): in this world the proposal's diff now gives deciders who can adopt, 240 s on 2 workers |
| `Authority_noforce` | `Authority` | `violation:DecidersCanAdopt` | a suggestion that nobody may make falls to the admins, whose adoption folder rules reject without Force |
| `Authority_fakeadopt` | `Authority` | `violation:AdoptionHoldsContent` | without rule 7, c's clean merge of `main` with a's suggestion marks it accepted with none of its content |
| `Authority_base` | `Authority` | `violation:AuthorisedContent` | proposals counted in a merge's base, without rule 7: a suggestion's content enters with no decider |
| `Authority_first` | `Authority` | `violation:AuthorisedContent` | rule 3 checked against the first accepted parent only: c's merge changes F, which only one parent's roster lets c change |
| `Authority_anyone` | `Authority` | `violation:DecisionsByDeciders` | any member may write a decision: c rejects a's suggestion, which only b may decide |
| `Authority_none` | `Authority` | `violation:RemovedSilenced` | no revocation (04 §4.4 and ADR 0051 alone). After a removes c's laptop, the laptop changes the root file on the genesis, and a's device merges it (5 states) |
| `Authority_tip` | `Authority` | `violation:RemovedSilenced` | 09 §9.7's policy, no merge of a head signed outside `main`'s roster. The removed laptop signs the merge of its change with the removal itself, and b's device, which has not yet seen the removal, fast-forwards to it (6 states) |
| `Authority_tipquiet`, `Authority_tiplive` | `Authority` | `violation:ConvergedWhenQuiet`, `violation:temporal` | the same policy leaves an accepted commit out of every `main` for good |
| `Authority_contentonly` | `Authority` | `violation:RemovalRevokesAdded` | revoking the content but not the keys: the key that the removed laptop added for c stays in the roster |
| `TraceSample_good` | `TraceSample` | `trace:accepted` (fast set) | the pattern of every trace module (13 §13.3) on a toy save-then-capture spec: a log whose values carry quotes, backslashes, nested tuples and records is explained line by line, and its `ASSUME`s check that TLC reads back what the NDJSON held |
| `TraceSample_bad` | `TraceSample` | `trace:rejected:2` | a capture of a path never saved: no behaviour explains line 2, and the verdict names it |
| `RosterTrace_good` | `RosterTrace` | `trace:accepted` (fast set) | a run of pigeon-core's roster fold, written by `crates/pigeon-core/tests/roster_trace.rs` (seed 1): each logged change is one of `Roster`'s `Ops`, and each roster the code computed, at commits and merges, is the model's (13 §13.3) |
| `RosterTrace_bad` | `RosterTrace` | `trace:rejected:7` | the same run with the roster of its last checked step altered: the verdict names line 7 |
| `AdmitTrace_good` | `AdmitTrace` | `trace:accepted` (fast set) | a plan of pigeon-core's `capacity::admit`, written by `crates/pigeon-core/tests/admit_trace.rs` (seed 1): the instance it planned, then each admission or deferral in the code's order, is a behaviour of `Admission` (13 §13.3) |
| `AdmitTrace_bad` | `AdmitTrace` | `trace:rejected:4` | the same plan with its last decision reversed: the verdict names line 4 |
| `AuthorityTrace_good` | `AuthorityTrace` | `trace:accepted` (fast set) | a history judged by pigeon-core's `commit::judge`, written by `crates/pigeon-core/tests/authority_trace.rs` (seed 1): each commit, on one accepted parent and signed by any of four devices with or without a Force, writes versions with roster edits (`F` transferred, device 4 added, a device removed), or rejects or withdraws a proposal the code keeps, or adopts one (as `proposal::adoption` writes it, or with a change dropped or one more), and gets `Judge`'s verdict in `Authority` (04 §4.4, §4.5, 13 §13.3) |
| `AuthorityTrace_bad` | `AuthorityTrace` | `trace:rejected:21` | the same history with the verdict on its last commit reversed: the verdict names line 21 |
| `CatalogueTrace_good` | `CatalogueTrace` | `trace:accepted` (fast set) | a history of three devices (Alice, Bob, Paul) merged by pigeon-core's `catalogue::Engine`, written by `crates/pigeon-core/tests/catalogue_trace.rs` (seed 1): saves, deletions, renames, *Keep mine*, *Keep both*, edits of copies, suggestions in Alice's folder and pulls. Each commit, merges included, gets `Record`'s verdict, and an accepted one `View`'s catalogue (each version, where it is shown and the commit it represents) and `Open`'s conflicts (05 §5.2–5.3, 13 §13.3) |
| `CatalogueTrace_bad` | `CatalogueTrace` | `trace:rejected:28` | the same history with one version lost from the catalogue of its last accepted commit: the verdict names line 28 |
| `CaptureInstall_own` | `CaptureInstall` | `pass` (fast set) | issue #1, [07 §7.5](../../docs/spec/07-capture-and-install.md): capture and install of one path while a person writes in place, writes through a handle opened earlier, replaces atomically, creates and deletes at any instant, also while `pigeond` is down; the group changes or deletes the path; one crash. Nothing saved is lost (NoLoss); when idle, the path holds what the index says or the person's content (Journal); the index never calls a changed file clean (IndexHonest); what install moves away was captured (CaptureFirst), is the file it expected, and is written by nobody afterwards (NoOverwrite); a deletion the person made stays (DeletionKept); under fairness every save is captured (SavesCaptured) and the path ends at `current(p)` with no divergence and no kept aside (Converges). Own folder, two person actions: 122 560 states, 61 698 distinct, 9 s on 1 worker |
| `CaptureInstall_other` | `CaptureInstall` | `pass` (fast set) | the same in another person's folder: saves become suggestions, which are accepted or rejected, then resolved (*Take the group's version*, *Suggest again*), also while new saves race the choice: 1 240 787 states, 612 281 distinct, 6 s |
| `CaptureInstall_crash2` | `CaptureInstall` | `pass` (fast set) | two crashes, one during recovery: a second replay decides as the first (ADR 0036's law): 360 940 states, 171 214 distinct, 4 s |
| `CaptureInstall_hidden` | `CaptureInstall` | `pass` (fast set) | writers that escape the writer scan and close within the keep period: nothing is lost, though NoOverwrite does not hold for them ([07 §7.5](../../docs/spec/07-capture-and-install.md)) |
| `CaptureInstall_own3`, `CaptureInstall_other3`, `CaptureInstall_otherlive` | `CaptureInstall` | `pass` (full set) | three person actions in the own folder (1 048 763 distinct states, 9 s); three in another's folder with escaping writers (21 968 788 distinct states, 3 minutes); liveness in another's folder (612 281 distinct states, 72 s) |
| `CaptureInstall_rec07` | `CaptureInstall` | `violation:DeletionKept` | the recovery table of 07 before [07 §7.5](../../docs/spec/07-capture-and-install.md) journalled the proof, which decides from *p* alone: the person deletes the new file after step 7, `pigeond` is killed before step 8, and recovery puts the old file back |
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

## Notes on the models

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
([09 §9.4](../../docs/spec/09-network-and-joining.md)). There the server answers `CHECKPRESENT` with `inAnnex`
(`P2P/Annex.hs`), which ignores the removal lock, and a drop whose
`LOCKCONTENT` fails counts that answer (`Annex/NumCopies.hs`). The 8-step
counterexample of `AnnexDrop_p2p` (two custodians begin, each `LOCKCONTENT`
meets the other's removal lock, each `CHECKPRESENT` says present, both remove)
is E23's race with real git-annex: with numcopies 1 no copy is left, with
numcopies = mincopies = 2 on three custodians one is. Behind pigeon's probe
([09 §9.4](../../docs/spec/09-network-and-joining.md)) the check is `"safe"` again, and E23's same races keep every copy.
`AnnexDrop_log` is E5: counting a stale log drops the last copies, while the
design checks each copy, as git-annex does (E5, and E23's stale log).
`AnnexDrop_trusted` is E23's `trust`: a trusted custodian counts unasked from
the log, so pigeon never sets `trust` ([03 §3.5](../../docs/spec/03-repository-and-roles.md)).
Laptops are
not modelled: they are untrusted and never counted, and their drops leave the
custody copies alone. Nor are remote drops (pigeon's filter refuses `REMOVE`,
E13), or the forced drops of purge and Drop, which skip the check (purge has
the `Purge` model).

**Storage implementation boundaries** (07 §7.3, 07 §7.5, 08 §8.3).
`CaptureTransaction` separates staging, commit failure and rollback: a clean
staged index is not evidence of a committed save. Its unguarded variant
violates `CapturedBeforeDisplacement`. `RecoveredNames` runs two recovery
batches with journal sequence reuse: a replacing destination violates
`EverySaveKept`. `Retention` distinguishes historical references from the
current tree; its current-only enumerator violates `Complete`.

`StorageTrace` checks observations of the real filesystem driver at those
boundaries and at aside release. `scripts/check-storage-traces` requires
all four actions, rejects a corrupted observation of each, and fails on
missing logs or fields. These are boundary predicates, not a complete
transition-by-transition refinement of `CaptureInstall`; its full daemon
trace remains unregistered. Increasing that model's bounds alone would not
cover staged capture failures or the physical recovery-name namespace.

**`CaptureInstall`** ([07 §7.7](../../docs/spec/07-capture-and-install.md)) models one path on
one laptop. Every write makes a new content, so a content names the save that
made it. A file's stat is its inode and mtime, and a write in the same clock
tick keeps the mtime, as in racy git. Install's steps 1 to 3 are one action;
steps 4 to 9, `Crash`, `Recover` (forward or back, [07 §7.5](../../docs/spec/07-capture-and-install.md)) and its end are
one action each. Capture takes a quiet path, unchanged since the last tick.
The person acts at any instant, pigeond being up or down; the group
(`NewVersion`, `Decide`) changes `current(p)` and judges suggestions.
Each negative configuration removes one safeguard (`Off`) and must fail. The
model found that the recovery table, which decided from *p* alone, undoes a
finished deletion and a deletion the person made after step 7; [07 §7.5](../../docs/spec/07-capture-and-install.md)
journals the proof and rolls proved entries forward. Its first repair, which
decided from the staged file, failed the double replay of
`CaptureInstall_crash2`. Not modelled: several paths and packages, thin hard
links and their link count, the resolution of a recovered version (it is
captured; the model checks only that it is kept), and the scan's retry period
(a deferred path is retried once its writer has closed).

**`Repair`** ([08 §8.4](../../docs/spec/08-custody-loops.md)) takes keys of one unit each.
Each custodian has room above its reserve, and the order in which
`sizebalanced` and overflow rank it. A get copies only
an intact object, since git-annex verifies it on arrival. A get counts as a
copy only once synced (ADR 0046). A verified drop is one step: `AnnexDrop`
shows the drop itself is safe behind the probe of [09 §9.4](../../docs/spec/09-network-and-joining.md). Assumption F
(02 §2.2) bounds the faults: within a repair epoch fewer than K custodians
lose or corrupt a key. The model counts the epoch on intact copies, so a rot opens
it when it happens. Each negative configuration removes one safeguard
(`Off`) and must fail. The model found two flaws, fixed by ADR 0046, [09 §9.4](../../docs/spec/09-network-and-joining.md). A
rotten copy can prove a custodian's drop, since git-annex's lock checks only
presence. A custodian at its reserve still wants its share, so overflow,
counting it as taking the key, never chose another custodian. Not
modelled: keys of different sizes; `maxsize` apart from real room; stale
status reports ([08 §8.9](../../docs/spec/08-custody-loops.md) shows that they cost space, never safety); the
seven days before an admin removes a silent custodian.

**`Evict`** ([08 §8.5](../../docs/spec/08-custody-loops.md)) models one laptop with two
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
when it runs ([09 §9.4](../../docs/spec/09-network-and-joining.md)). From git it takes that `git sparse-checkout set`
unlinks a hidden file that matches the index, and that git clears the bit
of a file present on disk, so capture sees a file made in a hidden folder.
Each negative configuration removes one safeguard (`Off`) and must fail. The
model and E24 found that hiding with git's unlink loses a write through an
earlier handle, that a drop's guard must count staged and moved-in files and
cover the drop of old versions, and that a keep can show a version main has
replaced; [08 §8.5](../../docs/spec/08-custody-loops.md) fixes all three. Not modelled: install's own steps (the
`CaptureInstall` model), packages, divergences and conflicts, and a writer
that escapes the scan.

**`Authority`** ([04 §4.3–4.5](../../docs/spec/04-roster-and-authority.md),
[09 §9.7](../../docs/spec/09-network-and-joining.md)) models acceptance with three persons:
the admin a, b who owns a folder F, and the member c. There are two paths: a
file in F, and a file at the root (add-only, the admins are its stewards;
a person who alone wrote it changes it too, ADR 0070: each commit records
the person it writes as, its proposal's author for an adoption).
The devices of a and b are honest `pigeond`s: they capture, revise, adopt,
reject and withdraw suggestions, give F to someone else, remove c's devices,
fetch and merge. c's devices belong to an attacker who signs any change,
merge or decision on any parent it knows, and may add a key for c. That is a
faulty member, or c's stolen laptop once a has removed it.

The verdict of each commit is computed by the rules of 04 §4.4 (1–4 and 7)
when the commit is written. A path's value is the set of versions that the
newest changes of that path wrote: a conflict keeps several. This is the
abstraction of the catalogue that `models/catalogue` checks in full
([05 §5.2](../../docs/spec/05-catalogue-and-names.md)). Increasing commit ids give the canonical order; `Roster` checks
the fold's order and its edits. A value that keeps every version of the
old one only adds to the path: in the code the new version is a conflict
copy, a new file, which anyone may add at the root. A decision is never
changed, and an admin may adopt with Force what its deciders would; rule 7
holds a deletion unless the base changed the path since. `AuthorityTrace`
found these three, where the code was right.

The model found two things:

- **A removed device keeps signing.** On an old parent, the removed laptop's
  commits are valid. 09 §9.7's policy of not merging them lets them in
  through a device that has not yet seen the removal (`Authority_tip`). It
  also keeps accepted commits out of `main` (`Authority_tipquiet`). ADR 0051
  revokes them instead.
- **Deciders who cannot decide.** ADR 0051 judges deciders on the
  proposal's parents, so some deciders cannot adopt (`Authority_proposal`;
  on its own diff, `Authority_mixed` failed too until a conflict copy
  counted as a new file). Admins cannot adopt without
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

**`Catalogue`** ([05 §5.2](../../docs/spec/05-catalogue-and-names.md)) states
the deterministic merge over abstract slots, the oracle
`models/catalogue/catalogue.py` without its names: a slot is `<<p>>` for a
path, and `Append(x, o)` for the copy of slot `x` that commit `o`
represents, so names never collide and folding is left to the code. Events
are contributions and the conflict events they freeze, named by keys so
that one freeze reached twice is one event; `Heads`, `Reps`, `Shown` and
`View` are steps 2–5, `Freeze` step 6 and `Open` the open conflicts of
§5.3. It has no `Next` of its own: `CatalogueTrace` replays the code's
commits through `Record`, which judges each against the catalogue of its
accepted parents (clean, a proposal when its person may not change a slot,
or a contribution). Run against a build of `catalogue::Engine` that never
freezes, it rejects the logs (line 9 to 15 of seeds 2, 6 and 7). A version
that a conflict event already put at one of its slot's copies is not shown
again as that slot's copy (`Copied`): seeds 24 and 197 of models-full's 200
found the code showing it twice, under a second name ` 2`, once a copy was
edited or a re-frozen side met the other.
