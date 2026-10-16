---------------------------- MODULE PurgeProof -------------------------------
(* Parameterized safety of the actual purge actions. A timely restore or
   cancellation prevents all drops; full seals also enforce the waiting
   period and prevent refetching an already covered reference. *)
EXTENDS Purge, TLAPS

ProofParameters == /\ Custodians # {}
                     /\ Custodians \cap Laptops = {}
                     /\ Decider \in Devices
                     /\ Share \subseteq Custodians
                     /\ LaptopCopies \subseteq Laptops
                     /\ Share \cup LaptopCopies # {}
                     /\ R \in Nat /\ MaxTime \in Nat
                     /\ Mode = "full"

ASSUME Parameters == ProofParameters
USE DEF ProofParameters, know, reports, ghost

StateType ==
  /\ now \in 0..MaxTime /\ dtime \in 0..NoTime /\ firstDrop \in 0..NoTime
  /\ knowsD \in [Devices -> BOOLEAN]
  /\ knowsR1 \in [Devices -> BOOLEAN]
  /\ knowsCancel \in [Devices -> BOOLEAN]
  /\ copy \in [Devices -> BOOLEAN]
  /\ seen \in [Custodians -> 0..NoTime]
  /\ seal \in [Custodians -> [r1: BOOLEAN, cancel: BOOLEAN, made: BOOLEAN]]
  /\ knownSeals \in [Devices -> SUBSET Custodians]
  /\ timely \in BOOLEAN /\ dropped \subseteq Devices /\ refetched \subseteq Devices

Knowledge ==
  /\ (dtime = NoTime <=> \A d \in Devices : ~knowsD[d])
  /\ \A d \in Devices : Keeps(d) => knowsD[d]
  /\ dtime # NoTime => dtime <= now
  /\ \A c \in Custodians : knowsD[c] => dtime <= seen[c] /\ seen[c] <= now

Seals ==
  /\ \A d \in Devices : knownSeals[d] \subseteq {c \in Custodians : seal[c].made}
  /\ \A c \in Custodians : seal[c].made =>
       /\ knowsD[c] /\ now >= dtime + R
       /\ (seal[c].r1 => knowsR1[c]) /\ (seal[c].cancel => knowsCancel[c])

DropHistory ==
  /\ (firstDrop = NoTime <=> dropped = {})
  /\ firstDrop = NoTime => \E d \in Devices : copy[d]
  /\ firstDrop # NoTime => firstDrop >= dtime + R /\ firstDrop <= now
  /\ \A d \in dropped : knowsD[d] /\ Custodians \subseteq knownSeals[d]
  /\ dropped # {} => \A c \in Custodians :
       seal[c].made /\ ~seal[c].r1 /\ ~seal[c].cancel

Witness == timely => \E c \in Custodians :
  Keeps(c) /\ (~seal[c].made \/ seal[c].r1 \/ seal[c].cancel)

Inductive == StateType /\ Knowledge /\ Seals /\ DropHistory /\ Witness /\ NoResurrection

THEOREM InitialCopy == Init => \E d \in Devices : copy[d]
<1>1. PICK d \in Share \cup LaptopCopies : TRUE
  BY Parameters
<1>2. d \in Devices BY <1>1, Parameters, SMT DEF Devices
<1>3. Init => copy[d] BY <1>1, <1>2 DEF Init, Devices
<1> QED BY <1>2, <1>3, SMT

THEOREM Initial == Init => Inductive
<1>1. Init => StateType
  <2>1. Init => now \in 0..MaxTime /\ dtime \in 0..NoTime /\ firstDrop \in 0..NoTime
    BY SMT, Parameters DEF Init, Devices, NoTime
  <2>2. Init => knowsD \in [Devices -> BOOLEAN] /\ knowsR1 \in [Devices -> BOOLEAN]
               /\ knowsCancel \in [Devices -> BOOLEAN] /\ copy \in [Devices -> BOOLEAN]
    BY SMT, Parameters DEF Init, Devices
  <2>3. Init => seen \in [Custodians -> 0..NoTime]
    BY SMT, Parameters DEF Init, NoTime
  <2>4. Init => seal \in [Custodians -> [r1: BOOLEAN, cancel: BOOLEAN, made: BOOLEAN]]
    BY SMT, Parameters DEF Init, NoSeal
  <2>5. Init => knownSeals \in [Devices -> SUBSET Custodians]
    BY SMT, Parameters DEF Init, Devices
  <2>6. Init => timely \in BOOLEAN /\ dropped \subseteq Devices /\ refetched \subseteq Devices
    BY DEF Init, Devices
  <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6 DEF StateType
<1>2. Init => Knowledge
  BY SMT, Parameters DEF Init, Knowledge, Keeps, Devices, NoTime
<1>3. Init => Seals
  BY SMT, Parameters DEF Init, Seals, Devices, NoTime, NoSeal
<1>4. Init => DropHistory
  <2>1. Init => firstDrop = NoTime /\ dropped = {}
    BY DEF Init
  <2>2. Init => \E d \in Devices : copy[d]
    BY InitialCopy
  <2> QED BY <2>1, <2>2 DEF DropHistory
<1>5. Init => Witness /\ NoResurrection
  BY SMT DEF Init, Witness, NoResurrection
<1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5 DEF Inductive

THEOREM FullRelease == \A d \in Devices : Released(d) <=>
  /\ knowsD[d] /\ ~Keeps(d) /\ Custodians \subseteq knownSeals[d]
  /\ \A c \in Custodians :
       (seal[c].r1 => knowsR1[d]) /\ (seal[c].cancel => knowsCancel[d])
BY Parameters DEF Released

THEOREM ReleaseBarrier == \A d \in Devices :
  Inductive /\ Released(d) =>
    /\ \A c \in Custodians : seal[c].made /\ ~seal[c].r1 /\ ~seal[c].cancel
    /\ now >= dtime + R
    /\ ~timely
BY SMT, Parameters, FullRelease DEF Inductive, StateType, Knowledge, Seals, DropHistory,
  Witness, Keeps, Devices

THEOREM Step == Inductive /\ [Next]_vars => Inductive'
<1>1. SUFFICES ASSUME Inductive, [Next]_vars PROVE Inductive'
  OBVIOUS
<1>2. Inductive /\ Tick => Inductive'
  BY SMT, Parameters DEF Tick, Inductive, StateType, Knowledge, Seals,
    DropHistory, Witness, NoResurrection, Keeps, Devices, NoTime
<1>3. Inductive /\ Decide => Inductive'
  BY SMT, Parameters DEF Decide, Inductive, StateType, Knowledge, Seals,
    DropHistory, Witness, NoResurrection, Keeps, Devices, NoTime
<1>4. Inductive /\ Cancel => Inductive'
  BY SMT, Parameters DEF Cancel, Inductive, StateType, Knowledge, Seals,
    DropHistory, Witness, NoResurrection, Keeps, Unsealed, Devices, NoTime
<1>5. \A d \in Devices : Inductive /\ Restore(d) => Inductive'
  BY SMT, Parameters DEF Restore, Inductive, StateType, Knowledge, Seals,
    DropHistory, Witness, NoResurrection, Keeps, Unsealed, Devices, NoTime
<1>6. \A a, b \in Devices : Inductive /\ SyncHistory(a, b) => Inductive'
  BY SMT, Parameters DEF SyncHistory, Inductive, StateType, Knowledge, Seals,
    DropHistory, Witness, NoResurrection, Keeps, Unsealed, Devices, NoTime
<1>7. \A a, b \in Devices : Inductive /\ SyncReports(a, b) => Inductive'
  BY SMT, Parameters DEF SyncReports, Inductive, StateType, Knowledge, Seals,
    DropHistory, Witness, NoResurrection, Keeps, Devices, NoTime
<1>8. \A c \in Custodians : Inductive /\ Seal(c) => Inductive'
  BY SMT, Parameters DEF Seal, Inductive, StateType, Knowledge, Seals,
    DropHistory, Witness, NoResurrection, Keeps, Devices, NoTime
<1>9. \A d \in Devices : Inductive /\ Drop(d) => Inductive'
  BY SMT, Parameters, ReleaseBarrier, FullRelease DEF Drop, Inductive, StateType, Knowledge, Seals,
    DropHistory, Witness, NoResurrection, Keeps, Devices, NoTime
<1>10. \A d, from \in Devices : Inductive /\ Fetch(d, from) => Inductive'
  BY SMT, Parameters, FullRelease DEF Fetch, Wants, Inductive, StateType, Knowledge, Seals,
    DropHistory, Witness, NoResurrection, Keeps, Devices, NoTime
<1>11. \A c \in Custodians : Inductive /\ Restart(c) => Inductive'
  BY SMT, Parameters DEF Restart, Inductive, StateType, Knowledge, Seals,
    DropHistory, Witness, NoResurrection, Keeps, Devices, NoTime
<1>12. Inductive /\ UNCHANGED vars => Inductive'
  BY SMT DEF vars, know, reports, ghost, Inductive, StateType, Knowledge,
    Seals, DropHistory, Witness, NoResurrection, Keeps, Devices, NoTime
<1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7, <1>8, <1>9, <1>10,
  <1>11, <1>12, SMT DEF Next

THEOREM Safety == Spec => [](KeepWins /\ WaitingPeriod /\ NoResurrection)
<1>1. Spec => []Inductive BY Initial, Step, PTL DEF Spec
<1>2. Inductive => KeepWins /\ WaitingPeriod /\ NoResurrection
  BY SMT DEF Inductive, StateType, DropHistory, Witness, Keeps, KeepWins, WaitingPeriod
<1> QED BY <1>1, <1>2, PTL
=============================================================================
