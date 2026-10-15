-------------------------- MODULE RecoveredNames ----------------------------
(* Two completed recovery batches restart their journal numbers at 1.
   Recovery destinations must remain unique across that restart. *)
EXTENDS Naturals, FiniteSets
CONSTANTS Contents, Names, Off
VARIABLES promised, archive
vars == <<promised, archive>>
Init == /\ promised = {} /\ archive = [n \in Names |-> 0]
Free == {n \in Names : archive[n] = 0}
Preserve(c) ==
  /\ c \in Contents \ promised /\ Free # {}
  /\ LET dest == IF "names" \in Off THEN 1 ELSE CHOOSE n \in Free : TRUE
     IN archive' = [archive EXCEPT ![dest] = c]
  /\ promised' = promised \cup {c}
Next == \E c \in Contents : Preserve(c)
Spec == Init /\ [][Next]_vars
EverySaveKept == promised \subseteq {archive[n] : n \in Names}
=============================================================================
