-------------------------- MODULE PurgeMembership --------------------------
(* Membership extension of Purge (08 §8.6). This model isolates one decision
   made under the original Old quorum, including an offline deciding laptop.
   Original waiting-time checks are abstracted here; Purge and its proof
   cover them separately. A new backup is pending until all old custodians
   acknowledge its pending introduction. Knowing that pending introduction
   fences sealing decisions from the old roster. Activation merges every
   ack's history, and requires cancellation or completed old seals when the
   decision is known. An unseen old-roster decision cannot seal after an ack;
   it must be cancelled/reissued under the new roster. New-roster decisions
   use Purge with that fixed quorum. No liveness or arbitrary churn is claimed.
   Modes naive and no-ack remove independent safety boundaries. *)
EXTENDS Naturals, FiniteSets
CONSTANTS Old, Joiner, Decider, Share, Mode
Devices == Old \cup {Joiner, Decider}
VARIABLES decided, knowsD, restored, cancelled, sealed, sealR, sealC,
          knownSeals, copy, dropped, refetched, timely,
          pending, seenPending, acked, ackD, ackR, ackC, ackS,
          joined, joinClosed, anyDrop, newFetchedOld
vars == <<decided, knowsD, restored, cancelled, sealed, sealR, sealC,
          knownSeals, copy, dropped, refetched, timely,
          pending, seenPending, acked, ackD, ackR, ackC, ackS,
          joined, joinClosed, anyDrop, newFetchedOld>>

Init == /\ decided = FALSE /\ knowsD = {} /\ restored = {} /\ cancelled = {}
        /\ sealed = {} /\ sealR = {} /\ sealC = {}
        /\ knownSeals = [d \in Devices |-> {}]
        /\ copy = Share \cup {Decider} /\ dropped = {} /\ refetched = {}
        /\ timely = FALSE /\ pending = FALSE /\ seenPending = {}
        /\ acked = {} /\ ackD = {} /\ ackR = {} /\ ackC = {} /\ ackS = {}
        /\ joined = FALSE /\ joinClosed = FALSE /\ anyDrop = FALSE
        /\ newFetchedOld = FALSE

Keeps(d) == d \in restored \cup cancelled
EligibleUnsealed(d) == (d \in Old /\ d \notin sealed)
                      \/ (d = Joiner /\ joined /\ ~joinClosed)
Released(d) == /\ d \in knowsD /\ ~Keeps(d)
               /\ Old \subseteq knownSeals[d]
               /\ (sealR # {} => d \in restored)
               /\ (sealC # {} => d \in cancelled)
               /\ (Mode # "naive" \/ ~joined)
Wants(d) == /\ ~Released(d)
            /\ (d \in Old \/ (d = Joiner /\ joined) \/ d \in restored)

Decide == /\ ~decided /\ decided' = TRUE /\ knowsD' = {Decider}
          /\ UNCHANGED <<restored, cancelled, sealed, sealR, sealC, knownSeals,
                         copy, dropped, refetched, timely, pending, seenPending,
                         acked, ackD, ackR, ackC, ackS, joined, joinClosed,
                         anyDrop, newFetchedOld>>
Restore(d) == /\ decided /\ restored = {}
              /\ restored' = {d}
              /\ timely' = (timely \/ EligibleUnsealed(d))
              /\ UNCHANGED <<decided, knowsD, cancelled, sealed, sealR, sealC,
                             knownSeals, copy, dropped, refetched, pending,
                             seenPending, acked, ackD, ackR, ackC, ackS,
                             joined, joinClosed, anyDrop, newFetchedOld>>
Cancel == /\ decided /\ cancelled = {} /\ cancelled' = {Decider}
          /\ UNCHANGED <<decided, knowsD, restored, sealed, sealR, sealC,
                         knownSeals, copy, dropped, refetched, timely, pending,
                         seenPending, acked, ackD, ackR, ackC, ackS, joined,
                         joinClosed, anyDrop, newFetchedOld>>
History(a, b) ==
  /\ a # b
  /\ knowsD' = IF a \in knowsD THEN knowsD \cup {b} ELSE knowsD
  /\ restored' = IF a \in restored THEN restored \cup {b} ELSE restored
  /\ cancelled' = IF a \in cancelled THEN cancelled \cup {b} ELSE cancelled
  /\ timely' = (timely \/ (EligibleUnsealed(b) /\ ~Keeps(b) /\ Keeps(a)))
  /\ UNCHANGED <<decided, sealed, sealR, sealC, knownSeals, copy, dropped,
                 refetched, pending, seenPending, acked, ackD, ackR, ackC,
                 ackS, joined, joinClosed, anyDrop, newFetchedOld>>
Reports(a, b) ==
  /\ a # b /\ ~(knownSeals[a] \subseteq knownSeals[b])
  /\ knownSeals' = [knownSeals EXCEPT ![b] = @ \cup knownSeals[a]]
  /\ UNCHANGED <<decided, knowsD, restored, cancelled, sealed, sealR, sealC,
                 copy, dropped, refetched, timely, pending, seenPending,
                 acked, ackD, ackR, ackC, ackS, joined, joinClosed,
                 anyDrop, newFetchedOld>>
Seal(c) ==
  /\ c \in knowsD /\ c \notin sealed
  /\ (Mode = "no-ack" \/ c \notin seenPending)
  /\ sealed' = sealed \cup {c}
  /\ sealR' = IF c \in restored THEN sealR \cup {c} ELSE sealR
  /\ sealC' = IF c \in cancelled THEN sealC \cup {c} ELSE sealC
  /\ knownSeals' = [knownSeals EXCEPT ![c] = @ \cup {c}]
  /\ UNCHANGED <<decided, knowsD, restored, cancelled, copy, dropped,
                 refetched, timely, pending, seenPending, acked, ackD, ackR,
                 ackC, ackS, joined, joinClosed, anyDrop, newFetchedOld>>
BeginJoin == /\ ~pending /\ pending' = TRUE
             /\ UNCHANGED <<decided, knowsD, restored, cancelled, sealed,
                            sealR, sealC, knownSeals, copy, dropped, refetched,
                            timely, seenPending, acked, ackD, ackR, ackC,
                            ackS, joined, joinClosed, anyDrop, newFetchedOld>>
LearnPending(c) == /\ pending /\ c \notin seenPending
                   /\ seenPending' = seenPending \cup {c}
                   /\ UNCHANGED <<decided, knowsD, restored, cancelled, sealed,
                                  sealR, sealC, knownSeals, copy, dropped,
                                  refetched, timely, pending, acked, ackD,
                                  ackR, ackC, ackS, joined, joinClosed,
                                  anyDrop, newFetchedOld>>
Ack(c) == /\ c \in seenPending /\ c \notin acked
          /\ acked' = acked \cup {c}
          /\ ackD' = IF c \in knowsD THEN ackD \cup {c} ELSE ackD
          /\ ackR' = IF c \in restored THEN ackR \cup {c} ELSE ackR
          /\ ackC' = IF c \in cancelled THEN ackC \cup {c} ELSE ackC
          /\ ackS' = IF c \in sealed THEN ackS \cup {c} ELSE ackS
          /\ UNCHANGED <<decided, knowsD, restored, cancelled, sealed, sealR,
                         sealC, knownSeals, copy, dropped, refetched, timely,
                         pending, seenPending, joined, joinClosed,
                         anyDrop, newFetchedOld>>
Activate ==
  /\ pending /\ ~joined
  /\ (Mode = "no-ack" \/
      (/\ acked = Old
       /\ ((ackD = {} /\ Joiner \notin knowsD) \/ ackC # {}
           \/ Joiner \in cancelled \/ ackS = Old)))
  /\ joined' = TRUE /\ joinClosed' = (ackS = Old)
  /\ knowsD' = IF ackD # {} THEN knowsD \cup {Joiner} ELSE knowsD
  /\ restored' = IF ackR # {} THEN restored \cup {Joiner} ELSE restored
  /\ cancelled' = IF ackC # {} THEN cancelled \cup {Joiner} ELSE cancelled
  /\ knownSeals' = [knownSeals EXCEPT ![Joiner] = @ \cup ackS]
  /\ timely' = (timely \/ (ackS # Old /\
                            (Keeps(Joiner) \/ ackR # {} \/ ackC # {})))
  /\ UNCHANGED <<decided, sealed, sealR, sealC, copy, dropped, refetched,
                 pending, seenPending, acked, ackD, ackR, ackC, ackS,
                 anyDrop, newFetchedOld>>
Drop(d) == /\ d \in copy /\ Released(d)
           /\ copy' = copy \ {d} /\ dropped' = dropped \cup {d}
           /\ anyDrop' = TRUE
           /\ UNCHANGED <<decided, knowsD, restored, cancelled, sealed,
                          sealR, sealC, knownSeals, refetched, timely, pending,
                          seenPending, acked, ackD, ackR, ackC, ackS,
                          joined, joinClosed, newFetchedOld>>
Fetch(d, source) ==
  /\ d \notin copy /\ source \in copy /\ Wants(d)
  /\ copy' = copy \cup {d}
  /\ refetched' = IF d \in dropped THEN refetched \cup {d} ELSE refetched
  /\ newFetchedOld' = (newFetchedOld \/ (d = Joiner /\ anyDrop /\ ~Keeps(d)))
  /\ UNCHANGED <<decided, knowsD, restored, cancelled, sealed, sealR, sealC,
                 knownSeals, dropped, timely, pending, seenPending, acked,
                 ackD, ackR, ackC, ackS, joined, joinClosed, anyDrop>>

Next == \/ Decide \/ Cancel \/ BeginJoin \/ Activate
        \/ \E d \in Devices : Restore(d) \/ Drop(d)
        \/ \E c \in Old : Seal(c) \/ LearnPending(c) \/ Ack(c)
        \/ \E a, b \in Devices : History(a, b) \/ Reports(a, b) \/ Fetch(a, b)
Spec == Init /\ [][Next]_vars
TypeOK == /\ decided \in BOOLEAN /\ pending \in BOOLEAN /\ joined \in BOOLEAN
          /\ joinClosed \in BOOLEAN /\ timely \in BOOLEAN
          /\ anyDrop \in BOOLEAN /\ newFetchedOld \in BOOLEAN
          /\ knowsD \subseteq Devices /\ restored \subseteq Devices
          /\ cancelled \subseteq Devices /\ copy \subseteq Devices
          /\ dropped \subseteq Devices /\ refetched \subseteq Devices
          /\ sealed \subseteq Old /\ sealR \subseteq sealed /\ sealC \subseteq sealed
          /\ knownSeals \in [Devices -> SUBSET Old]
          /\ seenPending \subseteq Old /\ acked \subseteq seenPending
          /\ ackD \subseteq acked /\ ackR \subseteq acked
          /\ ackC \subseteq acked /\ ackS \subseteq acked
KeepWins == timely => copy # {}
NoResurrection == \A d \in refetched : Keeps(d)
NoNewBackupRefetch == newFetchedOld => Keeps(Joiner)
=============================================================================
