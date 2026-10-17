----------------------------- MODULE PurgeTrace -----------------------------
(* Fixed-membership native-driver observations. The log contains only
   admitted main ancestry, authenticated received seals, and real content
   presence. Purge supplies the transition relation, clocks and ghosts.
   R=0 is an ordering fixture, never evidence that a month has elapsed. *)
EXTENDS Purge, Sequences, PurgeCodeLog
CONSTANT TraceLog
VARIABLE at
AllVars == <<vars, at>>
ToSet(s) == {s[i] : i \in DOMAIN s}
Node(e, d) == CHOOSE n \in ToSet(e.vars.nodes) : n.id = d
Matches(e, kd, kr, kc, ss, ks, cp) ==
  /\ \A d \in Devices :
       /\ kd[d] = Node(e, d).decision
       /\ kr[d] = Node(e, d).restored
       /\ kc[d] = Node(e, d).cancelled
       /\ cp[d] = Node(e, d).present
       /\ ks[d] = ToSet(Node(e, d).reports)
  /\ \A c \in Custodians :
       /\ ss[c].made = Node(e, c).sealed
       /\ ss[c].r1 = Node(e, c).seal_restore
       /\ ss[c].cancel = Node(e, c).seal_cancel
MatchesNow(e) == Matches(e, knowsD, knowsR1, knowsCancel, seal, knownSeals, copy)
MatchesNext(e) == Matches(e, knowsD', knowsR1', knowsCancel', seal', knownSeals', copy')
Native(e) == CASE e.action = "Decide" -> Decide
              [] e.action = "Restore" -> Restore(e.vars.from)
              [] e.action = "Cancel" -> Cancel
              [] e.action = "SyncHistory" -> SyncHistory(e.vars.from, e.vars.to)
              [] e.action = "SyncReports" -> SyncReports(e.vars.from, e.vars.to)
              [] e.action = "Seal" -> Seal(e.vars.from)
              [] e.action = "Drop" -> Drop(e.vars.from)
              [] e.action = "Fetch" -> Fetch(e.vars.to, e.vars.from)
              [] e.action = "Stutter" -> UNCHANGED vars
              [] OTHER -> FALSE
TraceInit == /\ Init /\ at = 1
             /\ TraceLog[1].action = "Init" /\ MatchesNow(TraceLog[1])
TraceNext == /\ at <= Len(TraceLog)
             /\ IF at = Len(TraceLog)
                THEN /\ UNCHANGED vars /\ at' = at + 1
                ELSE /\ Native(TraceLog[at + 1])
                     /\ MatchesNext(TraceLog[at + 1])
                     /\ at' = at + 1
TraceSpec == TraceInit /\ [][TraceNext]_AllVars
TraceNotConsumed == at <= Len(TraceLog)
=============================================================================
