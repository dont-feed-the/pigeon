---------------------------- MODULE CustodyTokens ----------------------------
(* Custody tokens for ONE blob.
   - K tokens exist forever; a node holds at most one; holding a token = promise to keep the content.
   - Unsync = hand your token to a node that has the content, has no token and wants to keep it,
     then delete once it acknowledges.  No locks, no ids.
   - Network: messages are never consumed (any message may be delivered again, late, reordered)
     and may be lost.  Users change their mind at any time; untokened nodes fetch and evict freely.
   Nonce = FALSE : replies name the token only.  Counterexample: a delayed ack of an earlier hand-off
                   of the same token is mistaken for the current one (ABA) -> both nodes delete.
   Nonce = TRUE  : every offer carries a fresh id; a reply only answers the offer with that id.
   IdempotentAck = TRUE re-acknowledges an offer for a token one already holds (tempting for retries). *)
EXTENDS Naturals, FiniteSets
CONSTANTS N, K, IdempotentAck, Nonce, MaxOffers
Nodes  == 1..N
Tokens == 1..K
VARIABLES has, tok, offering, want, msgs, next
vars == <<has, tok, offering, want, msgs, next>>

M(type, t, id, from, to) == [type |-> type, t |-> t, id |-> id, from |-> from, to |-> to]
None == [to |-> 0, id |-> 0]

Init == /\ has      = [n \in Nodes |-> TRUE]
        /\ tok      = [n \in Nodes |-> IF n <= K THEN {n} ELSE {}]
        /\ offering = [n \in Nodes |-> None]          \* current offer: target and id
        /\ want     \in [Nodes -> BOOLEAN]            \* TRUE = wants to unsync
        /\ msgs     = {}
        /\ next     = 1

ChangeMind(n) == /\ want' = [want EXCEPT ![n] = ~@]
                 /\ UNCHANGED <<has, tok, offering, msgs, next>>

Fetch(n) == /\ ~has[n] /\ \E m \in Nodes : has[m]
            /\ has' = [has EXCEPT ![n] = TRUE]
            /\ UNCHANGED <<tok, offering, want, msgs, next>>

Delete(n) == /\ has[n] /\ tok[n] = {}                 \* the only way content disappears
             /\ has' = [has EXCEPT ![n] = FALSE]
             /\ UNCHANGED <<tok, offering, want, msgs, next>>

Offer(n, m) == /\ want[n] /\ offering[n] = None /\ m # n /\ next <= MaxOffers
               /\ LET id == IF Nonce THEN next ELSE 0 IN
                  /\ \E t \in tok[n] : msgs' = msgs \cup {M("offer", t, id, n, m)}
                  /\ offering' = [offering EXCEPT ![n] = [to |-> m, id |-> id]]
               /\ next' = next + 1
               /\ UNCHANGED <<has, tok, want>>

Receive(m) == \E o \in msgs : o.type = "offer" /\ o.to = m /\
    IF \/ has[m] /\ ~want[m] /\ tok[m] = {}
       \/ IdempotentAck /\ o.t \in tok[m]
    THEN /\ tok'  = [tok EXCEPT ![m] = {o.t}]
         /\ msgs' = msgs \cup {M("ack", o.t, o.id, m, o.from)}
         /\ UNCHANGED <<has, offering, want, next>>
    ELSE /\ msgs' = msgs \cup {M("nack", o.t, o.id, m, o.from)}
         /\ UNCHANGED <<has, tok, offering, want, next>>

Reply(n) == \E r \in msgs : r.to = n /\ offering[n] # None
                         /\ r.from = offering[n].to /\ r.id = offering[n].id /\
    \/ r.type = "ack"  /\ r.t \in tok[n] /\ tok' = [tok EXCEPT ![n] = @ \ {r.t}]
                       /\ offering' = [offering EXCEPT ![n] = None]
                       /\ UNCHANGED <<has, want, msgs, next>>
    \/ r.type = "nack" /\ offering' = [offering EXCEPT ![n] = None]
                       /\ UNCHANGED <<has, tok, want, msgs, next>>

Lose == \E x \in msgs : msgs' = msgs \ {x} /\ UNCHANGED <<has, tok, offering, want, next>>

(* Messages are never consumed and delivery is never forced, so a lost message is just one that is
   never received: Lose adds no behaviour for safety and is left out of Next to keep the state space small. *)
Next == \/ \E n \in Nodes : \/ ChangeMind(n) \/ Fetch(n) \/ Delete(n)
                            \/ Receive(n) \/ Reply(n) \/ \E m \in Nodes : Offer(n, m)
Spec == Init /\ [][Next]_vars

(* ---- safety ---- *)
TokensNeverLost == \A t \in Tokens : \E n \in Nodes : t \in tok[n]
TokenHolderHasContent == \A n \in Nodes : tok[n] # {} => has[n]
KCopies == Cardinality({n \in Nodes : has[n]}) >= K       \* the guarantee
AtMostOneToken == \A n \in Nodes : Cardinality(tok[n]) <= 1
=============================================================================
