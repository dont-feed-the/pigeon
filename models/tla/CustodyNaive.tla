---------------------------- MODULE CustodyNaive ----------------------------
(* "Unsync after confirming another copy exists (and is not about to be deleted)."
   Lock = FALSE : confirm only.
   Lock = TRUE  : the confirming node also promises not to delete until the asker is done. *)
EXTENDS Naturals, FiniteSets
CONSTANTS Nodes, Lock
VARIABLES has, state, peer, locks, msgs
vars == <<has, state, peer, locks, msgs>>

Msg(type, from, to) == [type |-> type, from |-> from, to |-> to]

Init == /\ has   = [n \in Nodes |-> TRUE]
        /\ state = [n \in Nodes |-> "idle"]      \* idle | asking | granted | gone
        /\ peer  = [n \in Nodes |-> n]
        /\ locks = [n \in Nodes |-> {}]           \* nodes I promised to keep the file for
        /\ msgs  = {}

StartRelease(n, m) ==
    /\ state[n] = "idle" /\ has[n] /\ m # n
    /\ Lock => locks[n] = {}
    /\ state' = [state EXCEPT ![n] = "asking"] /\ peer' = [peer EXCEPT ![n] = m]
    /\ msgs' = msgs \cup {Msg("ask", n, m)}
    /\ UNCHANGED <<has, locks>>

Answer(m) == \E a \in msgs :
    /\ a.type = "ask" /\ a.to = m
    /\ IF has[m]
       THEN /\ msgs' = (msgs \ {a}) \cup {Msg("yes", m, a.from)}
            /\ locks' = IF Lock THEN [locks EXCEPT ![m] = @ \cup {a.from}] ELSE locks
       ELSE /\ msgs' = (msgs \ {a}) \cup {Msg("no", m, a.from)}
            /\ UNCHANGED locks
    /\ UNCHANGED <<has, state, peer>>

GetReply(n) == \E r \in msgs :
    /\ r.to = n /\ r.type \in {"yes", "no"} /\ state[n] = "asking"
    /\ state' = [state EXCEPT ![n] = IF r.type = "yes" THEN "granted" ELSE "idle"]
    /\ msgs' = msgs \ {r}
    /\ UNCHANGED <<has, peer, locks>>

Delete(n) ==
    /\ state[n] = "granted"
    /\ Lock => locks[n] = {}                     \* I promised someone to keep it
    /\ has' = [has EXCEPT ![n] = FALSE] /\ state' = [state EXCEPT ![n] = "gone"]
    /\ msgs' = IF Lock THEN msgs \cup {Msg("done", n, peer[n])} ELSE msgs
    /\ UNCHANGED <<peer, locks>>

GetDone(m) == \E d \in msgs :
    /\ d.type = "done" /\ d.to = m
    /\ locks' = [locks EXCEPT ![m] = @ \ {d.from}] /\ msgs' = msgs \ {d}
    /\ UNCHANGED <<has, state, peer>>

Next == \E n \in Nodes : \/ \E m \in Nodes : StartRelease(n, m)
                         \/ Answer(n) \/ GetReply(n) \/ Delete(n) \/ GetDone(n)
Spec == Init /\ [][Next]_vars

NoLoss == \E n \in Nodes : has[n]
=============================================================================
