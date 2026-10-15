----------------------------- MODULE Retention ------------------------------
(* The input boundary of Repair: overwritten/deleted versions remain in
   retained history. Enumerating just the current tree loses obligations. *)
EXTENDS Naturals, FiniteSets
CONSTANTS Keys, Off
VARIABLES current, history, enumerated, checked
vars == <<current, history, enumerated, checked>>
Init == /\ current = 0 /\ history = {} /\ enumerated = {} /\ checked = FALSE
Capture(k) == /\ k \in Keys \ history /\ current' = k
              /\ history' = history \cup {k} /\ checked' = FALSE
              /\ UNCHANGED enumerated
Delete == /\ current # 0 /\ current' = 0 /\ checked' = FALSE
          /\ UNCHANGED <<history, enumerated>>
Enumerate == /\ enumerated' = IF "current" \in Off
                             THEN {current} \ {0} ELSE history
             /\ checked' = TRUE /\ UNCHANGED <<current, history>>
Next == (\E k \in Keys : Capture(k)) \/ Delete \/ Enumerate
Spec == Init /\ [][Next]_vars
Complete == checked => enumerated = history
=============================================================================
