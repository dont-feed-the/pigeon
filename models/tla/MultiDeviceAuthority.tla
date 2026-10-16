----------------------- MODULE MultiDeviceAuthority -----------------------
(* Same-person native/DAV devices, proposal decisions and individual removal.
   This deliberately keeps Authority.tla's one-device-per-person world and
   proof contracts unchanged. One proposal edits an owner's folder; the
   owner's native peer and a member's native peer/two bridge clients observe
   it. Decision events stand for accepted commits whose complete causal
   histories are exchanged. No content, full merge validation, HTTP, client
   authentication or roster additions are modeled here.

   Permission belongs to a person; signatures and removal belong to devices.
   Concurrent accepted/rejected/withdrawn facts use spec 04 section 4.5's
   precedence. Removal suppresses unseen events of exactly its target device,
   while a removal itself survives (ADR 0009/0051). A past event already seen
   by its removal keeps counting. The four Weak modes are regression tests,
   not supported policies. *)
EXTENDS Naturals, FiniteSets, Sequences
CONSTANTS MaxEvents, Weak

Devices == {"owner", "member", "clientA", "clientB"}
Clients == {"clientA", "clientB"}
Person(d) == IF d = "owner" THEN "owner-person" ELSE "member-person"
Kinds == {"accepted", "rejected", "withdrawn", "remove"}
AuthorDevice == "clientA"
AuthorPerson == Person(AuthorDevice)

VARIABLES events, seen
vars == <<events, seen>>
Ids == 1..Len(events)
Room == Len(events) < MaxEvents

(* Genesis contains the proposal already fetched by all devices. It is a
   proposal because its author is not the owner of the changed folder. *)
Init == /\ events = <<>>
        /\ seen = [d \in Devices |-> {}]

Removals(S) == {i \in S : events[i].kind = "remove"}
RemovedBy(i) ==
  IF Weak = "person-remove"
  THEN {d \in Devices : Person(d) = Person(events[i].target)}
  ELSE {events[i].target}
Present(S) == Devices \ UNION {RemovedBy(i) : i \in Removals(S)}
ExpectedPresent(S) == Devices \ {events[i].target : i \in Removals(S)}
Counts(i, S) ==
  \/ events[i].kind = "remove"
  \/ Weak = "ignore-revocation"
  \/ ~\E r \in Removals(S) :
       events[i].signer = events[r].target /\ i \notin events[r].parents
Counting(S) == {i \in S : Counts(i, S)}
Has(S, k) == \E i \in Counting(S) : events[i].kind = k
Status(S) == IF Has(S, "accepted") THEN "accepted"
             ELSE IF Has(S, "rejected") THEN "rejected"
             ELSE IF Has(S, "withdrawn") THEN "withdrawn"
             ELSE "pending"

MayDecide(d, k) ==
  IF k = "withdrawn"
  THEN IF Weak = "device-withdraw" THEN d = AuthorDevice
       ELSE Person(d) = AuthorPerson
  ELSE Weak = "members-decide" \/ Person(d) = "owner-person"
MayRemove(d, target) ==
  Person(d) = "owner-person" \/ Person(d) = Person(target)

Issue(d, k, target) ==
  /\ events' = Append(events, [signer |-> d, kind |-> k,
                               target |-> target, parents |-> seen[d]])
  /\ seen' = [seen EXCEPT ![d] = @ \cup {Len(events) + 1}]
Decide(d, k) ==
  /\ Room /\ k \in Kinds \ {"remove"}
  /\ d \in Present(seen[d]) /\ Status(seen[d]) = "pending"
  /\ MayDecide(d, k)
  /\ Issue(d, k, "none")
Remove(d, target) ==
  /\ Room /\ d \in Present(seen[d]) /\ target \in Present(seen[d])
  /\ MayRemove(d, target)
  /\ Issue(d, "remove", target)
Fetch(d, e) ==
  /\ e # d /\ ~(seen[e] \subseteq seen[d])
  /\ seen' = [seen EXCEPT ![d] = @ \cup seen[e]]
  /\ UNCHANGED events
Next ==
  \/ \E d \in Devices, k \in Kinds \ {"remove"} : Decide(d, k)
  \/ \E d, target \in Devices : Remove(d, target)
  \/ \E d, e \in Devices : Fetch(d, e)
Spec == Init /\ [][Next]_vars

(* A separate small temporal check: once the finite event bound makes
   creation quiet, every never-removed device keeps exchanging histories
   with the other survivors. Removed/offline devices need not converge. *)
Survivors == ExpectedPresent(Ids)
SurvivorFetch(d, e) == d \in Survivors /\ e \in Survivors /\ Fetch(d, e)
Live == Spec /\ \A d, e \in Devices : WF_vars(SurvivorFetch(d, e))
Converge == <>[](\A d, e \in Survivors : seen[d] = seen[e])

TypeOK ==
  /\ Len(events) <= MaxEvents
  /\ seen \in [Devices -> SUBSET Ids]
  /\ \A i \in Ids :
       /\ events[i].signer \in Devices /\ events[i].kind \in Kinds
       /\ events[i].parents \subseteq 1..(i-1)
       /\ events[i].kind = "remove" => events[i].target \in Devices
  /\ \A d \in Devices : \A i \in seen[d] : events[i].parents \subseteq seen[d]

(* Every locally issued action has the authority of its actual signing
   device's person, under the roster that action observed. Transport never
   grants the bridge's control/owner authority to a member client. *)
ActionsAuthorised ==
  \A i \in Ids : LET e == events[i] IN
    /\ e.signer \in ExpectedPresent(e.parents)
    /\ CASE e.kind = "withdrawn" -> Person(e.signer) = AuthorPerson
         [] e.kind = "remove" -> MayRemove(e.signer, e.target)
         [] OTHER -> Person(e.signer) = "owner-person"

(* Check enabledness as well as absence of forbidden writes: modeling only
   safety of issued decisions would miss a sibling's incorrectly denied UI
   action. The event bound is excluded from this availability obligation. *)
SiblingCanWithdraw ==
  Room => \A d \in Devices \ {AuthorDevice} :
    (d \in ExpectedPresent(seen[d]) /\ Person(d) = AuthorPerson
       /\ Status(seen[d]) = "pending") => ENABLED Decide(d, "withdrawn")
RemovalOnlyTarget == \A d \in Devices : Present(seen[d]) = ExpectedPresent(seen[d])
RemovedUnseenSilent ==
  \A d \in Devices : \A i \in Counting(seen[d]) :
    events[i].kind # "remove" =>
      \A r \in Removals(seen[d]) :
        events[i].signer = events[r].target => i \in events[r].parents
SeenBeforeRemovalSurvives ==
  \A d \in Devices : \A i \in seen[d] :
    (\A r \in Removals(seen[d]) :
       events[i].signer = events[r].target => i \in events[r].parents)
      => i \in Counting(seen[d])
SiblingsKeepTheirEvents ==
  \A d \in Devices : \A i \in seen[d] :
    (\A r \in Removals(seen[d]) : events[i].signer # events[r].target)
      => i \in Counting(seen[d])
EqualHistoryAgrees ==
  \A d, e \in Devices : seen[d] = seen[e] => Status(seen[d]) = Status(seen[e])

(* Keep the three-event check focused on one bridge key's removal, rather
   than spending release time exploring permutations of unrelated device
   removals. The two-event checks and weakening configs remain unrestricted. *)
FocusedScenario ==
  /\ Cardinality(Removals(Ids)) <= 1
  /\ \A r \in Removals(Ids) : events[r].target = "clientA"

(* Intentional reachability sentinels, checked by separate negative configs.
   They ensure the good checks exercise concurrency and actual sibling
   authority after an individual client removal rather than vacuous guards. *)
NoConcurrentDecisions ==
  ~\E i, j \in Ids :
    /\ events[i].kind \in Kinds \ {"remove"}
    /\ events[j].kind \in Kinds \ {"remove"}
    /\ events[i].kind # events[j].kind
    /\ i \notin events[j].parents /\ j \notin events[i].parents
NoSiblingAfterRemoval ==
  ~\E i, r \in Ids :
    /\ events[r].kind = "remove" /\ events[r].target \in Clients
    /\ r \in events[i].parents /\ events[i].kind = "withdrawn"
    /\ events[i].signer \in Clients \ {events[r].target}
NoRevokedAndSiblingWithdrawal ==
  ~\E r, stale, sibling \in Ids :
    /\ events[r].kind = "remove" /\ events[r].target = "clientA"
    /\ events[stale].kind = "withdrawn" /\ events[stale].signer = "clientA"
    /\ stale \notin events[r].parents
    /\ events[sibling].kind = "withdrawn" /\ events[sibling].signer = "clientB"
    /\ r \in events[sibling].parents
    /\ \E d \in Devices :
         /\ {r, stale, sibling} \subseteq seen[d]
         /\ stale \notin Counting(seen[d]) /\ sibling \in Counting(seen[d])

View == [events |-> events, seen |-> seen,
         status |-> [d \in Devices |-> Status(seen[d])],
         present |-> [d \in Devices |-> Present(seen[d])]]
=============================================================================
