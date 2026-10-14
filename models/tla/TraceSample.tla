---------------------------- MODULE TraceSample ----------------------------
(***************************************************************************)
(* The shape of every *Trace.tla (ADR 0008), on a toy spec, with the log   *)
(* read as a module that scripts/trace-to-tla generates (spec 13 §13.3).        *)
(*                                                                         *)
(* The toy spec: a save of a path, then its capture, once.  TraceNext      *)
(* takes exactly the logged step, so TLC searches the behaviours of the    *)
(* spec that explain the log line by line.  TraceNotConsumed is violated   *)
(* exactly when one of them explains every line: that is acceptance.  If   *)
(* none does, TLC finds no error and the depth of its search names the     *)
(* first line that no behaviour explains.                                  *)
(*                                                                         *)
(* The ASSUMEs check that the encodings of the generated module (quotes,   *)
(* backslashes, nested tuples and records, booleans, negative integers)    *)
(* reach TLC as the NDJSON wrote them.                                     *)
(***************************************************************************)
EXTENDS Integers, Sequences, TraceSampleGoodLog, TraceSampleBadLog

CONSTANT TraceLog

ASSUME /\ GoodLog[1].vars.note = "say \"hi\" \\ bye"
       /\ GoodLog[1].vars.flags = <<TRUE, FALSE>> /\ GoodLog[1].vars.delta = -3
       /\ GoodLog[2].vars.note = "" /\ GoodLog[2].vars.flags = <<>>
       /\ GoodLog[3].vars.note = "{\"not\": \"json\"}"
       /\ GoodLog[3].vars.flags = <<<<1, 2>>, <<3>>>>
       /\ GoodLog[4].vars.note = "<<\"x\">>" /\ GoodLog[4].vars.flags[1].k = 1
       /\ Len(GoodLog) = 4 /\ Len(BadLog) = 2

Paths == {"p1", "p2"}

VARIABLES saved, captured, l

Save(p) == saved' = saved \cup {p} /\ UNCHANGED captured
Capture(p) == p \in saved /\ p \notin captured
              /\ captured' = captured \cup {p} /\ UNCHANGED saved

Logged(a, p) == l <= Len(TraceLog) /\ TraceLog[l].action = a
                /\ TraceLog[l].vars.path = p

TraceInit == saved = {} /\ captured = {} /\ l = 1
TraceNext == /\ \E p \in Paths : \/ Logged("Save", p) /\ Save(p)
                                 \/ Logged("Capture", p) /\ Capture(p)
             /\ l' = l + 1

TraceNotConsumed == l <= Len(TraceLog)
=============================================================================
