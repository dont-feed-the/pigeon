------------------------ MODULE RosterDaemonTrace --------------------------
(* Membership refinement of actual daemon ref moves. Inputs are the
   accepted history, signed Git tree rosters and the daemon's loaded roster.
   Change and the canonical history fold are the operators of Roster.
   Display names, folder rules, UUIDs and settings remain outside Roster's
   membership abstraction; the raw rosters and edits are retained in logs. *)
EXTENDS Roster, RosterDaemonCodeLog
CONSTANT TraceLog
VARIABLE l
ToSet(s) == {s[i] : i \in DOMAIN s}
AsRos(r) == [mem |-> ToSet(r.mem), adm |-> ToSet(r.adm),
            dev |-> [d \in Devices |-> r.dev[d]], gone |-> ToSet(r.gone)]
Ev == TraceLog[l].vars
Rows == Ev.commits
ReadGraph == [i \in 1..Len(Rows) |->
  [parents |-> ToSet(Rows[i].parents), signer |-> Rows[i].signer,
   time |-> Rows[i].time, op |-> "observed", edits |-> ToSet(Rows[i].edits),
   merge |-> Rows[i].fork, ros |-> AsRos(Rows[i].ros)]]
CanEdit(c, p, e) ==
  LET S == cm[p].ros  ps == S.dev[cm[c].signer]
      recovery == cm[c].signer \in ToSet(Rows[p].recovery)
      admin == ps \in S.adm /\ ~recovery
  IN CASE e.k = "add" -> admin \/ e.p = ps
       [] e.k = "rem" -> admin \/ S.dev[e.d] \in {0, ps}
       [] e.k = "remp" -> admin /\ e.p # ps
       [] e.k = "capacity" -> ~recovery /\ (admin \/ S.dev[e.d] = ps)
       [] OTHER -> admin
GoodCommit(c) ==
  /\ Valid(cm[c].ros)
  /\ cm[c].ros.gone \cap Present(cm[c].ros) = {}
  /\ IF c = 1 THEN cm[c].parents = {}
     ELSE /\ cm[c].parents # {}
          /\ IF cm[c].merge
             THEN /\ cm[c].edits = {}
                  /\ \E p \in cm[c].parents : cm[p].ros.dev[cm[c].signer] # 0
                  /\ cm[c].ros = FoldFrom(cm[1].ros, NonMergeAnc(c)).s
             ELSE /\ \A p \in cm[c].parents :
                       /\ cm[p].ros.dev[cm[c].signer] # 0
                       /\ \A e \in ToSet(Rows[c].auth) : CanEdit(c, p, e)
                       /\ LET r == Change(cm[p].ros, cm[c].edits) IN r.ok /\ r.s = cm[c].ros
Observed == /\ cm = ReadGraph
            /\ \A c \in Ids : GoodCommit(c)
            /\ AsRos(Ev.loaded) = cm[Ev.head].ros
            /\ Ev.head \in Ids /\ Ev.before \in Ids \cup {0}
            /\ TraceLog[l].action \in {"Commit", "Merge"}
            /\ TraceLog[l].action = "Commit" =>
                 Ev.before \in cm[Ev.head].parents /\ cm[Ev.head].signer = Ev.device
TraceInit == /\ l = 1 /\ cm = ReadGraph
             /\ head = [r \in Replicas |-> 1] /\ key = [r \in Replicas |-> r]
TraceNext == /\ l <= Len(TraceLog) /\ Observed
             /\ l' = l + 1
             /\ cm' = IF l < Len(TraceLog) THEN
                  [i \in 1..Len(TraceLog[l+1].vars.commits) |->
                    LET r == TraceLog[l+1].vars.commits[i] IN
                    [parents |-> ToSet(r.parents), signer |-> r.signer, time |-> r.time,
                     op |-> "observed", edits |-> ToSet(r.edits), merge |-> r.fork, ros |-> AsRos(r.ros)]]
                  ELSE cm
             /\ UNCHANGED <<head, key>>
TraceSpec == TraceInit /\ [][TraceNext]_<<vars, l>>
TraceNotConsumed == l <= Len(TraceLog)
=============================================================================
