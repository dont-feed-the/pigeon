----------------------- MODULE BridgeLifecycle -----------------------
(* Persisted per-client identity and the local RPC authorization boundary.
   Roster removal is deliberately not the local revocation linearization:
   a committed revoke closes local access even if stopping/removal fails. *)
EXTENDS Naturals, FiniteSets, Sequences, TLC
CONSTANT WeakRevocation
VARIABLE state

InitState == [started |-> FALSE, clients |-> {}, controls |-> {},
              checked |-> {}, safe |-> TRUE]
Ids(cs) == {c.id : c \in cs}
Client(cs, id) == CHOOSE c \in cs : c.id = id
Distinct(cs) == /\ Cardinality(Ids(cs)) = Cardinality(cs)
                /\ \A a, b \in cs : a.id # b.id =>
                    /\ a.device # b.device /\ a.annex # b.annex
                    /\ a.socket # b.socket
SameIdentity(a, b) == /\ a.id = b.id /\ a.user = b.user
                     /\ a.device = b.device /\ a.annex = b.annex
                     /\ a.socket = b.socket
Stable(old, new) == /\ Ids(old) = Ids(new)
                    /\ \A a \in old : LET b == Client(new, a.id) IN
                         /\ SameIdentity(a, b) /\ a.revoked = b.revoked

Start(s, e, t) ==
  LET cs == {e.clients[i] : i \in 1..Len(e.clients)}
      controls == {e.controls[i] : i \in 1..Len(e.controls)}
  IN /\ Distinct(cs)
     /\ ~s.started \/ Stable(s.clients, cs)
     /\ t = [s EXCEPT !.started = TRUE, !.clients = cs,
                       !.controls = controls, !.checked = {}]

Register(s, e, t) ==
  /\ s.started /\ e.id \notin Ids(s.clients) /\ ~e.revoked
  /\ Distinct(s.clients \cup {e})
  /\ t = [s EXCEPT !.clients = @ \cup {e}]

Revoke(s, e, t) ==
  /\ e.id \in Ids(s.clients)
  /\ t = [s EXCEPT
       !.clients = {IF c.id = e.id THEN [c EXCEPT !.revoked = TRUE] ELSE c
                    : c \in s.clients},
       !.checked = @ \ {e.id}]

Check(s, e, t) ==
  /\ e.id \in Ids(s.clients)
  /\ LET c == Client(s.clients, e.id)
         good == e.socket = c.socket /\ e.device = c.device
                 /\ e.annex = c.annex /\ e.shared
     IN /\ e.accepted = good
        /\ t = [s EXCEPT
             !.checked = IF e.accepted THEN @ \cup {e.id} ELSE @ \ {e.id},
             !.safe = @ /\ (e.accepted => ~c.revoked)]

Route(s, e, t) ==
  /\ e.id \in Ids(s.clients)
  /\ LET c == Client(s.clients, e.id) IN
     t = [s EXCEPT !.safe = @ /\ (~c.revoked /\ e.id \in s.checked
                                /\ c.user = e.user /\ c.socket = e.socket)]

Control(s, e, t) ==
  t = [s EXCEPT !.safe = @ /\ e \in s.controls]

Remove(s, e, t) ==
  /\ e.id \in Ids(s.clients)
  /\ LET c == Client(s.clients, e.id) IN
     t = [s EXCEPT !.safe = @ /\ c.revoked /\ e.device = c.device]

Step(s, event, t) == CASE event.action = "Start" -> Start(s, event.vars, t)
 [] event.action = "Register" -> Register(s, event.vars, t)
 [] event.action = "Revoke" -> Revoke(s, event.vars, t)
 [] event.action = "Check" -> Check(s, event.vars, t)
 [] event.action = "Route" -> Route(s, event.vars, t)
 [] event.action = "Control" -> Control(s, event.vars, t)
 [] event.action = "Remove" -> Remove(s, event.vars, t)
 [] OTHER -> FALSE

(* Two clients of the SAME person. Checked calls may finish before revoke;
   requests queued behind revoke must reload persisted authorization. *)
Clients == {[id |-> i, user |-> "person", device |-> i, annex |-> i,
             socket |-> i, revoked |-> FALSE] : i \in {1, 2}}
ControlPeer == [user |-> "person", socket |-> 3]
Init == state = [InitState EXCEPT !.started = TRUE, !.controls = {ControlPeer}]
Next ==
  \/ \E c \in Clients : Register(state, c, state')
  \/ \E id \in Ids(state.clients) : Revoke(state, [id |-> id], state')
  \/ \E c \in state.clients :
       /\ ~c.revoked \/ WeakRevocation
       /\ Check(state, [id |-> c.id, socket |-> c.socket, device |-> c.device,
                        annex |-> c.annex, shared |-> TRUE, accepted |-> TRUE], state')
  \/ \E c \in state.clients :
       /\ ~c.revoked /\ c.id \in state.checked
       /\ Route(state, [id |-> c.id, user |-> c.user, socket |-> c.socket], state')
  \/ \E c \in state.clients :
       /\ c.revoked
       /\ Remove(state, [id |-> c.id, device |-> c.device], state')
  \/ Control(state, ControlPeer, state')
  \/ state' = [state EXCEPT !.checked = {}]
Spec == Init /\ [][Next]_state
Safety == state.safe /\ Distinct(state.clients)
=============================================================================
