--------------------------- MODULE CaptureInstall ---------------------------
(* Capture and install of one path p on one laptop (spec 07 §7.3-7.6, ADR
   0034). A person and their applications act at any instant, also
   while pigeond is down: write in place or through a handle opened earlier,
   replace atomically, create, delete. The group changes current(p). pigeond
   captures quiet changes, refreshes the index, installs current(p) by the
   steps of 07 §7.5, releases kept asides, crashes and recovers from its
   journal. Contents are unique per write; an inode's stat is (inode, mtime),
   and a write in the same clock tick keeps the mtime (racy git). The
   CaptureInstall_own, _other, _crash2, _hidden, _own3, _other3 and
   _otherlive check the design; the others are negative tests. Off names the
   safeguards removed: "check", "scan", "racy", "quiet", "journal",
   "noreplace", "hash", "keep", and "recovery" (the recovery table of 07
   before spec 07 §7.5 journalled the proof). Hidden: some writers escape the
   writer scan (another user's process, a mapping macOS cannot see); they
   close within the keep period unless "keep" is off. *)
EXTENDS Naturals, FiniteSets
CONSTANTS MaxAct, MaxGroup, MaxCrash, MaxRes, MaxIno, Allowed, Hidden, Off
Absent == 0
NoStat == <<0, 0>>
Inodes == 1..MaxIno
MaxC == 1 + MaxGroup + MaxAct
Contents == 1..MaxC
None == [k |-> "none", key |-> Absent, st |-> NoStat]

VARIABLES
  fc, fm,     \* content (0 = none) and mtime of each inode
  fresh,      \* inodes written during the current clock tick
  at,         \* the inode at p, at the aside and at the stage name, or 0
  kept,       \* kept asides: [i, e], e the content the aside should hold
  orph,       \* files left in .git/pigeon/ that no journal names
  hnd, hid,   \* inodes open for writing: seen by the writer scan, or hidden
  punl,       \* inodes pigeon unlinked
  idx,        \* index entry of p: [key, st, racy]
  cur,        \* current(p), the version of p in main
  store,      \* contents in a commit (accepted or proposal) or captured
  saved,      \* contents the person wrote
  dropped,    \* contents the person discarded (overwritten, replaced, deleted)
  div,        \* divergence of p: [k: "none" | "waiting" | "conflict", key, st]
  pc,         \* pigeond: "idle", an install step, "down" after a crash, "rec"
  jr,         \* install journal entry, persisted while open
  deferred,   \* the writer scan found a writer: the path waits (in-use)
  who,        \* ghost: who last set the name p, "pigeon" or "person"
  pdel,       \* ghost: the person deleted the version capture last took from p,
              \* and capture has not seen the deletion yet
  gap,        \* ghost: inodes the person created at p while install had moved p aside
  disp,       \* ghost: [i, e, c] for each file install proved and moved away
  n           \* counters
vars == <<fc, fm, fresh, at, kept, orph, hnd, hid, punl, idx, cur, store, saved,
          dropped, div, pc, jr, deferred, who, pdel, gap, disp, n>>

Stat(i) == IF i = 0 THEN NoStat ELSE <<i, fm[i]>>
PathIs(k) == IF k = Absent THEN at.p = 0 ELSE at.p # 0 /\ fc[at.p] = k
Linked == ({at.p, at.aside, at.stage} \cup {x.i : x \in kept} \cup orph) \ {0}
(* update-index --refresh: git records the stat only if the file hashes to the
   key; the entry is racily clean if the file was written in this tick. *)
Record(k) == [key |-> k, racy |-> at.p \in fresh,
              st |-> IF k # Absent /\ PathIs(k) THEN Stat(at.p) ELSE NoStat]
Clean == \/ at.p = 0 /\ idx.key = Absent
         \/ /\ at.p # 0 /\ idx.key # Absent /\ idx.st = Stat(at.p)
            /\ (~idx.racy \/ "racy" \in Off)
Keep(i, e) == IF i = 0 THEN kept ELSE kept \cup {[i |-> i, e |-> e]}
Closed == [jr EXCEPT !.open = FALSE]

Init == /\ fc = [i \in Inodes |-> IF i = 1 THEN 1 ELSE 0]
        /\ fm = [i \in Inodes |-> 0]
        /\ fresh = {} /\ at = [p |-> 1, aside |-> 0, stage |-> 0]
        /\ kept = {} /\ orph = {} /\ hnd = {} /\ hid = {} /\ punl = {}
        /\ idx = [key |-> 1, st |-> <<1, 0>>, racy |-> FALSE]
        /\ cur = 1 /\ store = {1} /\ saved = {} /\ dropped = {} /\ div = None
        /\ pc = "idle" /\ deferred = FALSE /\ who = "pigeon" /\ pdel = FALSE /\ gap = {}
        /\ jr = [open |-> FALSE, new |-> 0, exp |-> 0, st |-> NoStat, tmp |-> 0, res |-> FALSE,
                proved |-> FALSE]
        /\ disp = {}
        /\ n = [act |-> 0, grp |-> 0, crash |-> 0, res |-> 0, ino |-> 1, cont |-> 1]

----------------------------------------------------------------------------
(* The person and their applications. *)
(* The trace adapter can also supply bytes seen earlier (a restore or an
   identical save). The bounded design model keeps its fresh-value cases. *)
WriteValue(i, c) ==   \* in place at p, or through a handle opened earlier
  /\ n.act < MaxAct
  /\ fc' = [fc EXCEPT ![i] = c]
  /\ fm' = [fm EXCEPT ![i] = IF i \in fresh THEN @ ELSE @ + 1]
  /\ fresh' = fresh \cup {i}
  /\ saved' = saved \cup {c}
  \* a write into a file the person deleted is the person's own loss
  /\ dropped' = dropped \cup {fc[i]} \cup (IF i \notin Linked \cup punl THEN {c} ELSE {})
  /\ who' = IF i = at.p THEN "person" ELSE who
  /\ pdel' = (pdel /\ i # at.p) /\ UNCHANGED gap
  /\ n' = [n EXCEPT !.act = @ + 1, !.cont = @ + 1]
  /\ UNCHANGED <<at, kept, orph, hnd, hid, punl, idx, cur, store, div, pc, jr, deferred, disp>>
Write(i) == WriteValue(i, n.cont + 1)
WriteAtP == at.p # 0 /\ Write(at.p)
WriteHandle == \E i \in hnd \cup hid : Write(i)
ReplaceValue(c) ==    \* write a new file and rename it over p, or create p
  LET i == n.ino + 1 IN
  /\ n.act < MaxAct /\ n.ino < MaxIno
  /\ fc' = [fc EXCEPT ![i] = c] /\ fresh' = fresh \cup {i}
  /\ at' = [at EXCEPT !.p = i]
  /\ saved' = saved \cup {c}
  /\ dropped' = IF at.p # 0 THEN dropped \cup {fc[at.p]} ELSE dropped
  /\ who' = "person" /\ pdel' = FALSE
  /\ gap' = IF (at.p = 0 /\ who = "pigeon" /\ pc # "idle") \/ at.p \in gap THEN gap \cup {i} ELSE gap
  /\ n' = [n EXCEPT !.act = @ + 1, !.ino = @ + 1, !.cont = @ + 1]
  /\ UNCHANGED <<fm, kept, orph, hnd, hid, punl, idx, cur, store, div, pc, jr, deferred, disp>>
Replace == ReplaceValue(n.cont + 1)
Delete ==
  /\ n.act < MaxAct /\ at.p # 0
  /\ at' = [at EXCEPT !.p = 0] /\ dropped' = dropped \cup {fc[at.p]}
  /\ who' = "person" /\ pdel' = ((IF div.k = "none" THEN idx.key ELSE div.key) # Absent /\ at.p \notin gap) /\ UNCHANGED gap
  /\ n' = [n EXCEPT !.act = @ + 1]
  /\ UNCHANGED <<fc, fm, fresh, kept, orph, hnd, hid, punl, idx, cur, store, saved, div, pc, jr, deferred, disp>>
Open ==
  /\ n.act < MaxAct /\ at.p # 0
  /\ \/ hnd' = hnd \cup {at.p} /\ UNCHANGED hid
     \/ Hidden /\ hid' = hid \cup {at.p} /\ UNCHANGED hnd
  /\ n' = [n EXCEPT !.act = @ + 1]
  /\ UNCHANGED <<fc, fm, fresh, at, kept, orph, punl, idx, cur, store, saved, dropped, div, pc, jr, deferred, who, pdel, gap, disp>>
Close == \E i \in hnd \cup hid :
  /\ hnd' = hnd \ {i} /\ hid' = hid \ {i}
  /\ UNCHANGED <<fc, fm, fresh, at, kept, orph, punl, idx, cur, store, saved, dropped, div, pc, jr, deferred, who, pdel, gap, disp, n>>
Tick ==
  /\ fresh # {} /\ fresh' = {}
  /\ UNCHANGED <<fc, fm, at, kept, orph, hnd, hid, punl, idx, cur, store, saved, dropped, div, pc, jr, deferred, who, pdel, gap, disp, n>>

(* The group: another version of p (or its deletion) is accepted elsewhere;
   a waiting suggestion of p then becomes a conflict. *)
VersionValueWith(v, notify) ==
  /\ n.grp < MaxGroup /\ v # cur
  /\ cur' = v /\ store' = store \cup ({v} \ {Absent})
  /\ div' = IF notify /\ div.k = "waiting" THEN [div EXCEPT !.k = "conflict"] ELSE div
  /\ n' = [n EXCEPT !.grp = @ + 1, !.cont = @ + 1]
  /\ UNCHANGED <<fc, fm, fresh, at, kept, orph, hnd, hid, punl, idx, saved, dropped, pc, jr, deferred, who, pdel, gap, disp>>
VersionValue(v) == VersionValueWith(v, TRUE)
NewVersion == \E v \in {n.cont + 1, Absent} : VersionValue(v)
(* The deciders accept or reject the person's suggestion; on acceptance only
   the index changes. *)
Decide ==
  /\ pc = "idle" /\ div.k = "waiting"
  /\ \/ cur' = div.key /\ idx' = Record(div.key) /\ div' = None
     \/ div' = [div EXCEPT !.k = "conflict"] /\ UNCHANGED <<cur, idx>>
  /\ UNCHANGED <<fc, fm, fresh, at, kept, orph, hnd, hid, punl, store, saved, dropped, pc, jr, deferred, who, pdel, gap, disp, n>>
(* Keep mine (own folder) or suggest again. *)
ResolveMine ==
  /\ pc = "idle" /\ div.k = "conflict" /\ n.res < MaxRes
  /\ IF Allowed THEN cur' = div.key /\ idx' = Record(div.key) /\ div' = None
     ELSE div' = [div EXCEPT !.k = "waiting"] /\ UNCHANGED <<cur, idx>>
  /\ n' = [n EXCEPT !.res = @ + 1]
  /\ UNCHANGED <<fc, fm, fresh, at, kept, orph, hnd, hid, punl, store, saved, dropped, pc, jr, deferred, who, pdel, gap, disp>>

----------------------------------------------------------------------------
(* pigeond. Capture takes a quiet path (unchanged for a tick, 07 §7.2) whose
   file differs from the index; capture and install never run at once. *)
CaptureWith(allowed) ==
  LET i == at.p  c == IF i = 0 THEN Absent ELSE fc[i]  st == Stat(i) IN
  /\ pc = "idle" /\ (i = 0 \/ i \notin fresh \/ "quiet" \in Off)
  \* a diverging path is compared with the person's side, others with the index
  /\ IF div.k = "none" THEN ~Clean /\ c # idx.key ELSE c # div.key
  /\ store' = store \cup ({c} \ {Absent}) /\ pdel' = FALSE /\ UNCHANGED gap
  /\ IF div.k # "none"      \* the person's side; it ends if it matches the index again
     THEN div' = (IF c = idx.key THEN None ELSE [div EXCEPT !.key = c, !.st = st]) /\ UNCHANGED <<cur, idx>>
     ELSE IF idx.key # cur     \* based on a version main has left: concurrent
     THEN div' = [k |-> "conflict", key |-> c, st |-> st] /\ UNCHANGED <<cur, idx>>
     ELSE IF allowed THEN cur' = c /\ idx' = Record(c) /\ UNCHANGED div
     ELSE div' = [k |-> "waiting", key |-> c, st |-> st] /\ UNCHANGED <<cur, idx>>
  /\ UNCHANGED <<fc, fm, fresh, at, kept, orph, hnd, hid, punl, saved, dropped, pc, jr, deferred, who, disp, n>>
Capture == CaptureWith(Allowed)
Refresh ==
  /\ pc = "idle" /\ at.p # 0 /\ fc[at.p] = idx.key /\ idx # Record(idx.key)
  /\ idx' = Record(idx.key)
  /\ UNCHANGED <<fc, fm, fresh, at, kept, orph, hnd, hid, punl, cur, store, saved, dropped, div, pc, jr, deferred, who, pdel, gap, disp, n>>

(* Install, 07 §7.5: steps 1-3 (content, new file, journal) in one action. *)
StageWith(new, exp, st, res, use, discardDeletion) ==
  LET t == IF new = Absent THEN 0 ELSE n.ino + 1 IN
  /\ new = Absent \/ n.ino < MaxIno
  /\ fc' = IF t = 0 THEN fc ELSE [fc EXCEPT ![t] = new]
  /\ fresh' = fresh \cup ({t} \ {0})
  /\ at' = [at EXCEPT !.stage = t]
  /\ jr' = [open |-> "journal" \notin Off, new |-> new, exp |-> exp, st |-> st, tmp |-> t,
            res |-> res, proved |-> FALSE]
  /\ pc' = "begun" /\ pdel' = IF discardDeletion THEN FALSE ELSE pdel
  /\ n' = [n EXCEPT !.ino = IF t = 0 THEN @ ELSE @ + 1, !.res = IF use THEN @ + 1 ELSE @]
  /\ UNCHANGED <<fm, kept, orph, hnd, hid, punl, idx, cur, store, saved, dropped, div, deferred, who, gap, disp>>
Stage(new, exp, st, res, use) == StageWith(new, exp, st, res, use, FALSE)
Start == pc = "idle" /\ ~deferred /\ div.k = "none" /\ idx.key # cur /\ Clean
         /\ Stage(cur, idx.key, idx.st, FALSE, FALSE)
(* Take the group's version: the proof compares with the person's version in
   the divergence table (07 §7.6). *)
(* A choice made while a newer save is not yet captured (Pending) is checked
   and fails; such choices are bounded by MaxRes, so that capture is not
   starved by a person who keeps choosing. *)
Pending == ~(PathIs(div.key) /\ Stat(at.p) = div.st)
TakeGroup ==
  /\ pc = "idle" /\ ~deferred /\ div.k = "conflict" /\ (Pending => n.res < MaxRes)
  /\ Stage(cur, div.key, div.st, TRUE, Pending)

Skip == /\ at' = [at EXCEPT !.stage = 0] /\ jr' = Closed /\ pc' = "idle"
        /\ UNCHANGED <<kept, who, deferred>>
MoveAside ==
  /\ pc = "begun"   \* step 4
  /\ IF jr.exp = Absent \/ at.p = 0
     THEN IF jr.exp = Absent /\ at.p = 0
          THEN pc' = "checked" /\ jr' = [jr EXCEPT !.proved = TRUE]     \* a creation
               /\ UNCHANGED <<at, kept, who, deferred>>
          ELSE Skip                    \* p was created or deleted: capture first
     ELSE /\ at' = [at EXCEPT !.aside = at.p, !.p = 0] /\ who' = "pigeon"
          /\ pc' = "aside" /\ UNCHANGED <<kept, jr, deferred>>
  /\ UNCHANGED <<fc, fm, fresh, orph, hnd, hid, punl, idx, cur, store, saved, dropped, div, pdel, gap, disp, n>>
(* The proof failed: a person saved or is saving p. The aside goes back with
   RENAME_NOREPLACE, or is kept and hashed at release if p was recreated. *)
Fail(busy) ==
  /\ IF at.p = 0
     THEN at' = [p |-> at.aside, aside |-> 0, stage |-> 0] /\ who' = "pigeon" /\ UNCHANGED kept
     ELSE at' = [at EXCEPT !.aside = 0, !.stage = 0] /\ kept' = Keep(at.aside, jr.exp) /\ UNCHANGED who
  /\ jr' = Closed /\ pc' = "idle" /\ deferred' = busy
Scan ==
  /\ pc = "aside"        \* step 5, the writer scan
  /\ IF at.aside \in hnd /\ "scan" \notin Off THEN Fail(TRUE)
     ELSE pc' = "scanned" /\ UNCHANGED <<at, kept, who, jr, deferred>>
  /\ UNCHANGED <<fc, fm, fresh, orph, hnd, hid, punl, idx, cur, store, saved, dropped, div, pdel, gap, disp, n>>
CheckWith(hash) ==
  /\ pc = "scanned"     \* step 6: the journalled stat
  /\ IF (Stat(at.aside) = jr.st /\ (~hash \/ fc[at.aside] = jr.exp)) \/ "check" \in Off
     THEN /\ pc' = "checked" /\ disp' = disp \cup {[i |-> at.aside, e |-> jr.exp, c |-> fc[at.aside]]}
          /\ jr' = [jr EXCEPT !.proved = TRUE]      \* journalled: `proved` (spec 07 §7.5)
          /\ UNCHANGED <<at, kept, who, deferred>>
     ELSE Fail(FALSE) /\ UNCHANGED disp
  /\ UNCHANGED <<fc, fm, fresh, orph, hnd, hid, punl, idx, cur, store, saved, dropped, div, pdel, gap, n>>
Check == CheckWith(FALSE)
RenameIn ==
  /\ pc = "checked"  \* step 7, RENAME_NOREPLACE
  /\ IF jr.new = Absent
     THEN pc' = "renamed" /\ UNCHANGED <<at, kept, who, jr, punl>>
     ELSE IF at.p = 0 \/ "noreplace" \in Off
     THEN /\ at' = [at EXCEPT !.p = at.stage, !.stage = 0] /\ who' = "pigeon"
          /\ punl' = punl \cup ({at.p} \ {0}) /\ pc' = "renamed" /\ UNCHANGED <<kept, jr>>
     ELSE /\ at' = [at EXCEPT !.aside = 0, !.stage = 0] /\ kept' = Keep(at.aside, jr.exp)
          /\ jr' = Closed /\ pc' = "idle" /\ UNCHANGED <<who, punl>>    \* raced
  /\ UNCHANGED <<fc, fm, fresh, orph, hnd, hid, idx, cur, store, saved, dropped, div, deferred, pdel, gap, disp, n>>
Index ==
  /\ pc = "renamed"     \* step 8
  /\ idx' = Record(jr.new) /\ div' = (IF jr.res THEN None ELSE div) /\ pc' = "indexed"
  /\ UNCHANGED <<fc, fm, fresh, at, kept, orph, hnd, hid, punl, cur, store, saved, dropped, jr, deferred, who, pdel, gap, disp, n>>
Done ==
  /\ pc = "indexed"      \* step 9: the aside is kept
  /\ kept' = Keep(at.aside, jr.exp) /\ at' = [at EXCEPT !.aside = 0] /\ jr' = Closed /\ pc' = "idle"
  /\ UNCHANGED <<fc, fm, fresh, orph, hnd, hid, punl, idx, cur, store, saved, dropped, div, deferred, who, pdel, gap, disp, n>>
(* A kept aside is released at a later full scan if nobody writes it and it
   still holds what it should; otherwise it is captured as a recovered version. *)
Release == \E x \in kept :
  /\ pc = "idle" /\ x.i \notin hnd /\ (x.i \notin hid \/ "keep" \in Off)
  /\ kept' = kept \ {x} /\ punl' = punl \cup {x.i}
  /\ store' = IF fc[x.i] = x.e \/ "hash" \in Off THEN store ELSE store \cup {fc[x.i]}
  /\ UNCHANGED <<fc, fm, fresh, at, orph, hnd, hid, idx, cur, saved, dropped, div, pc, jr, deferred, who, pdel, gap, disp, n>>
Retry ==
  /\ deferred /\ (at.p = 0 \/ at.p \notin hnd) /\ deferred' = FALSE
  /\ UNCHANGED <<fc, fm, fresh, at, kept, orph, hnd, hid, punl, idx, cur, store, saved, dropped, div, pc, jr, who, pdel, gap, disp, n>>

(* kill -9 at any instant; files, the index and the journal persist. *)
Crash ==
  /\ pc # "down" /\ n.crash < MaxCrash /\ pc' = "down" /\ deferred' = FALSE
  /\ n' = [n EXCEPT !.crash = @ + 1]
  /\ UNCHANGED <<fc, fm, fresh, at, kept, orph, hnd, hid, punl, idx, cur, store, saved, dropped, div, jr, who, pdel, gap, disp>>
(* Recovery at start, before any capture (spec 07 §7.5). An entry whose proof is
   journalled rolls forward: step 7 if its new file is still at its stage name
   and p is free (else it raced), then step 8. Any other entry rolls back: the
   aside goes back with NOREPLACE, or is kept and hashed at release if p is
   another file. The entry ends before its staged file is removed, so a second
   replay decides the same. With "recovery" off, the table of 07 before the
   journal recorded the proof decides from p alone: forward only if p has the
   new file's inode. *)
StepSeven == jr.new # Absent /\ at.stage # jr.tmp     \* the new file left its stage name
Forward == IF "recovery" \in Off THEN at.p # 0 /\ at.p = jr.tmp ELSE jr.proved
Recover ==
  /\ pc = "down"
  /\ IF ~jr.open
     THEN /\ orph' = orph \cup ({at.aside} \ {0})    \* nobody knows this aside
          /\ at' = [at EXCEPT !.aside = 0, !.stage = 0] /\ pc' = "idle"
          /\ UNCHANGED <<who, disp>>
     ELSE /\ IF Forward
             THEN IF jr.new # Absent /\ ~StepSeven /\ at.p = 0
                  THEN at' = [at EXCEPT !.p = at.stage, !.stage = 0] /\ who' = "pigeon" /\ UNCHANGED disp
                  ELSE UNCHANGED <<at, who, disp>>
             ELSE IF at.aside # 0 /\ at.p = 0
             THEN /\ at' = [at EXCEPT !.p = at.aside, !.aside = 0] /\ who' = "pigeon"
                  /\ disp' = {d \in disp : d.i # at.aside}     \* put back: not moved away
             ELSE UNCHANGED <<at, who, disp>>
          /\ pc' = "rec" /\ UNCHANGED orph
  /\ UNCHANGED <<fc, fm, fresh, kept, hnd, hid, punl, idx, cur, store, saved, dropped, div, jr, deferred, pdel, gap, n>>
(* The entry ends (step 8 first if rolled forward); then the staged file is
   removed and the aside kept until release. *)
RecoverEnd ==
  /\ pc = "rec"
  /\ IF Forward /\ (StepSeven \/ jr.new = Absent)
     THEN idx' = Record(jr.new) /\ div' = (IF jr.res THEN None ELSE div)
     ELSE UNCHANGED <<idx, div>>
  /\ kept' = Keep(at.aside, jr.exp) /\ at' = [at EXCEPT !.aside = 0, !.stage = 0]
  /\ jr' = Closed /\ pc' = "idle"
  /\ UNCHANGED <<fc, fm, fresh, orph, hnd, hid, punl, cur, store, saved, dropped, deferred, who, pdel, gap, disp, n>>

Person == WriteAtP \/ WriteHandle \/ Replace \/ Delete \/ Open \/ Close \/ Tick
Pigeon == Capture \/ Refresh \/ Start \/ TakeGroup \/ MoveAside \/ Scan \/ Check \/ RenameIn
          \/ Index \/ Done \/ Release \/ Retry \/ Crash \/ Recover \/ RecoverEnd
Next == Person \/ Pigeon \/ NewVersion \/ Decide \/ ResolveMine
Spec == Init /\ [][Next]_vars
Fairness == /\ WF_vars(Capture) /\ WF_vars(Refresh) /\ WF_vars(Start) /\ WF_vars(TakeGroup)
            /\ WF_vars(MoveAside) /\ WF_vars(Scan) /\ WF_vars(Check) /\ WF_vars(RenameIn)
            /\ WF_vars(Index) /\ WF_vars(Done) /\ WF_vars(Release) /\ WF_vars(Retry)
            /\ WF_vars(Recover) /\ WF_vars(RecoverEnd) /\ WF_vars(Decide)
            /\ WF_vars(Close) /\ WF_vars(Tick)
LiveSpec == Spec /\ Fairness

----------------------------------------------------------------------------
(* NoLoss (spec 07 §7.5): every content the person wrote is captured, at a name
   pigeon keeps, or discarded by the person. *)
NoLoss == \A c \in saved : c \in store \/ c \in dropped \/ \E i \in Linked : fc[i] = c
(* INV-Journal: when idle, a name pigeon last set holds what the index says or
   the person's content. *)
Journal == pc = "idle" /\ who = "pigeon" => PathIs(idx.key) \/ (at.p # 0 /\ fc[at.p] \in saved)
(* E19: git never calls a changed file clean. *)
IndexHonest == Clean => PathIs(idx.key)
(* INV-Capture: what install moves away for good is in a commit. *)
CaptureFirst == \A d \in disp : d.c \in store
(* INV-NoOverwrite: install moves away only the file it expects (what it last
   installed, or the version the person chose to discard), and nobody writes
   that file afterwards. *)
NoOverwrite == \A d \in disp : d.c = d.e /\ fc[d.i] = d.e
(* A deletion the person made stays until capture has seen it. *)
DeletionKept == pdel => at.p = 0 \/ who = "person"
(* Liveness under Fairness: every save is captured, and once the person and the
   group stop, p ends at current(p), with no divergence and no kept aside. *)
SavesCaptured == \A c \in Contents : (c \in saved /\ c \notin dropped) ~> (c \in store \/ c \in dropped)
Converges == <>[](pc = "idle" /\ div.k = "none" /\ idx.key = cur /\ PathIs(cur) /\ kept = {})
=============================================================================
