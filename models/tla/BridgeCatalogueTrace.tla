----------------------- MODULE BridgeCatalogueTrace -----------------------
(* Actual requests, RPC replies, continuation buffers and returned metadata
   must follow BridgeCatalogue.Step exactly. No missing steps are invented. *)
EXTENDS BridgeCatalogue, BridgeCatalogueCodeLog
CONSTANT TraceLog
VARIABLE position
traceVars == <<state, position>>
TraceInit == state = InitState /\ position = 1
TraceNext == /\ position <= Len(TraceLog)
             /\ Step(state, TraceLog[position], state')
             /\ position' = position + 1
TraceSpec == TraceInit /\ [][TraceNext]_traceVars
TraceNotConsumed == position <= Len(TraceLog)
=============================================================================
