------------------------ MODULE CustodyTokensProof ---------------------------
(* Prove the original nonce hand-off protocol for arbitrary node/token counts
   and offer bounds. A token always has a holder whose pending offer is newer
   than every acknowledgement for that token, so no old reply can remove the
   last such holder. Messages may be replayed without any delivery bound. *)
EXTENDS CustodyTokens, TLAPS, FiniteSetTheorems

ProofParameters == /\ N \in Nat /\ K \in Nat /\ K <= N
                     /\ MaxOffers \in Nat
                     /\ Nonce = TRUE /\ IdempotentAck = FALSE

ASSUME Parameters == ProofParameters
USE DEF ProofParameters

StateType ==
  /\ next \in Nat /\ next >= 1
  /\ has \in [Nodes -> BOOLEAN] /\ want \in [Nodes -> BOOLEAN]
  /\ tok \in [Nodes -> SUBSET Tokens]
  /\ offering \in [Nodes -> {None} \cup [to: Nodes, id: 1..(next - 1)]]
  /\ msgs \subseteq [type: {"offer", "ack", "nack"}, t: Tokens,
                     id: 1..(next - 1), from: Nodes, to: Nodes]

SingleTokens == \A n \in Nodes : tok[n] = {} \/ \E t \in Tokens : tok[n] = {t}
ActiveOffers == \A n \in Nodes : offering[n] # None => tok[n] # {}
FreshHolder(t, n) ==
  /\ t \in tok[n]
  /\ \A r \in msgs : r.type = "ack" /\ r.t = t =>
       offering[n] = None \/ r.id < offering[n].id
Witnesses == \A t \in Tokens : \E n \in Nodes : FreshHolder(t, n)
Inductive == StateType /\ SingleTokens /\ ActiveOffers /\ Witnesses
             /\ TokenHolderHasContent

THEOREM Initial == Init => Inductive
BY SMT, Parameters DEF Init, Inductive, StateType, SingleTokens, ActiveOffers,
  Witnesses, FreshHolder, TokenHolderHasContent, Nodes, Tokens, None

THEOREM Step == Inductive /\ [Next]_vars => Inductive'
<1>1. SUFFICES ASSUME Inductive, [Next]_vars PROVE Inductive'
  OBVIOUS
<1>2. \A n \in Nodes : Inductive /\ ChangeMind(n) => Inductive'
  BY SMT DEF ChangeMind, Inductive, StateType, SingleTokens, ActiveOffers,
    Witnesses, FreshHolder, TokenHolderHasContent
<1>3. \A n \in Nodes : Inductive /\ Fetch(n) => Inductive'
  BY SMT DEF Fetch, Inductive, StateType, SingleTokens, ActiveOffers,
    Witnesses, FreshHolder, TokenHolderHasContent
<1>4. \A n \in Nodes : Inductive /\ Delete(n) => Inductive'
  BY SMT DEF Delete, Inductive, StateType, SingleTokens, ActiveOffers,
    Witnesses, FreshHolder, TokenHolderHasContent
<1>5. \A n, m \in Nodes : Inductive /\ Offer(n, m) => Inductive'
  BY SMT, Parameters DEF Offer, Inductive, StateType, SingleTokens, ActiveOffers,
    Witnesses, FreshHolder, TokenHolderHasContent, M, None
<1>6. \A n \in Nodes : Inductive /\ Receive(n) => Inductive'
  BY SMT, Parameters DEF Receive, Inductive, StateType, SingleTokens, ActiveOffers,
    Witnesses, FreshHolder, TokenHolderHasContent, M, None
<1>7. \A n \in Nodes : Inductive /\ Reply(n) => Inductive'
  BY SMT DEF Reply, Inductive, StateType, SingleTokens, ActiveOffers,
    Witnesses, FreshHolder, TokenHolderHasContent, None
<1>8. Inductive /\ UNCHANGED vars => Inductive'
  BY SMT DEF vars, Inductive, StateType, SingleTokens, ActiveOffers,
    Witnesses, FreshHolder, TokenHolderHasContent
<1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7, <1>8, SMT DEF Next

Holders == {n \in Nodes : has[n]}
Holder(t) == CHOOSE n \in Nodes : t \in tok[n]

THEOREM Copies == Inductive => KCopies
<1>1. SUFFICES ASSUME Inductive PROVE KCopies
  OBVIOUS
<1>2. \A t \in Tokens : Holder(t) \in Holders /\ t \in tok[Holder(t)]
  BY <1>1, SMT DEF Inductive, StateType, Witnesses, FreshHolder,
    TokenHolderHasContent, Holder, Holders
<1>3. [t \in Tokens |-> Holder(t)] \in Injection(Tokens, Holders)
  <2>1. [t \in Tokens |-> Holder(t)] \in [Tokens -> Holders]
    BY <1>2, SMT
  <2>2. \A t, u \in Tokens : Holder(t) = Holder(u) => t = u
    <3>1. SUFFICES ASSUME NEW t \in Tokens, NEW u \in Tokens,
                        Holder(t) = Holder(u) PROVE t = u
      OBVIOUS
    <3>2. Holder(t) \in Nodes /\ t \in tok[Holder(t)] /\ u \in tok[Holder(t)]
      BY <3>1, <1>2, SMT DEF Holders
    <3>3. tok[Holder(t)] = {} \/ \E v \in Tokens : tok[Holder(t)] = {v}
      BY <1>1, <3>2 DEF Inductive, SingleTokens
    <3> QED BY <3>2, <3>3, SMT
  <2> QED BY <2>1, <2>2, SMT DEF Injection, IsInjective
<1>4. IsFiniteSet(Nodes) /\ Cardinality(Tokens) = K
  BY Parameters, FS_Interval, SMT DEF Nodes, Tokens
<1>5. IsFiniteSet(Holders)
  BY <1>4, FS_Subset, SMT DEF Holders
<1>6. Cardinality(Tokens) <= Cardinality(Holders)
  BY <1>3, <1>5, FS_Injection
<1> QED BY <1>4, <1>6 DEF KCopies, Holders

THEOREM Safety == Spec => [](TokensNeverLost /\ TokenHolderHasContent
                            /\ KCopies /\ AtMostOneToken)
<1>1. Spec => []Inductive BY Initial, Step, PTL DEF Spec
<1>2. Inductive => TokensNeverLost /\ TokenHolderHasContent
  BY SMT DEF Inductive, Witnesses, FreshHolder, TokensNeverLost
<1>3. Inductive => AtMostOneToken
  BY FS_EmptySet, FS_Singleton, SMT DEF Inductive, SingleTokens, AtMostOneToken
<1>4. Inductive => KCopies BY Copies
<1> QED BY <1>1, <1>2, <1>3, <1>4, PTL
=============================================================================
