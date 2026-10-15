---------------------------- MODULE AuthorityTrace ----------------------------
(***************************************************************************)
(* Trace validation of pigeon-core's authority rule against Authority's    *)
(* Judge (spec 04 §4.4, 13 §13.3).  crates/pigeon-core/tests/              *)
(* authority_trace.rs builds random histories in Authority's world: each    *)
(* commit has one accepted parent, any signer, a tree, roster edits and a   *)
(* Force; it logs each commit as New takes it, with the code's verdict.     *)
(* TraceNext appends the commit as Authority does and takes the step only   *)
(* if Judge's verdict is the logged one, so a log is accepted exactly when  *)
(* the code agrees with the model on every commit.  TraceNotConsumed is     *)
(* violated when a behaviour explains every line: that is acceptance        *)
(* (scripts/run-tlc, trace:accepted).                                       *)
(***************************************************************************)
EXTENDS Authority, AuthorityCodeLog, AuthorityCodeBadLog

CONSTANT TraceLog

VARIABLE l

ToSet(s) == {s[i] : i \in DOMAIN s}
Ev == TraceLog[l]

TraceInit == Init /\ l = 1

TraceNext == /\ l <= Len(TraceLog)
             /\ Ev.action = "Judge"
             /\ LET e == Ev.vars
                    c == New({e.parent}, e.signer, "traced",
                             [q \in CPaths |-> ToSet(e.tree[q])], {},
                             ToSet(e.redits), e.force, e.parent)
                IN /\ e.parent \in Ids /\ cm[e.parent].acc
                   /\ c.acc = e.acc
                   /\ cm' = Append(cm, c)
             /\ UNCHANGED <<head, has>>
             /\ l' = l + 1

TraceNotConsumed == l <= Len(TraceLog)
=============================================================================
