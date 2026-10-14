------------------------------- MODULE Repair --------------------------------
(* Repair and placement on custodians (spec 08 §8.3-8.4, §8.8-8.10; ADRs 0046,
   0049, 0050, 0054; 13 §13.2). Keys are one unit each. Custodians hold, get,
   sync, scrub and drop them; faults lose a disk, rot a copy or remove a
   custodian; crashes lose what was not synced. Assumption F (02 §2.2) bounds
   the faults: within a repair epoch of a key, fewer than K failure domains
   lose or corrupt it. The epoch is counted on intact, synced copies, so a
   silent corruption opens it when it happens, not when a scrub finds it, and
   a key not yet protected is in one.
   Grounded in git-annex 10.20261006:
   - a get verifies the content on arrival, so it never copies a bad object
     (Annex/Transfer.hs, Annex/Verify.hs);
   - a get refuses to cross annex.diskreserve, counting the downloads still in
     progress (checkDiskSpace' in Annex/Content/LowLevel.hs);
   - a verified drop locks K copies (LOCKCONTENT, lockContentShared in
     Annex/Content.hs): the lock checks that the object is present, never that
     it is intact; AnnexDrop models the drop itself, here it is one step;
   - fsck moves a bad object out and logs it absent (Command/Fsck.hs).
   Off names the safeguards a negative configuration removes. *)
EXTENDS Integers, FiniteSets, Sequences

CONSTANTS Cust,      \* custodians
          Joiners,   \* custodians that may join later (the others start in)
          Dom,       \* [Cust -> failure domains] (ADR 0049)
          Room,      \* [Cust -> units of free space above the reserve]
          Rank,      \* [Keys -> Seq(Cust)]: sizebalanced's and domain repair's order
          K,         \* copies needed, in K domains
          Keys,      \* keys
          Start,     \* custodians holding the keys OnCust at start
          OnCust,    \* keys held at start by the custodians Start
          OnLaptop,  \* keys only the laptop holds at start
          Restored,  \* TRUE: a joiner may bring a disk restored from a backup
          MaxFaults, \* bound on faults (lost disks, rotten copies, removals)
          MaxEnv,    \* bound on crashes and partitions
          Off        \* safeguards removed: "hash", "sync", "fsck", "prio",
                     \* "inflight", "wantroom"

VARIABLES member,    \* custodians in the roster
          dead,      \* custodians removed (`remove-computer`, `git annex dead`)
          lost,      \* members whose repository is gone (repository-lost)
          checking,  \* members whose restored repository is not yet fsck'd
          up,        \* reachable custodians
          has,       \* has[c][k]: c's annex holds an object for k
          bad,       \* bad[c][k]: that object is corrupt, unknown to anyone
          synced,    \* synced[c][k]: that object is on disk (ADR 0046)
          inflight,  \* inflight[c]: keys c is downloading
          lap,       \* keys the laptop holds
          ever,      \* ghost: keys that have been protected
          faults,    \* ghost: faults[k], domains that lost k in its epoch
          hit,       \* ghost: <<key, domain>> pairs the last step hit
          starved,   \* ghost: an endangered key lost room to another key
          nf, ne     \* fault and environment counters
vars == <<member, dead, lost, checking, up, has, bad, synced, inflight, lap,
          ever, faults, hit, starved, nf, ne>>

Domains(S) == {Dom[c] : c \in S}
Live == member \ lost
Members == member \ (lost \cup checking)
Used(c) == Cardinality({k \in Keys : has[c][k]})
HasRoom(c) == IF "inflight" \in Off THEN Used(c) < Room[c]
              ELSE Used(c) + Cardinality(inflight[c]) < Room[c]
Free(c) == Room[c] - Used(c) - Cardinality(inflight[c])

\* What pigeon counts (06 §6.2): synced copies of members not being checked.
Counted(k) == {c \in Live : has[c][k] /\ ("sync" \in Off \/ synced[c][k])
                            /\ ("fsck" \in Off \/ c \notin checking)}
Protected(k) == Cardinality(Domains(Counted(k))) >= K
Endangered(k) == k \in ever /\ ~Protected(k)
\* What really survives: intact, synced copies.
Good(k) == {c \in Live : has[c][k] /\ ~bad[c][k] /\ synced[c][k]}

\* sizebalanced's share: the first K members in the key's rank order.
RECURSIVE FirstK(_, _, _)
FirstK(s, S, n) == IF n = 0 \/ s = <<>> THEN {}
                   ELSE IF Head(s) \in S THEN {Head(s)} \cup FirstK(Tail(s), S, n - 1)
                   ELSE FirstK(Tail(s), S, n)
Share(k) == FirstK(Rank[k], Members, K)
Getting(c, k) == has[c][k] \/ k \in inflight[c]
\* The share's members that can take the key (08 §8.3, ADR 0050).
Taking(k) == IF "wantroom" \in Off THEN Share(k)
             ELSE {c \in Share(k) : Getting(c, k) \/ HasRoom(c)}
\* Domain repair (ADR 0049): while those that hold or take the key span fewer
\* than K domains, the best-ranked member with room in each missing domain.
RECURSIVE Fill(_, _, _)
Fill(k, s, span) ==
  IF Cardinality(span) >= K \/ s = <<>> THEN {}
  ELSE LET c == Head(s) IN
       IF c \in Members /\ Dom[c] \notin span /\ (Getting(c, k) \/ HasRoom(c))
         THEN {c} \cup Fill(k, Tail(s), span \cup {Dom[c]})
         ELSE Fill(k, Tail(s), span)
DomainRepair(k) == Fill(k, Rank[k], Domains(Counted(k) \cup Taking(k)))
Wants(c, k) == c \in Share(k) \/ c \in DomainRepair(k)

\* The endangered keys c wants and has not started (ADR 0050).
EndWanted(c) == {e \in Keys : Endangered(e) /\ Wants(c, e) /\ ~Getting(c, e)}

\* Copies a verified drop by a custodian may lock. With the hash safeguard the
\* holder hashes its object before granting the lock to another custodian.
Lockable(c, k) == {s \in (Live \cap up) \ {c} :
                     /\ has[s][k]
                     /\ ("sync" \in Off \/ synced[s][k])
                     /\ ("fsck" \in Off \/ s \notin checking)
                     /\ ("hash" \in Off \/ ~bad[s][k])}
\* Copies a laptop's drop may lock: the holder does not hash for a laptop.
LaptopLockable(k) == {s \in Live \cap up :
                        /\ has[s][k]
                        /\ ("sync" \in Off \/ synced[s][k])
                        /\ ("fsck" \in Off \/ s \notin checking)}
Proof(S) == \E P \in SUBSET S : Cardinality(P) = K /\ Cardinality(Domains(P)) = K

\* Values for the configurations: one domain per custodian, or 2 and 3 on one
\* disk; the joiner 3 has room for one key; keys ranked differently.
OwnDisks == [c \in Cust |-> c]
SharedDisk == [c \in Cust |-> IF c = 3 THEN 2 ELSE c]
RoomTight == [c \in Cust |-> IF c = 3 THEN 1 ELSE 2]
RoomAmple == [c \in Cust |-> 2]
RankAB == [k \in Keys |-> IF k = "a" THEN <<3, 1, 2>> ELSE <<1, 2, 3>>]
\* Four custodians, one key: the joiner 3 is at its reserve (no room), yet
\* sizebalanced ranks it first.
RoomNone3 == [c \in Cust |-> IF c = 3 THEN 0 ELSE 2]
RankA4 == [k \in Keys |-> <<3, 2, 4, 1>>]
RankAB4 == [k \in Keys |-> IF k = "a" THEN <<3, 1, 4, 2>> ELSE <<1, 2, 3, 4>>]

Init == /\ member = Cust \ Joiners
        /\ dead = {} /\ lost = {} /\ checking = {}
        /\ up = Cust
        /\ has = [c \in Cust |-> [k \in Keys |-> c \in Start /\ k \in OnCust]]
        /\ bad = [c \in Cust |-> [k \in Keys |-> FALSE]]
        /\ synced = has
        /\ inflight = [c \in Cust |-> {}]
        /\ lap = OnLaptop
        /\ ever = {k \in OnCust : Cardinality(Domains(Start)) >= K}
        /\ faults = [k \in Keys |-> {}]
        /\ hit = {}
        /\ starved = FALSE
        /\ nf = 0 /\ ne = 0

\* After every step: a key is protected once; an epoch ends when its intact
\* copies span K domains again (02 §2.1).
Ghosts == /\ ever' = ever \cup {k \in Keys : Protected(k)'}
          /\ faults' = [k \in Keys |->
                          IF Cardinality(Domains(Good(k)')) >= K THEN {}
                          ELSE faults[k] \cup {p[2] : p \in {q \in hit' : q[1] = k}}]
\* F: a fault keeps fewer than K domains lost per epoch. A key whose intact
\* copies never spanned K domains is in an epoch too: a copy that rots before
\* the key is counted protected counts against F.
F(h) == \A k \in Keys :
          Cardinality(faults[k] \cup {p[2] : p \in {q \in h : q[1] = k}}) < K

Sys == <<member, dead, lost, checking, lap, nf, ne>>

StartGet(c, k) ==
  /\ c \in Members \cap up /\ Wants(c, k) /\ ~Getting(c, k) /\ HasRoom(c)
  /\ \/ k \in lap
     \/ \E s \in (Live \cap up) \ {c} : has[s][k] /\ ~bad[s][k]
  /\ LET short == ~Endangered(k) /\ Free(c) - 1 < Cardinality(EndWanted(c))
     IN /\ ("prio" \notin Off => ~short)
        /\ starved' = (starved \/ short)
  /\ inflight' = [inflight EXCEPT ![c] = @ \cup {k}]
  /\ hit' = {}
  /\ UNCHANGED <<Sys, up, has, bad, synced>>

\* The download ends: git-annex verified it against the key.
FinishGet(c, k) ==
  /\ k \in inflight[c] /\ c \in up
  /\ \/ k \in lap
     \/ \E s \in (Live \cap up) \ {c} : has[s][k] /\ ~bad[s][k]
  /\ has' = [has EXCEPT ![c][k] = TRUE]
  /\ bad' = [bad EXCEPT ![c][k] = FALSE]
  /\ synced' = [synced EXCEPT ![c][k] = FALSE]
  /\ inflight' = [inflight EXCEPT ![c] = @ \ {k}]
  /\ hit' = {} /\ starved' = starved
  /\ UNCHANGED <<Sys, up>>

AbortGet(c, k) ==
  /\ k \in inflight[c]
  /\ \/ c \notin up
     \/ ~(k \in lap \/ \E s \in (Live \cap up) \ {c} : has[s][k] /\ ~bad[s][k])
  /\ inflight' = [inflight EXCEPT ![c] = @ \ {k}]
  /\ hit' = {} /\ starved' = starved
  /\ UNCHANGED <<Sys, up, has, bad, synced>>

\* After each batch pigeond syncs what arrived (ADR 0046).
Sync(c, k) ==
  /\ has[c][k] /\ ~synced[c][k] /\ c \in Live
  /\ synced' = [synced EXCEPT ![c][k] = TRUE]
  /\ hit' = {} /\ starved' = starved
  /\ UNCHANGED <<Sys, up, has, bad, inflight>>

\* Rebalancing (08 §8.10): a custodian drops a key it no longer wants.
Drop(c, k) ==
  /\ c \in Members \cap up /\ has[c][k] /\ ~Wants(c, k)
  /\ Proof(Lockable(c, k))
  /\ has' = [has EXCEPT ![c][k] = FALSE]
  /\ bad' = [bad EXCEPT ![c][k] = FALSE]
  /\ hit' = {} /\ starved' = starved
  /\ UNCHANGED <<Sys, up, synced, inflight>>

\* The laptop drops what custody protects (08 §8.3); its copy never counts.
LaptopDrop(k) ==
  /\ k \in lap /\ Proof(LaptopLockable(k))
  /\ lap' = lap \ {k}
  /\ hit' = {} /\ starved' = starved
  /\ UNCHANGED <<member, dead, lost, checking, nf, ne, up, has, bad, synced, inflight>>

\* A scrub finds a bad object: moved out, logged absent; repair follows (08 §8.8).
Scrub(c, k) ==
  /\ c \in Live \cap up /\ has[c][k] /\ bad[c][k]
  /\ has' = [has EXCEPT ![c][k] = FALSE]
  /\ bad' = [bad EXCEPT ![c][k] = FALSE]
  /\ hit' = {} /\ starved' = starved
  /\ UNCHANGED <<Sys, up, synced, inflight>>

\* A joiner comes in empty, or on a disk restored from a backup whose objects
\* may be bad; it counts only after a hashing fsck (08 §8.4, 09 §9.9).
Join(c) ==
  /\ c \in Joiners \ (member \cup dead)
  /\ member' = member \cup {c}
  /\ \/ /\ UNCHANGED <<checking, has, bad, synced>>
     \/ /\ Restored
        /\ \E P \in SUBSET Keys \ {{}}, B \in SUBSET Keys :
             /\ B \subseteq P /\ Cardinality(P) <= Room[c]
             /\ has' = [has EXCEPT ![c] = [k \in Keys |-> k \in P]]
             /\ synced' = [synced EXCEPT ![c] = [k \in Keys |-> k \in P]]
             /\ bad' = [bad EXCEPT ![c] = [k \in Keys |-> k \in B]]
        /\ checking' = checking \cup {c}
  /\ hit' = {} /\ starved' = starved
  /\ UNCHANGED <<dead, lost, up, inflight, lap, nf, ne>>

FsckDone(c) ==
  /\ c \in checking \cap up
  /\ checking' = checking \ {c}
  /\ has' = [has EXCEPT ![c] = [k \in Keys |-> has[c][k] /\ ~bad[c][k]]]
  /\ bad' = [bad EXCEPT ![c] = [k \in Keys |-> FALSE]]
  /\ hit' = {} /\ starved' = starved
  /\ UNCHANGED <<member, dead, lost, lap, nf, ne, up, synced, inflight>>

\* An admin removes a custodian whose repository is lost (Needs you, 08 §8.4).
RemoveLost(c) ==
  /\ c \in lost
  /\ member' = member \ {c} /\ dead' = dead \cup {c} /\ lost' = lost \ {c}
  /\ hit' = {} /\ starved' = starved
  /\ UNCHANGED <<checking, lap, nf, ne, up, has, bad, synced, inflight>>

\* Faults, bounded by F and MaxFaults.
Rot(c, k) ==
  /\ nf < MaxFaults /\ c \in Live /\ has[c][k] /\ ~bad[c][k]
  /\ hit' = {<<k, Dom[c]>>} /\ F(hit')
  /\ bad' = [bad EXCEPT ![c][k] = TRUE]
  /\ nf' = nf + 1 /\ starved' = starved
  /\ UNCHANGED <<member, dead, lost, checking, lap, ne, up, has, synced, inflight>>

LoseDisk(c) ==
  /\ nf < MaxFaults /\ c \in Live
  /\ hit' = {<<k, Dom[c]>> : k \in {j \in Keys : has[c][j]}} /\ F(hit')
  /\ lost' = lost \cup {c} /\ checking' = checking \ {c}
  /\ has' = [has EXCEPT ![c] = [k \in Keys |-> FALSE]]
  /\ bad' = [bad EXCEPT ![c] = [k \in Keys |-> FALSE]]
  /\ inflight' = [inflight EXCEPT ![c] = {}]
  /\ nf' = nf + 1 /\ starved' = starved
  /\ UNCHANGED <<member, dead, lap, ne, up, synced>>

\* An admin removes a custodian that still holds copies: they leave the group.
Remove(c) ==
  /\ nf < MaxFaults /\ c \in Live
  /\ hit' = {<<k, Dom[c]>> : k \in {j \in Keys : has[c][j]}} /\ F(hit')
  /\ member' = member \ {c} /\ dead' = dead \cup {c} /\ checking' = checking \ {c}
  /\ has' = [has EXCEPT ![c] = [k \in Keys |-> FALSE]]
  /\ bad' = [bad EXCEPT ![c] = [k \in Keys |-> FALSE]]
  /\ inflight' = [inflight EXCEPT ![c] = {}]
  /\ nf' = nf + 1 /\ starved' = starved
  /\ UNCHANGED <<lost, lap, ne, up, synced>>

\* A crash or power loss: what was not synced is gone, downloads stop.
Crash(c) ==
  /\ ne < MaxEnv /\ c \in Live
  /\ has' = [has EXCEPT ![c] = [k \in Keys |-> has[c][k] /\ synced[c][k]]]
  /\ bad' = [bad EXCEPT ![c] = [k \in Keys |-> bad[c][k] /\ synced[c][k]]]
  /\ inflight' = [inflight EXCEPT ![c] = {}]
  /\ ne' = ne + 1 /\ hit' = {} /\ starved' = starved
  /\ UNCHANGED <<member, dead, lost, checking, lap, nf, up, synced>>

Partition(c) == /\ ne < MaxEnv /\ c \in up /\ up' = up \ {c} /\ ne' = ne + 1
                /\ hit' = {} /\ starved' = starved
                /\ UNCHANGED <<member, dead, lost, checking, lap, nf, has, bad, synced, inflight>>
Heal(c) == /\ c \notin up /\ up' = up \cup {c}
           /\ hit' = {} /\ starved' = starved
           /\ UNCHANGED <<Sys, has, bad, synced, inflight>>

Next == /\ \/ \E c \in Cust, k \in Keys :
                \/ StartGet(c, k) \/ FinishGet(c, k) \/ AbortGet(c, k) \/ Sync(c, k)
                \/ Drop(c, k) \/ Scrub(c, k) \/ Rot(c, k)
           \/ \E k \in Keys : LaptopDrop(k)
           \/ \E c \in Cust : Join(c) \/ FsckDone(c) \/ RemoveLost(c) \/ LoseDisk(c)
                              \/ Remove(c) \/ Crash(c) \/ Partition(c) \/ Heal(c)
        /\ Ghosts

\* Liveness assumptions (13 §13.2): transfers, syncs and scrubs run; partitions
\* heal; admins remove lost custodians and add the joiner.
Fairness == /\ \A c \in Cust, k \in Keys :
                 /\ WF_vars(StartGet(c, k) /\ Ghosts) /\ WF_vars(FinishGet(c, k) /\ Ghosts)
                 /\ WF_vars(AbortGet(c, k) /\ Ghosts) /\ WF_vars(Sync(c, k) /\ Ghosts)
                 /\ WF_vars(Scrub(c, k) /\ Ghosts)
            /\ \A c \in Cust :
                 /\ WF_vars(FsckDone(c) /\ Ghosts) /\ WF_vars(Heal(c) /\ Ghosts)
                 /\ WF_vars(RemoveLost(c) /\ Ghosts) /\ WF_vars(Join(c) /\ Ghosts)

Spec == Init /\ [][Next]_vars
FairSpec == Spec /\ Fairness

ASSUME Start \subseteq Cust \ Joiners

TypeOK == /\ member \subseteq Cust /\ lost \subseteq member /\ checking \subseteq member
          /\ inflight \in [Cust -> SUBSET Keys] /\ lap \subseteq Keys
          /\ nf \in 0..MaxFaults /\ ne \in 0..MaxEnv
\* P1: a key once protected keeps an intact copy, on the laptop or on disk.
Survival == \A k \in ever : k \in lap \/ Good(k) # {}
\* The reserve is never crossed (ADR 0050).
ReserveKept == \A c \in Cust : Used(c) <= Room[c]
\* Admission never gives an endangered key's room to another (ADR 0050).
EndangeredFirst == ~starved
\* INV-K, as a property of steps: a copy leaves only with its disk, its
\* custodian, a scrub of a bad object, a crash before its sync, or a drop of an
\* unwanted key that K copies in K domains back.
INV_K == [][\A c \in Cust, k \in Keys :
              (has[c][k] /\ ~has'[c][k]) =>
                \/ c \in lost' \/ c \notin member' \/ bad[c][k] \/ ~synced[c][k]
                \/ (~Wants(c, k) /\ Proof(Lockable(c, k)))]_vars
\* The capacity assumption of the liveness configurations (13 §13.2): the
\* members still standing and the joiners to come span K domains.
CapacityHolds == Cardinality(Domains((member \ lost) \cup (Joiners \ (member \cup dead)))) >= K
\* Under fairness, an endangered key gets K domains back.
Restores == \A k \in Keys : [](Endangered(k) => <>~Endangered(k))
=============================================================================
