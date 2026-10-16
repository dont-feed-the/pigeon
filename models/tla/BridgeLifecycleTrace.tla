----------------------- MODULE BridgeLifecycleTrace -----------------------
(* Consume every real event in order; no implicit register, revoke, identity
   check or restart is inserted to make a trace fit the protocol. *)
EXTENDS BridgeLifecycle, BridgeLifecycleCodeLog
CONSTANT TraceLog
VARIABLE position
traceVars == <<state, position>>
TraceInit == state = InitState /\ position = 1
TraceNext == /\ position <= Len(TraceLog)
             /\ Step(state, TraceLog[position], state')
             /\ state'.safe
             /\ position' = position + 1
TraceSpec == TraceInit /\ [][TraceNext]_traceVars
TraceNotConsumed == position <= Len(TraceLog)
=============================================================================
