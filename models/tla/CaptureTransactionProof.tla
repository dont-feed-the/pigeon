---------------------- MODULE CaptureTransactionProof -----------------------
(* Unbounded safety proof of the actual CaptureTransaction actions. Values
   need not be finite; the commit/install guard is enabled. *)
EXTENDS CaptureTransactionInvariant, TLAPS

ASSUME Parameters == 1 \in Values /\ Off = {}

THEOREM Initial == Init => Inductive
BY SMT, Parameters DEF Init, Inductive, StateType, CapturedBeforeDisplacement

THEOREM Step == Inductive /\ [Next]_vars => Inductive'
<1>1. SUFFICES ASSUME Inductive, [Next]_vars PROVE Inductive'
  OBVIOUS
<1>2. \A v \in Values : Inductive /\ Save(v) => Inductive'
    BY SMT DEF Save, Inductive, StateType, CapturedBeforeDisplacement
<1>3. \A v \in Values : Inductive /\ Receive(v) => Inductive'
    BY SMT DEF Receive, Inductive, StateType, CapturedBeforeDisplacement
<1>4. Inductive /\ Stage => Inductive'
    BY SMT DEF Stage, Inductive, StateType, CapturedBeforeDisplacement
<1>5. Inductive /\ Commit => Inductive'
    BY SMT DEF Commit, Inductive, StateType, CapturedBeforeDisplacement
<1>6. Inductive /\ Fail => Inductive'
    BY SMT DEF Fail, Inductive, StateType, CapturedBeforeDisplacement
<1>7. Inductive /\ Rollback => Inductive'
    BY SMT DEF Rollback, Inductive, StateType, CapturedBeforeDisplacement
<1>8. Inductive /\ Install => Inductive'
    BY SMT, Parameters DEF Install, Inductive, StateType, CapturedBeforeDisplacement
<1>9. Inductive /\ UNCHANGED vars => Inductive'
    BY SMT DEF vars, Inductive, StateType, CapturedBeforeDisplacement
<1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7, <1>8, <1>9, SMT DEF Next

THEOREM Safety == Spec => []CapturedBeforeDisplacement
<1>1. Spec => []Inductive BY Initial, Step, PTL DEF Spec
<1>2. Inductive => CapturedBeforeDisplacement BY DEF Inductive
<1> QED BY <1>1, <1>2, PTL
=============================================================================
