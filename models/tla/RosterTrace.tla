----------------------------- MODULE RosterTrace -----------------------------
(***************************************************************************)
(* Trace validation of pigeon-core's roster fold against Roster (ADR 0008, *)
(* 13 §13.3).  crates/pigeon-core/tests/roster_trace.rs runs a simulated   *)
(* group whose rosters come from the Rust fold and logs each step; the log *)
(* is read as a module that scripts/trace-to-tla generates (ADR 0053).     *)
(*                                                                         *)
(* TraceNext takes exactly the logged step: Commit(r, t) with the logged   *)
(* op and edits, or Merge(r, y), and requires the roster the model writes  *)
(* to equal the roster the code computed.  So a log is accepted only if    *)
(* every change the code let a replica sign is one of the model's Ops, and *)
(* every roster of the code, at commits and at merges, is the model's.     *)
(* TraceNotConsumed is violated exactly when a behaviour explains every    *)
(* line: that is acceptance (scripts/run-tlc, trace:accepted).             *)
(***************************************************************************)
EXTENDS Roster, RosterCodeLog, RosterCodeBadLog

CONSTANT TraceLog

VARIABLE l

ToSet(s) == {s[i] : i \in DOMAIN s}
AsRos(v) == [mem |-> ToSet(v.mem), adm |-> ToSet(v.adm),
             dev |-> [d \in Devices |-> v.dev[d]], gone |-> ToSet(v.gone)]

Ev == TraceLog[l]
Newest == cm'[Len(cm')]

TraceCommit == /\ Ev.action = "Commit"
               /\ Commit(Ev.vars.r, Ev.vars.t)
               /\ Newest.op = Ev.vars.op
               /\ Newest.edits = ToSet(Ev.vars.edits)
               /\ Newest.ros = AsRos(Ev.vars.ros)

TraceMerge == /\ Ev.action = "Merge"
              /\ Merge(Ev.vars.r, Ev.vars.y)
              /\ IF Ev.vars.ff THEN cm' = cm
                 ELSE Len(cm') = Len(cm) + 1 /\ Newest.ros = AsRos(Ev.vars.ros)

TraceInit == Init /\ l = 1
TraceNext == l <= Len(TraceLog) /\ (TraceCommit \/ TraceMerge) /\ l' = l + 1

TraceNotConsumed == l <= Len(TraceLog)
=============================================================================
