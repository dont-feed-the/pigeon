------------------------ MODULE BridgeSharedStorage ------------------------
(* One content key, two managed clients, and one shared host. This models the
   pool's retain/verify/rename/fsync/relink boundary, not filesystem internals.
   References denote inode aliases. A writable laptop source may change while
   being retained; an immutable custodian source may be linked directly.
   Crash loses unpublished work and may lose an unsynced rename. Pool objects
   are append-only: private-reference drops are not coordinated pool GC.
   Remote denotes one independent custodian, never another managed client.
   Bad switches are regression controls, not supported implementation modes. *)
EXTENDS Naturals, FiniteSets
CONSTANTS Mutable, LinkMutable, EarlyAck, CountAliases, RetrySync
Peers == {"custodian", "desktop", "phone"}
Clients == Peers \ {"custodian"}
None == "none"
Inodes == {None, "native", "snapshot"}
VARIABLE s
vars == <<s>>
Init == s = [pool |-> None, durable |-> FALSE,
  refs |-> [p \in Peers |-> IF p = "custodian" THEN "native" ELSE None],
  stable |-> [p \in Peers |-> IF p = "custodian" THEN "native" ELSE None],
  nativeValue |-> 0, snapshotValue |-> 0, holder |-> None,
  phase |-> "idle", stage |-> None, online |-> TRUE,
  acknowledged |-> FALSE, remote |-> FALSE, unsafeRead |-> FALSE]
Value(i) == IF i = "native" THEN s.nativeValue ELSE s.snapshotValue

Begin(p) ==
  /\ s.online /\ s.holder = None /\ s.pool = None
  /\ s.refs["custodian"] = "native"
  /\ s' = [s EXCEPT !.holder = p, !.phase = "stage",
       !.stage = IF Mutable /\ ~LinkMutable THEN "snapshot" ELSE "native",
       !.snapshotValue = s.nativeValue]
Verify ==
  /\ s.online /\ s.phase = "stage" /\ Value(s.stage) = 0
  /\ s' = [s EXCEPT !.phase = "verified"]
Abort ==
  /\ s.online /\ s.phase = "stage" /\ Value(s.stage) # 0
  /\ s' = [s EXCEPT !.holder = None, !.phase = "idle", !.stage = None]
Publish ==
  /\ s.online /\ s.phase = "verified"
  /\ s' = [s EXCEPT !.pool = s.stage, !.phase = "published",
       !.acknowledged = EarlyAck /\ s.holder = "custodian" /\ ~Mutable]
Sync ==
  /\ s.online /\ s.phase = "published"
  /\ s' = [s EXCEPT !.durable = TRUE, !.phase = "ready"]
Reuse(p) ==
  /\ s.online /\ s.holder = None /\ s.pool # None
  /\ Value(s.pool) = 0
  /\ s' = [s EXCEPT !.holder = p,
       !.phase = IF RetrySync THEN "published" ELSE "ready"]
Relink ==
  /\ s.online /\ s.phase = "ready"
  (* A laptop's writable working file is never relinked to the pool. *)
  /\ (~Mutable \/ s.holder # "custodian")
  /\ s' = [s EXCEPT !.refs[s.holder] = s.pool, !.phase = "linked"]
Finish ==
  /\ s.online /\ s.phase = "linked"
  /\ s' = [s EXCEPT !.stable[s.holder] = s.refs[s.holder],
       !.acknowledged = IF s.holder = "custodian" THEN TRUE ELSE @,
       !.holder = None, !.phase = "idle", !.stage = None]
(* A laptop can finish retain without replacing its mutable source. *)
FinishLaptop ==
  /\ s.online /\ Mutable /\ s.phase = "ready" /\ s.holder = "custodian"
  /\ s' = [s EXCEPT !.holder = None, !.phase = "idle", !.stage = None]
WriteLaptop ==
  /\ s.online /\ Mutable /\ s.nativeValue = 0
  /\ s' = [s EXCEPT !.nativeValue = 1]
Drop(p) ==
  /\ s.online /\ s.holder # p /\ s.refs[p] # None
  /\ s' = [s EXCEPT !.refs[p] = None, !.stable[p] = None,
       !.acknowledged = IF p = "custodian" THEN FALSE ELSE @]
Read(p) ==
  /\ s.online /\ p \in Clients /\ s.refs[p] # None
  /\ s' = [s EXCEPT !.unsafeRead = @ \/ Value(s.refs[p]) # 0]
(* A failed syscall can unwind after rename, while the host stays up. The
   subsequent call sees the target but must not infer its rename was synced. *)
Interrupt ==
  /\ s.online /\ s.phase \in {"published", "linked"}
  /\ s' = [s EXCEPT !.holder = None, !.phase = "idle", !.stage = None]
Crash ==
  /\ s.online
  /\ \E surviving \in IF s.durable THEN {s.pool} ELSE {None, s.pool} :
     s' = [s EXCEPT !.online = FALSE, !.pool = surviving,
       !.durable = surviving # None, !.refs = s.stable,
       !.holder = None, !.phase = "idle", !.stage = None]
Recover == /\ ~s.online /\ s' = [s EXCEPT !.online = TRUE]
RemoteRetain == /\ ~s.remote /\ s' = [s EXCEPT !.remote = TRUE]
RemoteDrop == /\ s.remote /\ s' = [s EXCEPT !.remote = FALSE]
Next ==
  \/ \E p \in Peers : Begin(p) \/ Reuse(p) \/ Drop(p)
  \/ \E p \in Clients : Read(p)
  \/ Verify \/ Abort \/ Publish \/ Sync \/ Relink \/ Finish \/ FinishLaptop
  \/ WriteLaptop \/ Interrupt \/ Crash \/ Recover \/ RemoteRetain \/ RemoteDrop
Spec == Init /\ [][Next]_vars

TypeOK == /\ s.pool \in Inodes /\ s.refs \in [Peers -> Inodes]
          /\ s.stable \in [Peers -> Inodes] /\ s.nativeValue \in {0, 1}
PoolImmutable == s.pool = None \/ (~Mutable \/ s.pool # "native")
PoolValid == s.pool = None \/ Value(s.pool) = 0
PinnedAcknowledgement == ~s.acknowledged \/
  (s.durable /\ s.pool # None /\ s.refs["custodian"] = s.pool
   /\ s.stable["custodian"] = s.pool /\ Value(s.pool) = 0)
ClientReferencesValid == \A p \in Clients : s.refs[p] = None \/
  (s.durable /\ s.refs[p] = s.pool /\ Value(s.refs[p]) = 0)
ReadsValid == ~s.unsafeRead
(* Protected counts stored custody domains, not presently reachable servers.
   A host crash takes every local alias offline together. Clients aren't
   custodians, so even many aliases add no custody domain. *)
LocalCustody == ~Mutable /\ s.stable["custodian"] # None
CustodyDomains == (IF LocalCustody THEN {"shared-host"} ELSE {})
                 \cup (IF s.remote THEN {"remote-host"} ELSE {})
ClaimedCopies == Cardinality(CustodyDomains) +
  IF CountAliases THEN Cardinality({p \in Clients : s.refs[p] # None}) ELSE 0
IndependentCustody == ClaimedCopies <= Cardinality(CustodyDomains)
=============================================================================
