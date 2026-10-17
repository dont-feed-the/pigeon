---------------------------- MODULE AuthorityTrace ----------------------------
(***************************************************************************)
(* Trace validation of pigeon-core's judge, commit::judge, against         *)
(* Authority's Judge (spec 04 §4.4, §4.5, 13 §13.3).                       *)
(* crates/pigeon-core/tests/authority_trace.rs builds random histories in  *)
(* Authority's world: each commit builds on one accepted parent, has any   *)
(* signer, a Force or not, and either writes a tree with roster edits, or  *)
(* rejects or withdraws a proposal (a commit the code keeps, refused for   *)
(* folder rules alone), or adopts one, a merge of the parent and the       *)
(* proposal.  It logs each commit with the code's verdict; the trace       *)
(* appends it to cm with New, which judges it, and requires the model's    *)
(* verdict to equal the code's, so a behaviour that consumes the whole log *)
(* shows that the code agrees with the model on every commit.              *)
(* TraceNotConsumed is violated when a behaviour explains every line: that *)
(* is acceptance (scripts/run-tlc, trace:accepted).                        *)
(***************************************************************************)
EXTENDS Authority, AuthorityCodeLog, AuthorityCodeBadLog

CONSTANT TraceLog

VARIABLE l

ToSet(s) == {s[i] : i \in DOMAIN s}
ConcretePaths == {"F/f", "r"} \cup UNION {
                  {TraceLog[i].vars.tree[j].path : j \in DOMAIN TraceLog[i].vars.tree} :
                  i \in DOMAIN TraceLog}
ConcreteFolders == {q \in ConcretePaths : Len(q) >= 2 /\ SubSeq(q, 1, 2) = "F/"}
ConcreteTree(rows) == [q \in CPaths |->
                       IF \E j \in DOMAIN rows : rows[j].path = q
                       THEN {(CHOOSE row \in ToSet(rows) : row.path = q).version}
                       ELSE {}]
Ev == TraceLog[l]

TraceInit == Init /\ l = 1

TraceNext == /\ l <= Len(TraceLog)
             /\ Ev.action = "Judge"
             /\ LET e == Ev.vars
                    c == New(ToSet(e.parents), e.signer, "traced",
                             ConcreteTree(e.tree), ToSet(e.dec),
                             ToSet(e.redits), e.force, e.cbase)
                IN /\ ToSet(e.parents) \subseteq Ids
                   /\ Cardinality({e.tree[j].path : j \in DOMAIN e.tree}) = Len(e.tree)
                   /\ e.cbase \in ToSet(e.parents) /\ cm[e.cbase].acc
                   /\ c.acc = e.acc
                   /\ cm' = Append(cm, c)
             /\ UNCHANGED <<head, has>>
             /\ l' = l + 1

TraceNotConsumed == l <= Len(TraceLog)
=============================================================================
