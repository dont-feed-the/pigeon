------------------------- MODULE BridgeSyncTrace -------------------------
(* Consume every actual transaction observation in order, without repair. *)
EXTENDS BridgeSync, BridgeSyncCodeLog
CONSTANT TraceLog
VARIABLES state, position
traceVars == <<state, position>>
TraceInit == state = InitState /\ position = 1
TraceNext == /\ position <= Len(TraceLog)
             /\ Step(state, TraceLog[position], state')
             /\ position' = position + 1
TraceSpec == TraceInit /\ [][TraceNext]_traceVars
TraceNotConsumed == position <= Len(TraceLog)
=============================================================================
