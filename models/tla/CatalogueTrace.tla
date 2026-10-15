---------------------------- MODULE CatalogueTrace ----------------------------
(***************************************************************************)
(* Trace validation of pigeon-core's catalogue merge against Catalogue's   *)
(* Record (spec 05 §5.2, 13 §13.3).  crates/pigeon-core/tests/             *)
(* catalogue_trace.rs runs three devices of Alice, Bob and Paul that save, *)
(* delete, rename (most with a Pigeon-Renamed trailer, ADR 0072), keep    *)
(* one version or both, edit copies, suggest in Alice's folder and pull,   *)
(* and logs every commit as catalogue::Engine judges it, in the abstract   *)
(* slots of Catalogue: its parents, rank, tree, resolutions and renames,   *)
(* with the code's verdict and, when accepted, its                         *)
(* catalogue (each version where it is shown and the commit it represents) *)
(* and open conflicts.  TraceNext takes the step only if Step agrees on    *)
(* all three, so a log is accepted exactly when the code computes the      *)
(* model's merge on every commit (scripts/run-tlc, trace:accepted).        *)
(***************************************************************************)
EXTENDS Catalogue, CatalogueCodeLog, CatalogueCodeBadLog

CONSTANT TraceLog

VARIABLE l

ToSet(s) == {s[i] : i \in DOMAIN s}
E == TraceLog[l].vars
Rec == [id |-> E.id, parents |-> ToSet(E.parents), rank |-> E.rank, alice |-> E.alice,
        tree |-> ToSet(E.tree), res |-> ToSet(E.res), rn |-> ToSet(E.rn)]

TraceInit == Init /\ l = 1

TraceNext == /\ l <= Len(TraceLog)
             /\ TraceLog[l].action = "Record"
             /\ LET s == Step(Rec) IN
                  /\ s.kind = E.outcome
                  /\ s.kind \in {"clean", "contribution"} =>
                       LET P == Present(s.X, s.C, s.st) IN
                         /\ P.view = ToSet(E.view)
                         /\ P.open = ToSet(E.open)
             /\ Record(Rec)
             /\ l' = l + 1

TraceNotConsumed == l <= Len(TraceLog)
=============================================================================
