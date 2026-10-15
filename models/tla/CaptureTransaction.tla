------------------------ MODULE CaptureTransaction -------------------------
(* Capture's Git boundary: adding to the annex/index precedes committing.
   A failed commit leaves a durable intent; install must wait for rollback
   or commit. Three values suffice for old, saved and incoming versions. *)
EXTENDS Naturals, FiniteSets
CONSTANTS Values, Off
VARIABLES live, idx, head, commits, phase, pending, prior, displaced
vars == <<live, idx, head, commits, phase, pending, prior, displaced>>
Init == /\ live = 1 /\ idx = 1 /\ head = 1 /\ commits = {1}
        /\ phase = "idle" /\ pending = 1 /\ prior = 1 /\ displaced = {}
Save(v) == /\ v \in Values /\ live' = v
           /\ UNCHANGED <<idx, head, commits, phase, pending, prior, displaced>>
Stage == /\ phase = "idle" /\ live # idx
         /\ pending' = live /\ prior' = idx /\ idx' = live /\ phase' = "staged"
         /\ UNCHANGED <<live, head, commits, displaced>>
Commit == /\ phase = "staged" /\ commits' = commits \cup {pending}
          /\ head' = pending /\ phase' = "idle"
          /\ UNCHANGED <<live, idx, pending, prior, displaced>>
Fail == /\ phase = "staged" /\ phase' = "failed"
        /\ UNCHANGED <<live, idx, head, commits, pending, prior, displaced>>
Rollback == /\ phase = "failed" /\ idx' = prior /\ phase' = "idle"
            /\ UNCHANGED <<live, head, commits, pending, prior, displaced>>
Receive(v) == /\ v \in Values /\ phase # "staged"
              /\ head' = v /\ commits' = commits \cup {v}
              /\ UNCHANGED <<live, idx, phase, pending, prior, displaced>>
Install == /\ live = idx /\ idx # head
           /\ phase = "idle" \/ "guard" \in Off
           /\ displaced' = displaced \cup {idx} /\ live' = head /\ idx' = head
           /\ UNCHANGED <<head, commits, phase, pending, prior>>
Next == (\E v \in Values : Save(v) \/ Receive(v)) \/ Stage \/ Commit \/ Fail \/ Rollback \/ Install
Spec == Init /\ [][Next]_vars
CapturedBeforeDisplacement == displaced \subseteq commits
=============================================================================
