----------------------- MODULE BridgeSparseConflict -----------------------
(* Sparse incoming installation and narrow recovery of old phantom overlays.
   One accepted path lost its name to a concurrent accepted version; its old
   complete Git entry survives at a conflict-copy path. An absent sparse file
   is not a pending local save. Legacy recovery requires every observed guard,
   and rechecks absence after Git reads before removing a phantom entry.
   Filesystem observations, verified pool content and accepted-tree membership
   are inputs; this does not prove hashing, filesystem atomicity or native
   catalogue correctness. A live write may race the awaited inspection.
   Weak switches intentionally regress one implementation guard. *)
EXTENDS Naturals, FiniteSets
CONSTANTS Legacy, Weak

Old == [oid |-> "old", mode |-> "100644", kind |-> "blob"]
New == [oid |-> "new", mode |-> "100644", kind |-> "blob"]
Copies == {Old, [Old EXCEPT !.mode = "100755"],
               [Old EXCEPT !.kind = "tree"], [Old EXCEPT !.oid = "other"]}
Kinds == {"none", "concurrent", "suggestion", "recovered", "deletion"}

VARIABLE s
vars == <<s>>
HiddenAbsent(x) == x.hidden /\ x.file = "absent"
Eligible(x) ==
  /\ x.overlay = "concurrent" /\ ~x.stat /\ HiddenAbsent(x)
  /\ x.mineMatches /\ x.copyNamed /\ x.copy = Old /\ x.poolVerified
CanRepair(x) ==
  /\ x.overlay = "concurrent" /\ ~x.stat
  /\ (Weak = "no-recheck" \/ HiddenAbsent(x))
  /\ x.mineMatches /\ x.copyNamed
  /\ (IF Weak = "key-only" THEN x.copy.oid = Old.oid ELSE x.copy = Old)
  /\ (Weak = "no-verify" \/ x.poolVerified)

Initial(hidden, file, overlay, stat, mineMatches, copyNamed, copy, verified) ==
  [pc |-> "inspect", hidden |-> hidden, file |-> file,
   overlay |-> overlay, stat |-> stat, mineMatches |-> mineMatches,
   copyNamed |-> copyNamed, copy |-> copy, poolVerified |-> verified,
   index |-> Old, inspected |-> FALSE, wrote |-> FALSE,
   liveConflict |-> overlay = "none" /\ file = "local",
   synthetic |-> FALSE, repaired |-> FALSE, unsafeRepair |-> FALSE,
   clean |-> (overlay = "none" /\ hidden /\ file = "absent")
              \/ (overlay = "concurrent" /\ ~stat /\ hidden /\ file = "absent"
                    /\ mineMatches /\ copyNamed /\ copy = Old /\ verified)]
Init ==
  IF ~Legacy
  THEN \E file \in {"absent", "local", "symlink", "unreadable"} :
         s = Initial(TRUE, file, "none", FALSE, TRUE, TRUE, Old, TRUE)
  ELSE \E hidden, stat, mineMatches, copyNamed, verified \in BOOLEAN,
          file \in {"absent", "local", "symlink", "unreadable"},
          overlay \in Kinds \ {"none"}, copy \in Copies :
         s = Initial(hidden, file, overlay, stat, mineMatches, copyNamed, copy, verified)

(* Inspection can await Git reads. Only cached structural evidence is used
   here; absence is checked again by Repair. A symlink or failed lstat is not
   absence. External writes do not acquire the daemon's group mutex. *)
Inspect ==
  /\ s.pc = "inspect"
  /\ s' = [s EXCEPT !.pc = "repair", !.inspected = HiddenAbsent(s)]
LocalWrite ==
  /\ ~s.wrote /\ s.pc \in {"inspect", "repair", "incoming"}
  /\ s' = [s EXCEPT !.file = "local", !.wrote = TRUE]
Repair ==
  /\ s.pc = "repair"
  /\ LET repair == Legacy /\ s.inspected /\ CanRepair(s) IN
       s' = [s EXCEPT !.pc = "incoming",
                     !.overlay = IF repair THEN "none" ELSE @,
                     !.repaired = repair,
                     !.unsafeRepair = repair /\ ~Eligible(s)]
Incoming ==
  /\ s.pc = "incoming"
  /\ s' = IF s.overlay # "none" \/ s.wrote
          THEN [s EXCEPT !.pc = "compact"]
          ELSE IF Weak = "lost-first"
                    \/ ~(IF Weak = "hidden-only" THEN s.hidden ELSE HiddenAbsent(s))
          THEN [s EXCEPT !.pc = "compact", !.overlay = "concurrent",
                         !.stat = s.file # "absent", !.synthetic = s.file = "absent"]
          ELSE [s EXCEPT !.pc = "compact", !.index = New]
Compact ==
  /\ s.pc = "compact"
  /\ s' = IF (s.overlay # "none" /\ s.file \in {"absent", "symlink", "unreadable"})
               \/ s.wrote
          THEN [s EXCEPT !.pc = "blocked"]
          ELSE [s EXCEPT !.pc = "fetch"]
Fetch == s.pc = "fetch" /\ s' = [s EXCEPT !.pc = "done"]
Advance == Inspect \/ Repair \/ Incoming \/ Compact \/ Fetch
Next == Advance \/ LocalWrite
Spec == Init /\ [][Next]_vars
Live == Spec /\ WF_vars(Advance)

NoSyntheticOverlay == ~s.synthetic
OnlyProvenPhantomRepaired == ~s.unsafeRepair
LiveWritePreserved == ~s.wrote \/ s.file = "local"
(* Even uncertain/missing files may be genuine pending changes. Preserve
   their table entry when the evidence does not establish a phantom. *)
PendingPreserved ==
  (Legacy /\ ~s.clean) => s.overlay # "none"
AdvancedWithoutOverlay ==
  (~Legacy /\ s.clean /\ ~s.wrote /\ s.pc \in {"compact", "fetch", "done"})
    => s.index = New /\ s.overlay = "none"
LiveConflictRecorded ==
  (~Legacy /\ s.liveConflict /\ ~s.wrote /\ s.pc \in {"compact", "fetch", "done"})
    => s.index = Old /\ s.overlay = "concurrent"
(* Only clean sparse input and fully proven phantom input promise progress.
   A later live write requires normal capture; uncertain recovery must wait. *)
CanFetch == <>[](~s.clean \/ s.wrote \/ s.pc = "done")
=============================================================================
