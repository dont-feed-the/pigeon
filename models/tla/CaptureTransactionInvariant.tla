-------------------- MODULE CaptureTransactionInvariant ---------------------
(* Auxiliary state facts shared by the proof and symbolic induction checks. *)
EXTENDS CaptureTransaction

StateType == /\ live \in Values /\ idx \in Values /\ head \in Values
             /\ pending \in Values /\ prior \in Values
             /\ commits \in SUBSET Values /\ displaced \in SUBSET Values
             /\ phase \in {"idle", "staged", "failed"}

Inductive == /\ StateType /\ CapturedBeforeDisplacement
             /\ head \in commits
             /\ (phase = "idle" => idx \in commits)
             /\ (phase # "idle" => idx = pending /\ prior \in commits)
=============================================================================
