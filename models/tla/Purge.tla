------------------------------- MODULE Purge -------------------------------
(* Purge of one key against a concurrent restore or cancel (ADR 0042, spec 08 §8.6).
   r0 is a reference the purge decision covers; r1 is a new reference to the same
   key (a restore during the waiting period, or a copy re-added offline); a cancel
   deletes the decision. History (decision, r1, cancel) and status reports (seals)
   travel separately, between any two devices, in any order: partitions, delays
   and duplicates are interleavings of SyncHistory and SyncReports.
   Time is each custodian's own elapsed time since it received the decision, so
   no clock offset appears. Mode:
     "full"  : drop when every custodian sealed and this device merged each
               sealer's main (the protocol of ADR 0042);
     "seals" : every seal, without merging the sealers' mains (negative test);
     "none"  : each device drops when its own waiting period ends (negative test). *)
EXTENDS Naturals, FiniteSets
CONSTANTS Custodians, Laptops, Decider, Share, LaptopCopies, R, MaxTime, Mode
Devices == Custodians \cup Laptops
NoTime == MaxTime + R + 1
NoSeal == [r1 |-> FALSE, cancel |-> FALSE, made |-> FALSE]

VARIABLES now, dtime, knowsD, knowsR1, knowsCancel, seen, seal, knownSeals,
          copy, timely, firstDrop, dropped, refetched
know == <<knowsD, knowsR1, knowsCancel>>
reports == <<seal, knownSeals>>
ghost == <<timely, firstDrop, dropped, refetched>>
vars == <<now, dtime, know, seen, reports, copy, ghost>>

Init == /\ now = 0 /\ dtime = NoTime
        /\ knowsD = [d \in Devices |-> FALSE]
        /\ knowsR1 = [d \in Devices |-> FALSE]
        /\ knowsCancel = [d \in Devices |-> FALSE]
        /\ seen = [c \in Custodians |-> NoTime]
        /\ seal = [c \in Custodians |-> NoSeal]
        /\ knownSeals = [d \in Devices |-> {}]
        /\ copy = [d \in Devices |-> d \in Share \cup LaptopCopies]
        /\ timely = FALSE /\ firstDrop = NoTime
        /\ dropped = {} /\ refetched = {}

\* Before a custodian's seal; in Mode "none", before its own waiting period ends.
Unsealed(c) == c \in Custodians /\ IF Mode = "none" THEN now < seen[c] + R ELSE ~seal[c].made
Keeps(d) == knowsR1[d] \/ knowsCancel[d]       \* an uncovered reference, or no decision

Released(d) ==
  /\ knowsD[d] /\ ~Keeps(d)
  /\ CASE Mode = "none"  -> IF d \in Custodians THEN now >= seen[d] + R ELSE now >= dtime + R
       [] Mode = "seals" -> Custodians \subseteq knownSeals[d]
       [] Mode = "full"  -> /\ Custodians \subseteq knownSeals[d]
                            /\ \A x \in Custodians : (seal[x].r1 => knowsR1[d])
                                                     /\ (seal[x].cancel => knowsCancel[d])

Tick == /\ now < MaxTime /\ now' = now + 1
        /\ UNCHANGED <<dtime, seen, copy, ghost, reports, know>>

Decide == /\ dtime = NoTime /\ dtime' = now
          /\ knowsD' = [knowsD EXCEPT ![Decider] = TRUE]
          /\ seen' = IF Decider \in Custodians THEN [seen EXCEPT ![Decider] = now] ELSE seen
          /\ UNCHANGED <<now, knowsR1, knowsCancel, copy, ghost, reports>>

Cancel == /\ knowsD[Decider] /\ ~knowsCancel[Decider]
          /\ knowsCancel' = [knowsCancel EXCEPT ![Decider] = TRUE]
          /\ timely' = (timely \/ Unsealed(Decider))
          /\ UNCHANGED <<now, dtime, knowsD, knowsR1, seen, copy, firstDrop, dropped,
                         refetched, reports>>

Restore(d) == /\ knowsD[d] /\ \A e \in Devices : ~knowsR1[e]
              /\ knowsR1' = [knowsR1 EXCEPT ![d] = TRUE]
              /\ timely' = (timely \/ Unsealed(d))
              /\ UNCHANGED <<now, dtime, knowsD, knowsCancel, seen, copy, firstDrop,
                             dropped, refetched, reports>>

SyncHistory(a, b) ==
  /\ a # b
  /\ knowsD' = [knowsD EXCEPT ![b] = @ \/ knowsD[a]]
  /\ knowsR1' = [knowsR1 EXCEPT ![b] = @ \/ knowsR1[a]]
  /\ knowsCancel' = [knowsCancel EXCEPT ![b] = @ \/ knowsCancel[a]]
  /\ seen' = IF b \in Custodians /\ ~knowsD[b] /\ knowsD[a]
             THEN [seen EXCEPT ![b] = now] ELSE seen
  /\ timely' = (timely \/ (Unsealed(b) /\ ((knowsR1[a] /\ ~knowsR1[b])
                                          \/ (knowsCancel[a] /\ ~knowsCancel[b]))))
  /\ UNCHANGED <<now, dtime, copy, firstDrop, dropped, refetched, reports>>

SyncReports(a, b) ==
  /\ a # b /\ ~(knownSeals[a] \subseteq knownSeals[b])
  /\ knownSeals' = [knownSeals EXCEPT ![b] = @ \cup knownSeals[a]]
  /\ UNCHANGED <<now, dtime, seen, seal, copy, ghost, know>>

Seal(c) == /\ Mode # "none" /\ knowsD[c] /\ ~seal[c].made /\ now >= seen[c] + R
           /\ seal' = [seal EXCEPT ![c] = [r1 |-> knowsR1[c], cancel |-> knowsCancel[c],
                                           made |-> TRUE]]
           /\ knownSeals' = [knownSeals EXCEPT ![c] = @ \cup {c}]
           /\ UNCHANGED <<now, dtime, seen, copy, ghost, know>>

Drop(d) == /\ copy[d] /\ Released(d)
           /\ copy' = [copy EXCEPT ![d] = FALSE]
           /\ firstDrop' = IF firstDrop = NoTime THEN now ELSE firstDrop
           /\ dropped' = dropped \cup {d}
           /\ UNCHANGED <<now, dtime, seen, timely, refetched, reports, know>>

Wants(d) == IF d \in Custodians THEN d \in Share /\ ~Released(d) ELSE knowsR1[d]

Fetch(d, from) == /\ ~copy[d] /\ copy[from] /\ Wants(d)
                  /\ copy' = [copy EXCEPT ![d] = TRUE]
                  /\ refetched' = IF d \in dropped THEN refetched \cup {d} ELSE refetched
                  /\ UNCHANGED <<now, dtime, seen, timely, firstDrop, dropped, reports, know>>

Restart(c) == /\ knowsD[c] /\ ~seal[c].made /\ seen[c] # now   \* local state lost
              /\ seen' = [seen EXCEPT ![c] = now]
              /\ UNCHANGED <<now, dtime, copy, ghost, reports, know>>

Next == \/ Tick \/ Decide \/ Cancel
        \/ \E d \in Devices : Restore(d) \/ Drop(d)
        \/ \E c \in Custodians : Seal(c) \/ Restart(c)
        \/ \E a, b \in Devices : SyncHistory(a, b) \/ SyncReports(a, b) \/ Fetch(a, b)

Spec == Init /\ [][Next]_vars

(* A restore or cancel that reached a custodian before that custodian sealed keeps
   the content somewhere, whatever happened before or after. *)
KeepWins == timely => \E d \in Devices : copy[d]
(* Nothing is dropped before R has elapsed since the decision was made. *)
WaitingPeriod == firstDrop # NoTime => firstDrop >= dtime + R
(* A dropped copy comes back only for a reference the purge did not cover. *)
NoResurrection == \A d \in refetched : Keeps(d)
=============================================================================
