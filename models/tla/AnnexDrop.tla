------------------------------ MODULE AnnexDrop ------------------------------
(* git-annex's verified drop, as pigeon relies on it (spec 08 §8.3, 09 §9.4,
   ADR 0046; 13 §13.2). Custodians each hold or lack one key; each drops only its
   own copy, as pigeon does (remote REMOVE is refused by its filter, E13).
   Grounded in git-annex 10.20261006:
   - Begin: Command/Drop.hs performLocal takes lockContentForRemoval
     (Annex/Content.hs: an exclusive lock, refused while a shared lock is held),
     then verifiableCopies (Annex/NumCopies.hs) lists candidates from the
     location log (Logs/Location.hs): not dead, not untrusted, not annex-ignore;
     trusted repositories count at once, unasked (TrustedCopy).
   - Check: LOCKCONTENT (P2P/Protocol.hs) takes lockContentShared on the holder:
     a LockedCopy, kept until the drop ends. If it fails, the fallback asks
     CHECKPRESENT: a RecentlyVerifiedCopy. Over P2P the server answers with
     inAnnex (P2P/Annex.hs), which ignores a removal lock; over a local path
     Remote/Git.hs uses inAnnexSafe, which does not (Check = "p2p" or "safe").
   - Remove: with a removal lock, safeDropAnalysis (Types/NumCopies.hs) needs
     max(numcopies, mincopies) copies of any kind, and the proof has no end time.
   - Expire: a lock over P2P outlives a lost connection by 10 minutes only
     (p2pDefaultLockContentRetentionDuration); a stalled dropper keeps its proof.
   Purge and Drop (forced drops) bypass this protocol and are not modelled.
   INV_K: the key stays on at least K custodians (ADR 0046). *)
EXTENDS Naturals, FiniteSets

CONSTANTS Cust,      \* custodians, a set of numbers (the order ranks them)
          K,         \* numcopies = mincopies
          Check,     \* "safe": CHECKPRESENT honours removal locks; "p2p": it does not
          Choose,    \* "named": K named remotes (ADR 0046); "repos": every
                     \* remote the log lists; "ranked": only custodians ranked above
          Trusted,   \* custodians marked `trust`
          LogOnly,   \* TRUE: count what the location log says, unchecked
          Expiry,    \* TRUE: a lock can expire under a partition, mid-drop
          MaxGets    \* bound on copies made during a behaviour

VARIABLES has,     \* has[c]: c holds the content
          excl,    \* excl[c]: c's own drop holds the removal lock
          shared,  \* shared[c]: droppers holding a LOCKCONTENT lock on c's copy
          log,     \* log[o]: the custodians o's location log lists as holders
          up,      \* reachable custodians (the others are partitioned away)
          cand,    \* cand[d]: candidates d's drop has still to check
          proof,   \* proof[d]: the copies d's drop counts
          dropping, \* custodians running a drop
          dead,    \* custodians marked `dead`
          gets
vars == <<has, excl, shared, log, up, cand, proof, dropping, dead, gets>>

Holders == {c \in Cust : has[c]}
INV_K == Cardinality(Holders) >= K

\* What CHECKPRESENT answers about r's copy.
Present(r) == IF Check = "p2p" THEN has[r] ELSE has[r] /\ ~excl[r]

\* Copies d's location log lists, but not dead ones (Logs/Trust.hs).
Listed(d) == (log[d] \ {d}) \ dead
\* pigeon's choice of the remotes not ignored (ADR 0046), among listed copies.
Choices(d) ==
  CASE Choose = "repos"  -> {Listed(d)}
    [] Choose = "named"  -> {S \in SUBSET Listed(d) : Cardinality(S) = K}
    [] Choose = "ranked" -> {S \in SUBSET {c \in Listed(d) : c > d} :
                                Cardinality(S) = K}

Init == /\ has = [c \in Cust |-> TRUE]
        /\ excl = [c \in Cust |-> FALSE]
        /\ shared = [c \in Cust |-> {}]
        /\ log = [c \in Cust |-> Cust]
        /\ up = Cust
        /\ cand = [c \in Cust |-> {}]
        /\ proof = [c \in Cust |-> {}]
        /\ dropping = {}
        /\ dead = {}
        /\ gets = 0

Begin(d) ==
  /\ d \notin dropping /\ has[d] /\ shared[d] = {}
  /\ \E S \in Choices(d) :   \* trusted ones count unasked, even if ignored
       /\ cand' = [cand EXCEPT ![d] = IF LogOnly THEN {} ELSE S \ Trusted]
       /\ proof' = [proof EXCEPT ![d] =
                      IF LogOnly THEN Listed(d) ELSE Listed(d) \cap Trusted]
  /\ excl' = [excl EXCEPT ![d] = TRUE]
  /\ dropping' = dropping \cup {d}
  /\ UNCHANGED <<has, shared, log, up, dead, gets>>

CheckCopy(d, r) ==
  /\ d \in dropping /\ r \in cand[d]
  /\ cand' = [cand EXCEPT ![d] = @ \ {r}]
  /\ IF {d, r} \subseteq up
       THEN IF has[r] /\ ~excl[r]
              THEN /\ shared' = [shared EXCEPT ![r] = @ \cup {d}]   \* LockedCopy
                   /\ proof' = [proof EXCEPT ![d] = @ \cup {r}]
              ELSE /\ proof' = [proof EXCEPT ![d] =                 \* fallback
                                  IF Present(r) THEN @ \cup {r} ELSE @]
                   /\ UNCHANGED shared
       ELSE UNCHANGED <<shared, proof>>                              \* unreachable
  /\ UNCHANGED <<has, excl, log, up, dropping, dead, gets>>

Release(d) == /\ excl' = [excl EXCEPT ![d] = FALSE]
              /\ shared' = [c \in Cust |-> shared[c] \ {d}]
              /\ cand' = [cand EXCEPT ![d] = {}]
              /\ proof' = [proof EXCEPT ![d] = {}]
              /\ dropping' = dropping \ {d}

Remove(d) ==
  /\ d \in dropping /\ Cardinality(proof[d]) >= K
  /\ has' = [has EXCEPT ![d] = FALSE]
  /\ log' = [log EXCEPT ![d] = @ \ {d}]
  /\ Release(d)
  /\ UNCHANGED <<up, dead, gets>>

Abort(d) == /\ d \in dropping /\ Release(d)
            /\ UNCHANGED <<has, log, up, dead, gets>>

\* A partition outlasts the lock's retention while d stalls before removing.
Expire(d, r) ==
  /\ Expiry /\ d \in shared[r] /\ ~({d, r} \subseteq up)
  /\ shared' = [shared EXCEPT ![r] = @ \ {d}]
  /\ UNCHANGED <<has, excl, log, up, cand, proof, dropping, dead, gets>>

Get(r) ==
  /\ gets < MaxGets /\ ~has[r] /\ r \in up
  /\ \E h \in up : has[h]
  /\ has' = [has EXCEPT ![r] = TRUE]
  /\ log' = [log EXCEPT ![r] = @ \cup {r}]
  /\ gets' = gets + 1
  /\ UNCHANGED <<excl, shared, up, cand, proof, dropping, dead>>

\* o merges r's git-annex branch: it learns what r logged about itself.
Learn(o, r) ==
  /\ {o, r} \subseteq up /\ o # r
  /\ log' = [log EXCEPT ![o] = IF r \in log[r] THEN @ \cup {r} ELSE @ \ {r}]
  /\ UNCHANGED <<has, excl, shared, up, cand, proof, dropping, dead, gets>>

Partition(c) == /\ c \in up /\ up' = up \ {c}
                /\ UNCHANGED <<has, excl, shared, log, cand, proof, dropping, dead, gets>>
Heal(c) == /\ c \notin up /\ up' = up \cup {c}
           /\ UNCHANGED <<has, excl, shared, log, cand, proof, dropping, dead, gets>>

\* `git annex dead`: its copies never count again (Logs/Trust.hs).
MarkDead(c) == /\ dead = {} /\ dead' = {c}
               /\ UNCHANGED <<has, excl, shared, log, up, cand, proof, dropping, gets>>

Next == \E d, r \in Cust :
          \/ Begin(d) \/ CheckCopy(d, r) \/ Remove(d) \/ Abort(d) \/ Expire(d, r)
          \/ Get(d) \/ Learn(d, r) \/ Partition(d) \/ Heal(d) \/ MarkDead(d)

Spec == Init /\ [][Next]_vars

TypeOK == /\ has \in [Cust -> BOOLEAN] /\ excl \in [Cust -> BOOLEAN]
          /\ dropping \subseteq Cust /\ dead \subseteq Cust /\ gets \in 0..MaxGets
\* A copy locked by a drop in progress is never removed under it.
LockedStays == \A c \in Cust : shared[c] # {} => has[c]
=============================================================================
