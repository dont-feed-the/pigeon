-------------------------- MODULE AnnexDropProof ----------------------------
(* Verified local drops with safe presence checks, no trusted/log-only copies,
   and no lock expiry. Proves the actual AnnexDrop Next for every finite Cust,
   every feasible K, every MaxGets, and arbitrarily long executions. The
   expiry-enabled and ranked/P2P configurations still need separate checks. *)
EXTENDS AnnexDrop, TLAPS, FiniteSetTheorems

ProofParameters == /\ IsFiniteSet(Cust)
                     /\ K \in Nat /\ K <= Cardinality(Cust)
                     /\ MaxGets \in Nat
                     /\ Check = "safe" /\ Choose = "named"
                     /\ Trusted = {} /\ LogOnly = FALSE /\ Expiry = FALSE

ASSUME Parameters == ProofParameters
USE DEF ProofParameters

StateType == /\ has \in [Cust -> BOOLEAN] /\ excl \in [Cust -> BOOLEAN]
             /\ shared \in [Cust -> SUBSET Cust]
             /\ log \in [Cust -> SUBSET Cust]
             /\ cand \in [Cust -> SUBSET Cust]
             /\ proof \in [Cust -> SUBSET Cust]
             /\ up \subseteq Cust /\ dropping \subseteq Cust /\ dead \subseteq Cust
             /\ gets \in 0..MaxGets

Locks == /\ \A c \in Cust : excl[c] <=> c \in dropping
         /\ \A c \in Cust : shared[c] # {} => has[c] /\ ~excl[c]
         /\ \A d, r \in Cust : r \in proof[d] => d \in shared[r]

Inductive == StateType /\ Locks /\ INV_K

THEOREM NamedChoices == \A d \in Cust :
  Choices(d) = {S \in SUBSET Listed(d) : Cardinality(S) = K}
BY Parameters DEF Choices

THEOREM Initial == Init => Inductive
BY SMT, Parameters DEF Init, Inductive, StateType, Locks, INV_K, Holders

THEOREM RemoveKeepsCopies ==
  \A d \in Cust : Inductive /\ Remove(d) => INV_K'
<1>1. SUFFICES ASSUME NEW d \in Cust, Inductive, Remove(d) PROVE INV_K'
  OBVIOUS
<1>2. proof[d] \subseteq Holders'
  BY <1>1, SMT DEF Inductive, StateType, Locks, Remove, Release, Holders
<1>3. Holders' \subseteq Cust
  BY SMT DEF Holders
<1>4. Cardinality(proof[d]) <= Cardinality(Holders')
  BY <1>2, <1>3, Parameters, FS_Subset, SMT
<1>5. Cardinality(proof[d]) \in Nat /\ Cardinality(Holders') \in Nat
  BY <1>2, <1>3, Parameters, FS_Subset, FS_CardinalityType, SMT
<1> QED BY <1>1, <1>4, <1>5, Parameters, SMT DEF Remove, INV_K

THEOREM Step == Inductive /\ [Next]_vars => Inductive'
<1>1. SUFFICES ASSUME Inductive, [Next]_vars PROVE Inductive'
  OBVIOUS
<1>2. \A d \in Cust : Inductive /\ Begin(d) => Inductive'
  BY SMT, Parameters, NamedChoices DEF Begin, Listed, Inductive, StateType, Locks, INV_K, Holders
<1>3. \A d, r \in Cust : Inductive /\ CheckCopy(d, r) => Inductive'
  BY SMT, Parameters DEF CheckCopy, Present, Inductive, StateType, Locks, INV_K, Holders
<1>4. \A d \in Cust : Inductive /\ Remove(d) => Inductive'
  <2>1. SUFFICES ASSUME NEW d \in Cust, Inductive, Remove(d) PROVE Inductive'
    OBVIOUS
  <2>2. INV_K' BY <2>1, RemoveKeepsCopies
  <2>3. StateType' /\ Locks'
    BY <2>1, SMT DEF Remove, Release, Inductive, StateType, Locks
  <2> QED BY <2>2, <2>3 DEF Inductive
<1>5. \A d \in Cust : Inductive /\ Abort(d) => Inductive'
  BY SMT DEF Abort, Release, Inductive, StateType, Locks, INV_K, Holders
<1>6. \A d, r \in Cust : Inductive /\ Expire(d, r) => Inductive'
  BY SMT, Parameters DEF Expire
<1>7. \A d \in Cust : Inductive /\ Get(d) => Inductive'
  <2>1. SUFFICES ASSUME NEW d \in Cust, Inductive, Get(d) PROVE Inductive'
    OBVIOUS
  <2>2. Holders \subseteq Holders'
    BY <2>1, SMT DEF Get, Holders, Inductive, StateType
  <2>3. Holders' \subseteq Cust BY SMT DEF Holders
  <2>4. Cardinality(Holders) <= Cardinality(Holders')
    BY <2>2, <2>3, Parameters, FS_Subset, SMT
  <2>5. Cardinality(Holders) \in Nat /\ Cardinality(Holders') \in Nat
    BY <2>2, <2>3, Parameters, FS_Subset, FS_CardinalityType, SMT
  <2> QED BY <2>1, <2>4, <2>5, Parameters, SMT DEF Get, Inductive, StateType, Locks, INV_K
<1>8. \A d, r \in Cust : Inductive /\ Learn(d, r) => Inductive'
  BY SMT DEF Learn, Inductive, StateType, Locks, INV_K, Holders
<1>9. \A d \in Cust : Inductive /\ (Partition(d) \/ Heal(d) \/ MarkDead(d)) => Inductive'
  BY SMT DEF Partition, Heal, MarkDead, Inductive, StateType, Locks, INV_K, Holders
<1>10. Inductive /\ UNCHANGED vars => Inductive'
  BY SMT DEF vars, Inductive, StateType, Locks, INV_K, Holders
<1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7, <1>8, <1>9, <1>10, SMT DEF Next

THEOREM Safety == Spec => [](TypeOK /\ LockedStays /\ INV_K)
<1>1. Spec => []Inductive BY Initial, Step, PTL DEF Spec
<1>2. Inductive => TypeOK /\ LockedStays /\ INV_K
  BY SMT DEF Inductive, StateType, Locks, TypeOK, LockedStays
<1> QED BY <1>1, <1>2, PTL
=============================================================================
