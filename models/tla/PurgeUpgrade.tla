--------------------------- MODULE PurgeUpgrade ---------------------------
(* Initial erasure preparation (proposed ADRs 0076-0077). Two concurrent
   requests can split immutable device choices. Preparation then waits;
   only explicit refounding and rejoining creates a fresh authority domain.
   Old choices remain intact, and old computers may still finish the old
   preparation. A restored local repository must recover its external vote
   before voting again. This finite model covers one new genesis and joins
   before its initial activation, not arbitrary later membership churn.
   Mutations forget a durable choice, admit a partial quorum, or reuse votes
   from the old domain. Each must produce an actual safety counterexample. *)
EXTENDS Naturals, FiniteSets
CONSTANTS People, Backup, Requests, ForgetChoice, PartialQuorum, ReuseOldVotes
Domains == {0, 1}
None == "none"
VARIABLES at, groups, participants, requested, votes, choices, enabled, lagging
vars == <<at, groups, participants, requested, votes, choices, enabled, lagging>>

Init ==
  /\ at = [p \in People |-> 0]
  /\ groups = {0}
  /\ participants = [g \in Domains |-> IF g = 0 THEN People ELSE {}]
  /\ requested = [g \in Domains |-> {}]
  /\ votes = [g \in Domains |-> [p \in People |-> None]]
  /\ choices = {}
  /\ enabled = [g \in Domains |-> None]
  /\ lagging = {}

Prepare(p, r) ==
  /\ p \notin lagging /\ r \notin requested[at[p]]
  /\ requested' = [requested EXCEPT ![at[p]] = @ \cup {r}]
  /\ UNCHANGED <<at, groups, participants, votes, choices, enabled, lagging>>
Ack(p, r) ==
  /\ p \notin lagging /\ r \in requested[at[p]]
  /\ votes[at[p]][p] = None
  /\ votes' = [votes EXCEPT ![at[p]][p] = r]
  /\ choices' = choices \cup {<<at[p], p, r>>}
  /\ UNCHANGED <<at, groups, participants, requested, enabled, lagging>>
Rewind(p) ==
  /\ p \notin lagging
  /\ lagging' = lagging \cup {p}
  /\ votes' = IF ForgetChoice THEN [votes EXCEPT ![at[p]][p] = None] ELSE votes
  /\ UNCHANGED <<at, groups, participants, requested, choices, enabled>>
Recover(p) ==
  /\ p \in lagging /\ lagging' = lagging \ {p}
  /\ UNCHANGED <<at, groups, participants, requested, votes, choices, enabled>>
Refound(p) ==
  /\ 1 \notin groups /\ p \notin lagging
  /\ groups' = groups \cup {1}
  /\ at' = [at EXCEPT ![p] = 1]
  /\ participants' = [participants EXCEPT ![1] = {p}]
  /\ UNCHANGED <<requested, votes, choices, enabled, lagging>>
Rejoin(p) ==
  /\ 1 \in groups /\ at[p] = 0 /\ p \notin lagging /\ enabled[1] = None
  /\ at' = [at EXCEPT ![p] = 1]
  /\ participants' = [participants EXCEPT ![1] = @ \cup {p}]
  /\ UNCHANGED <<groups, requested, votes, choices, enabled, lagging>>
Acknowledged(g, p, r) == votes[g][p] = r
                        \/ (ReuseOldVotes /\ g = 1 /\ votes[0][p] = r)
Activate(g, r) ==
  /\ g \in groups /\ enabled[g] = None /\ r \in requested[g]
  /\ Backup \in participants[g]
  /\ IF PartialQuorum THEN \E p \in participants[g] : Acknowledged(g, p, r)
     ELSE \A p \in participants[g] : Acknowledged(g, p, r)
  /\ enabled' = [enabled EXCEPT ![g] = r]
  /\ UNCHANGED <<at, groups, participants, requested, votes, choices, lagging>>

Next == \/ \E p \in People, r \in Requests : Prepare(p, r) \/ Ack(p, r)
        \/ \E p \in People : Rewind(p) \/ Recover(p) \/ Refound(p) \/ Rejoin(p)
        \/ \E g \in Domains, r \in Requests : Activate(g, r)
Spec == Init /\ [][Next]_vars
TypeOK ==
  /\ at \in [People -> Domains] /\ groups \subseteq Domains
  /\ participants \in [Domains -> SUBSET People]
  /\ requested \in [Domains -> SUBSET Requests]
  /\ votes \in [Domains -> [People -> Requests \cup {None}]]
  /\ choices \subseteq Domains \X People \X Requests
  /\ enabled \in [Domains -> Requests \cup {None}]
  /\ lagging \subseteq People
OneChoicePerDomain ==
  \A a, b \in choices : (a[1] = b[1] /\ a[2] = b[2]) => a[3] = b[3]
ValidCertificate ==
  \A g \in groups : enabled[g] # None =>
    /\ Backup \in participants[g]
    /\ \A p \in participants[g] : <<g, p, enabled[g]>> \in choices
=============================================================================
