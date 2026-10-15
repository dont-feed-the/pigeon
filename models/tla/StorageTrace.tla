----------------------------- MODULE StorageTrace ---------------------------
(* Safety-boundary observations from the real filesystem driver, not a
   simulation of Git. Tests log hashes read from live/retained files and
   an independent tree-by-tree history oracle. This does not claim full
   CaptureInstall refinement; each named boundary must explain its log. *)
EXTENDS Naturals, Sequences, StorageCodeLog, StorageCodeBadLog
CONSTANT TraceLog
VARIABLE l
ToSet(s) == {s[i] : i \in DOMAIN s}
Ev == TraceLog[l]
Valid(e) == CASE e.action = "Capture" ->
                  (~e.vars.pending \/ e.vars.live = e.vars.saved \/ e.vars.captured)
             [] e.action = "Archive" ->
                  ToSet(e.vars.saved) \subseteq ToSet(e.vars.present)
             [] e.action = "Release" ->
                  /\ (e.vars.written => ~e.vars.released)
                  /\ ToSet(e.vars.saved) \subseteq ToSet(e.vars.present)
             [] e.action = "Retention" ->
                  ToSet(e.vars.expected) = ToSet(e.vars.enumerated)
             [] OTHER -> FALSE
TraceInit == l = 1
TraceNext == /\ l <= Len(TraceLog) /\ Valid(Ev) /\ l' = l + 1
TraceSpec == TraceInit /\ [][TraceNext]_l
TraceNotConsumed == l <= Len(TraceLog)
=============================================================================
