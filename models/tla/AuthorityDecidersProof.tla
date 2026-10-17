----------------------- MODULE AuthorityDecidersProof ------------------------
(* The configured adoption-permission guarantee follows from how deciders
   are selected. No claim is made here about the other Authority invariants. *)
EXTENDS Authority, TLAPS

ProofParameters == /\ ForceAdopt = TRUE
                     /\ DecidersAt \in {"decision", "mixed"}
                     /\ ConcreteNames = FALSE

ASSUME Parameters == ProofParameters
USE DEF ProofParameters

THEOREM AdoptPermissions ==
  \A x, p, u : u \in Deciders(p, x) => MayAdopt(u, x, p)
<1>1. SUFFICES ASSUME NEW x, NEW p, NEW u, u \in Deciders(p, x)
  PROVE MayAdopt(u, x, p)
  OBVIOUS
<1>2. CASE DecidersAt = "decision"
  BY <1>1, <1>2, Parameters, SMT DEF Deciders, AbstractDeciders, MayAdopt
<1>3. CASE DecidersAt = "mixed"
  <2>1. ADiff(x, p) \subseteq PDiff(p) /\ ADiff(x, p) \subseteq CPaths
    BY SMT DEF ADiff, AdoptTree
  <2>2. \A q \in ADiff(x, p) :
    AllowedC(u, q, cm[cm[p].cbase].tree[q], cm[p].tree[q], R(x), Counting(HistSet({x}))) =>
      AllowedC(u, q, cm[x].tree[q], AdoptTree(x, p)[q], R(x), Counting(HistSet({x})))
    BY <2>1, SMT DEF AllowedC, AdoptTree
  <2> QED BY <1>1, <1>3, <2>1, <2>2, Parameters, SMT DEF Deciders, AbstractDeciders, MayAdopt
<1> QED BY <1>2, <1>3, Parameters

THEOREM Safety == []DecidersCanAdopt
<1>1. DecidersCanAdopt
  BY AdoptPermissions DEF DecidersCanAdopt
<1> QED BY <1>1, PTL
=============================================================================
