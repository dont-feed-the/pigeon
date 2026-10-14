------------------------------ MODULE AdmitTrace ------------------------------
(***************************************************************************)
(* Trace validation of pigeon-core's admission against Admission (spec     *)
(* 08 §8.9, 13 §13.3).  crates/pigeon-core/tests/admit_trace.rs plans     *)
(* random keys with capacity::admit and logs the instance (Init: sizes,    *)
(* room, endangered keys), then each decision in the order the code took  *)
(* it (Admit or Defer of a key), then Done.  The instance comes from the   *)
(* log, not from Admission's Init; TraceNext takes exactly the logged      *)
(* step, so a log is accepted only if each decision of the code is the     *)
(* model's and every key is planned.  TraceNotConsumed is violated exactly *)
(* when a behaviour explains every line: that is acceptance                *)
(* (scripts/run-tlc, trace:accepted).                                      *)
(***************************************************************************)
EXTENDS Admission, Sequences, AdmitCodeLog, AdmitCodeBadLog

CONSTANT TraceLog

VARIABLE l

ToSet(s) == {s[i] : i \in DOMAIN s}
First == TraceLog[1].vars
LogKeys == DOMAIN First.size
Ev == TraceLog[l]

TraceInit == /\ TraceLog[1].action = "Init"
             /\ size = First.size /\ room = First.room
             /\ endangered = ToSet(First.endangered)
             /\ started = {} /\ deferred = {}
             /\ l = 2

TraceNext == /\ l <= Len(TraceLog)
             /\ \/ Ev.action = "Admit" /\ Admit(Ev.vars.k)
                \/ Ev.action = "Defer" /\ Defer(Ev.vars.k)
                \/ Ev.action = "Done" /\ Done
             /\ l' = l + 1

TraceNotConsumed == l <= Len(TraceLog)
=============================================================================
