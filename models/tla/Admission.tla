------------------------------ MODULE Admission ------------------------------
(* Admission on one custodian (spec 08 §8.9; pigeon-core `capacity`): the
   keys it wants and lacks, each with a size, some endangered, and its room,
   the free space above its reserve less the downloads in progress. Each key
   is admitted or deferred once. An endangered key is admitted if it fits in
   what is left; any other key if it fits with the endangered keys not yet
   admitted. With keys of one unit this is the guard of Repair's StartGet:
   HasRoom, and for a key not endangered Free - 1 >= Cardinality(EndWanted).
   Sizes, room and the endangered set are chosen at the start, so one
   configuration checks every small instance; AdmitTrace reads them from a
   log of the code instead. Off names a safeguard a negative configuration
   removes: "prio" (the endangered keys' room), "used" (what was admitted). *)
EXTENDS Integers, FiniteSets

CONSTANTS Keys,      \* the keys the custodian wants and lacks
          MaxSize,   \* sizes range over 0..MaxSize
          MaxRoom,   \* the room ranges over 0..MaxRoom
          Off        \* safeguards removed

VARIABLES size,       \* size[k]: the bytes of k
          room,       \* the room admission starts with
          endangered, \* the endangered keys
          started,    \* the keys admitted
          deferred    \* the keys deferred
vars == <<size, room, endangered, started, deferred>>

RECURSIVE SumOver(_)
SumOver(S) == IF S = {} THEN 0
              ELSE LET k == CHOOSE x \in S : TRUE IN size[k] + SumOver(S \ {k})

Used == SumOver(started)
Free == room - Used
\* The room kept for the endangered keys not yet admitted.
Held == SumOver(endangered \ started)
Fits(k) == LET left == IF "used" \in Off THEN room ELSE Free
               keep == IF k \in endangered \/ "prio" \in Off THEN 0 ELSE Held
           IN size[k] + keep <= left

Init == /\ size \in [Keys -> 0..MaxSize]
        /\ room \in 0..MaxRoom
        /\ endangered \in SUBSET Keys
        /\ started = {} /\ deferred = {}

Open(k) == k \in Keys \ (started \cup deferred)

Admit(k) == /\ Open(k) /\ Fits(k)
            /\ started' = started \cup {k}
            /\ UNCHANGED <<size, room, endangered, deferred>>

Defer(k) == /\ Open(k) /\ ~Fits(k)
            /\ deferred' = deferred \cup {k}
            /\ UNCHANGED <<size, room, endangered, started>>

\* Every key is planned: the plan is complete.
Done == started \cup deferred = Keys /\ UNCHANGED vars

Next == (\E k \in Keys : Admit(k) \/ Defer(k)) \/ Done

Spec == Init /\ [][Next]_vars

\* The reserve is never crossed: what is admitted fits in the room.
ReserveKept == Used <= room
\* An endangered key's room never goes to another: once a key that is not
\* endangered is admitted, every endangered key not yet admitted still fits.
EndangeredFirst == (started \ endangered # {}) => Held <= Free
\* A deferral is final: a deferred key would not fit now either.
DeferredStays == \A k \in deferred : ~Fits(k)
=============================================================================
