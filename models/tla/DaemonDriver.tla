--------------------------- MODULE DaemonDriver ----------------------------
(* The same journal protocol as CaptureInstall, with the daemon's other
   uses made explicit: free keeps the pointer and sets skip-worktree; keep
   clears that bit; thinning replaces a copy by a link to the same bytes;
   restore deliberately leaves the pointer for the next capture. These
   operations must not masquerade as CaptureInstall.Start/Index/Done.
   Protection and custody/drop decisions remain the scope of Evict and
   AnnexDrop; this model checks the filesystem/journal transitions. *)
EXTENDS CaptureInstall
VARIABLES hidden, operation
driverVars == <<vars, hidden, operation>>
SameControl == UNCHANGED <<hidden, operation>>
Begin(op, new, old, st, res) ==
  /\ StageWith(new, old, st, res, FALSE, op = "restore")
  /\ operation' = op /\ UNCHANGED hidden
(* The caller's dirty scan can be stale. Preparing a replacement is safe
   even then: its proof must still check both the recorded stat and hash.
   A changed presence is rejected before the journal begins. *)
StartInstall == /\ pc = "idle" /\ ~deferred /\ ~hidden /\ div.k = "none"
                /\ idx.key # cur /\ ((at.p = 0) = (idx.key = Absent))
                /\ Begin("install", cur, idx.key, Stat(at.p), FALSE)
StartResolve == /\ pc = "idle" /\ ~deferred /\ div.k # "none"
                /\ Begin("resolve", cur, div.key, div.st, TRUE)
StartFree == /\ pc = "idle" /\ ~deferred /\ ~hidden /\ div.k = "none"
             /\ at.p # 0 /\ idx.key = cur /\ cur # Absent
             /\ Begin("free", Absent, idx.key, Stat(at.p), FALSE)
StartKeep == /\ pc = "idle" /\ hidden /\ at.p = 0 /\ idx.key = cur /\ cur # Absent
             /\ Begin("keep", cur, Absent, NoStat, FALSE)
StartThin == /\ pc = "idle" /\ ~hidden /\ Clean /\ idx.key = cur /\ cur # Absent
             /\ Begin("thin", cur, idx.key, idx.st, FALSE)
StartRestore(new, old) ==
  /\ pc = "idle" /\ ~deferred /\ new \in store
  /\ old = IF at.p = 0 THEN Absent ELSE idx.key
  /\ Begin("restore", new, old, Stat(at.p), FALSE)
HideIndex ==
  /\ pc = "checked" /\ operation = "free" /\ jr.new = Absent
  (* update-index sets the raw bit even if a person recreated p. A later
     Git refresh/capture clears it; the recreated inode must survive. *)
  /\ hidden' = TRUE /\ pc' = "indexed"
  /\ UNCHANGED <<fc, fm, fresh, at, kept, orph, hnd, hid, punl, idx, cur,
       store, saved, dropped, div, jr, deferred, who, pdel, gap, disp, n, operation>>
KeepIndex ==
  /\ operation = "keep" /\ pc \in {"renamed", "idle"} /\ at.p # 0
  /\ hidden' = FALSE /\ idx' = Record(idx.key)
  /\ pc' = IF pc = "renamed" THEN "indexed" ELSE "idle"
  /\ UNCHANGED <<fc, fm, fresh, at, kept, orph, hnd, hid, punl, cur,
       store, saved, dropped, div, jr, deferred, who, pdel, gap, disp, n, operation>>
(* Conflict-table persistence follows the physical install in resolve. *)
ResolveIndex ==
  /\ pc = "renamed" /\ operation = "resolve"
  /\ idx' = Record(jr.new) /\ pc' = "indexed"
  /\ UNCHANGED <<fc, fm, fresh, at, kept, orph, hnd, hid, punl, cur,
       store, saved, dropped, div, jr, deferred, who, pdel, gap, disp, n, hidden, operation>>
UnindexedDone ==
  /\ pc = "renamed" /\ operation \in {"restore", "thin"}
  /\ kept' = Keep(at.aside, jr.exp) /\ at' = [at EXCEPT !.aside = 0]
  /\ jr' = Closed /\ pc' = "idle"
  /\ who' = IF operation = "restore" THEN "person" ELSE who
  /\ saved' = IF operation = "restore" THEN saved \cup {jr.new} ELSE saved
  /\ UNCHANGED <<fc, fm, fresh, orph, hnd, hid, punl, idx, cur, store,
       dropped, div, deferred, pdel, gap, disp, n, hidden, operation>>
ResolveTable ==
  /\ pc = "idle" /\ div.k # "none" /\ idx.key = cur /\ PathIs(cur)
  /\ div' = None
  /\ UNCHANGED <<fc, fm, fresh, at, kept, orph, hnd, hid, punl, idx, cur,
       store, saved, dropped, pc, jr, deferred, who, pdel, gap, disp, n, hidden, operation>>
NoticeConflict ==
  /\ pc = "idle" /\ div.k = "none" /\ idx.key # cur /\ idx.key \in store
  /\ PathIs(idx.key)
  /\ div' = [k |-> "conflict", key |-> idx.key, st |-> Stat(at.p)]
  /\ UNCHANGED <<fc, fm, fresh, at, kept, orph, hnd, hid, punl, idx, cur,
       store, saved, dropped, pc, jr, deferred, who, pdel, gap, disp, n, hidden, operation>>
(* Catalogue winner selection is abstract here, and checked separately by
   CatalogueTrace. The index can hold a captured contribution before the
   incoming pass records that it lost its original name. ADR 0071 captures
   a recent save beside the installed version even if the index is current. *)
ConcurrentCapture(recent, allowed) ==
  LET c == IF at.p = 0 THEN Absent ELSE fc[at.p] IN
  /\ pc = "idle" /\ div.k = "none" /\ (at.p = 0 \/ at.p \notin fresh)
  /\ (idx.key # cur \/ recent) /\ c # idx.key /\ (allowed \/ recent)
  /\ store' = store \cup ({c} \ {Absent}) /\ idx' = Record(c) /\ pdel' = FALSE
  /\ cur' \in IF recent THEN {cur} ELSE {cur, c}
  /\ UNCHANGED <<fc, fm, fresh, at, kept, orph, hnd, hid, punl, saved,
       dropped, div, pc, jr, deferred, who, gap, disp, n, hidden, operation>>
AdoptPresent ==
  /\ pc = "idle" /\ idx.key # cur /\ PathIs(cur)
  /\ idx' = Record(cur) /\ pdel' = FALSE
  /\ UNCHANGED <<fc, fm, fresh, at, kept, orph, hnd, hid, punl, cur, store,
       saved, dropped, div, pc, jr, deferred, who, gap, disp, n, hidden, operation>>
VisibleBit == hidden /\ at.p # 0 /\ hidden' = FALSE /\ UNCHANGED <<vars, operation>>
DriverRecoverEnd ==
  /\ pc = "rec"
  /\ IF operation \in {"free", "restore"}
     THEN UNCHANGED <<idx, div>>
     ELSE IF Forward /\ (StepSeven \/ jr.new = Absent)
          THEN idx' = Record(jr.new) /\ UNCHANGED div
          ELSE UNCHANGED <<idx, div>>
  /\ hidden' = IF Forward /\ operation = "free" THEN TRUE
                ELSE IF operation = "restore" THEN hidden
                ELSE IF Forward THEN FALSE ELSE hidden
  /\ kept' = Keep(at.aside, jr.exp) /\ at' = [at EXCEPT !.aside = 0, !.stage = 0]
  /\ jr' = Closed /\ pc' = "idle"
  /\ who' = IF Forward /\ operation = "restore" THEN "person" ELSE who
  /\ saved' = IF Forward /\ operation = "restore" THEN saved \cup {jr.new} ELSE saved
  /\ UNCHANGED <<fc, fm, fresh, orph, hnd, hid, punl, cur, store, dropped,
       deferred, pdel, gap, disp, n, operation>>
EndDivergence ==
  /\ pc = "idle" /\ div.k # "none"
  /\ (idx.key = cur \/ PathIs(idx.key))
  /\ div' = None
  /\ UNCHANGED <<fc, fm, fresh, at, kept, orph, hnd, hid, punl, idx, cur,
       store, saved, dropped, pc, jr, deferred, who, pdel, gap, disp, n, hidden, operation>>
ReSuggest ==
  /\ pc = "idle" /\ div.k = "conflict"
  /\ div' = [div EXCEPT !.k = "waiting"] /\ idx' = Record(cur)
  /\ UNCHANGED <<fc, fm, fresh, at, kept, orph, hnd, hid, punl, cur,
       store, saved, dropped, pc, jr, deferred, who, pdel, gap, disp, n, hidden, operation>>
Withdraw ==
  /\ pc = "idle" /\ div.k # "none"
  /\ dropped' = dropped \cup {div.key}
  /\ store' \in IF div.key = cur THEN {store} ELSE {store, store \ {div.key}}
  /\ UNCHANGED <<fc, fm, fresh, at, kept, orph, hnd, hid, punl, idx, cur,
       saved, div, pc, jr, deferred, who, pdel, gap, disp, n, hidden, operation>>
DriverJournal == pc = "idle" /\ who = "pigeon" /\ ~hidden =>
                   PathIs(idx.key) \/ (at.p # 0 /\ fc[at.p] \in saved)
DriverSafety == NoLoss /\ DriverJournal /\ IndexHonest /\ CaptureFirst /\ DeletionKept
=============================================================================
