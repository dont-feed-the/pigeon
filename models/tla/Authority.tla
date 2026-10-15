------------------------------ MODULE Authority ------------------------------
(* Acceptance of commits, proposals and decisions, and the removal of a device
   (spec 04 §4.3-4.5, 09 §9.7; ADRs 0009 and 0051).
   Persons 1 = a (the admin), 2 = b (owner of folder F), 3 = c (a member).
   Device i is person i's; device 4 is a fresh key that c may add. Devices 1
   and 2 are honest pigeonds. With Hostile, devices 3 and 4 belong to an
   attacker who signs whatever it likes on any parent it knows: a faulty
   member's device, or c's stolen laptop once the admin removes it.
   Paths: "f" in F (owned by b), "r" at the root (add-only, the admins are its
   stewards). A path's value is the set of versions the catalogue keeps there
   (a conflict keeps several); version n is the content commit n wrote.
   The catalogue is the abstraction of models/catalogue: the values of the
   newest changes of each path (spec 05 §5.2).
   Modes (the design first; each other value removes a safeguard):
     Revocation   "revoke": a removal revokes its device's commits that it had
                  not seen, and the devices they added; "contentonly": the
                  commits only; "none": the rule of 04 §4.4 alone; "tip": 09
                  §9.7, pigeond merges no head signed by a device absent from
                  its main's roster.
     DecidersAt   "decision": the persons who may make the adoption's change
                  on the decision's main, under its roster; "mixed": the
                  proposal's own diff under the decision's roster; "proposal":
                  its diff under the roster at its parents (ADR 0051's words).
     ForceAdopt   the admins adopt with Force what nobody else may.
     DecisionMerge  a commit with a parent outside A(c) must adopt it.
     Rule3        "every" accepted parent's roster, or the "first" one only.
     BaseOf       "accepted": the base of a commit is the catalogue of its
                  accepted history; "all": proposals count too.
     DecisionRule "deciders" (04 §4.3 table) or any "members". *)
EXTENDS Naturals, FiniteSets, Sequences
CONSTANTS MaxCommits, MaxHostile, Hostile, Ops,
          Revocation, DecidersAt, ForceAdopt, DecisionMerge, Rule3, BaseOf, DecisionRule

Persons == {1, 2, 3}
Devices == {1, 2, 3, 4}
Honest == {1, 2}
Attacker == IF Hostile THEN {3, 4} ELSE {}
CPaths == {"f", "r"}

VARIABLES cm, head, has
vars == <<cm, head, has>>

(* A roster: admins, the owner of F, device -> person (0 = absent), devices removed. *)
Genesis == [adm |-> {1}, own |-> 2, dev |-> [d \in Devices |-> IF d <= 3 THEN d ELSE 0],
            gone |-> {}]
Present(Ro) == {d \in Devices : Ro.dev[d] # 0}

(* Roster edits, as in Roster.tla: "own" is a compare-and-set, "add" applies if
   absent or equal, "rem" always applies. *)
Apply1(Ro, e) ==
  CASE e.k = "own" -> IF Ro.own = e.old THEN [ok |-> TRUE, s |-> [Ro EXCEPT !.own = e.new]]
                      ELSE [ok |-> Ro.own = e.new, s |-> Ro]
    [] e.k = "add" -> IF Ro.dev[e.d] = e.p THEN [ok |-> TRUE, s |-> Ro]
                      ELSE IF Ro.dev[e.d] = 0 /\ e.d \notin Ro.gone
                           THEN [ok |-> TRUE, s |-> [Ro EXCEPT !.dev[e.d] = e.p]]
                           ELSE [ok |-> FALSE, s |-> Ro]
    [] e.k = "rem" -> [ok |-> TRUE, s |-> [Ro EXCEPT !.dev[e.d] = 0, !.gone = @ \cup {e.d}]]
RECURSIVE ApplyAll(_, _)
ApplyAll(r, E) == IF E = {} THEN r
                  ELSE LET e == CHOOSE x \in E : TRUE
                           n == Apply1(r.s, e)
                       IN ApplyAll([ok |-> r.ok /\ n.ok, s |-> n.s], E \ {e})
Change(Ro, E) ==
  LET r == ApplyAll([ok |-> TRUE, s |-> Ro], E)
      kept == ApplyAll([ok |-> TRUE, s |-> Ro], {e \in E : e.k = "rem"}).s
  IN IF r.ok /\ r.s.adm # {} THEN [ok |-> TRUE, s |-> r.s] ELSE [ok |-> FALSE, s |-> kept]

(* Commits: cm[i] = [parents, signer, kind, tree, dec, redits, force, and, set
   when it is written and never changed: acc (the verdict of 04 §4.4), delta
   (the paths it changes against its base), cbase (a proposal's nearest
   accepted ancestor), anc]. Parents have smaller ids, so increasing id is a
   canonical topological order (the fold's order is checked by Roster.tla). *)
Ids == 1..Len(cm)
IsProposal(x) == x # 1 /\ ~cm[x].acc
(* The fold of the commits S, of which those in C count: a counting change
   applies as in Roster.tla; of the others only the removals apply, as of a
   skipped change (ADR 0051). *)
RECURSIVE FoldUpTo(_, _, _, _)
FoldUpTo(S, C, Ro, i) ==
  IF i > Len(cm) THEN Ro
  ELSE FoldUpTo(S, C, IF i \in C THEN Change(Ro, cm[i].redits).s
                      ELSE IF i \in S THEN Change(Ro, {e \in cm[i].redits : e.k = "rem"}).s
                      ELSE Ro, i + 1)
Fold(S, C) == FoldUpTo(S, C, Genesis, 1)

(* Revocation: in a set S, x is revoked when S holds a removal of x's signer
   that does not have x as an ancestor, or (Revocation = "revoke") when x's
   signer is not a device of the roster folded from x's ancestors. A revoked
   commit stays accepted, but its changes count in no catalogue and no fold,
   except its removals. *)
Removes(r, d) == \E e \in cm[r].redits : e.k = "rem" /\ e.d = d
Unseen(x, S) == \E r \in S : r # x /\ Removes(r, cm[x].signer) /\ x \notin cm[r].anc
RECURSIVE CountUpTo(_, _, _)
CountUpTo(S, C, i) ==
  IF i > Len(cm) THEN C
  ELSE CountUpTo(S, IF /\ i \in S
                       /\ \/ i = 1
                          \/ /\ ~Unseen(i, S)
                             /\ \/ Revocation = "contentonly"
                                \/ cm[i].signer \in Present(Fold(S \cap (cm[i].anc \ {i}), C \cap cm[i].anc))
                    THEN C \cup {i} ELSE C, i + 1)
Counting(S) == IF Revocation \in {"revoke", "contentonly"} THEN CountUpTo(S, {}, 1) ELSE S
RosterOf(S) == Fold(S, IF Revocation = "contentonly" THEN S ELSE Counting(S))

Hist(X) == UNION {cm[x].anc : x \in X}
HistSet(X) == IF BaseOf = "accepted" THEN {y \in Hist(X) : cm[y].acc} ELSE Hist(X)
Contrib(S, q) == {x \in S : q \in cm[x].delta}
Newest(S, q) == {x \in Contrib(S, q) : \A y \in Contrib(S, q) : y = x \/ x \notin cm[y].anc}
Cat(S) == [q \in CPaths |-> UNION {cm[x].tree[q] : x \in Newest(S, q)}]
TreeOf(X) == Cat(Counting(HistSet(X)))
R(x) == RosterOf(HistSet({x}))           \* the roster in force for x's children

(* Folder rules (04 §4.3) for person u changing q from old to new: F is
   b's; at the root anyone adds, and only the admins, its stewards, change
   or delete. A value that keeps every version of old only adds: a version
   beside them is a conflict copy, a new file (05 §5.3). *)
AllowedC(u, q, old, new, Ro) == IF q = "f" THEN u = Ro.own ELSE old \subseteq new \/ u \in Ro.adm

(* Proposals (04 §4.5). The adoption on main x: a path the proposal changed
   gets its value, or both values if x changed it meanwhile. *)
PDiff(p) == {q \in CPaths : cm[p].tree[q] # cm[cm[p].cbase].tree[q]}
AdoptTree(x, p) ==
  [q \in CPaths |-> IF q \in PDiff(p)
                    THEN IF cm[x].tree[q] = cm[cm[p].cbase].tree[q] THEN cm[p].tree[q]
                         ELSE cm[x].tree[q] \cup cm[p].tree[q]
                    ELSE cm[x].tree[q]]
ADiff(x, p) == {q \in CPaths : AdoptTree(x, p)[q] # cm[x].tree[q]}
(* Deciders of p for a decision on main x; if nobody may, the admins. *)
Deciders(p, x) ==
  LET Ro == IF DecidersAt = "proposal" THEN R(cm[p].cbase) ELSE R(x)
      diff == IF DecidersAt = "decision" THEN ADiff(x, p) ELSE PDiff(p)
      old(q) == IF DecidersAt = "decision" THEN cm[x].tree[q] ELSE cm[cm[p].cbase].tree[q]
      new(q) == IF DecidersAt = "decision" THEN AdoptTree(x, p)[q] ELSE cm[p].tree[q]
      D == {u \in Persons : \A q \in diff : AllowedC(u, q, old(q), new(q), Ro)}
  IN IF D = {} THEN Ro.adm ELSE D
Author(p) == R(cm[p].cbase).dev[cm[p].signer]
(* tree holds the change of p on a base BT (rule 7): each version it wrote,
   and nothing where it deleted a path that BT has not changed since. *)
Holds(p, BT, tree) ==
  \A q \in PDiff(p) : IF cm[p].tree[q] = {}
                     THEN tree[q] = {} \/ BT[q] # cm[cm[p].cbase].tree[q]
                     ELSE cm[p].tree[q] \subseteq tree[q]

(* The verdict of a commit with parents P, signer s, tree, decisions D,
   roster edits RE and Force F (04 §4.4 rules 1-4, and DecisionMerge). A
   decision is never changed: none names a proposal its base decided. A
   parent outside A must be a proposal whose change the tree holds, adopted
   by a decider or, with Force, an admin. *)
Judge(P, s, tree, D, RE, F) ==
  LET A == {p \in P : cm[p].acc}
      BS == IF BaseOf = "accepted" THEN HistSet(A) ELSE Hist(P)
      BT == Cat(Counting(BS))
      delta == {q \in CPaths : tree[q] # BT[q]}
      empty == delta = {} /\ D = {} /\ RE = {}
      AR == IF Rule3 = "every" \/ A = {} THEN A ELSE {CHOOSE p \in A : \A o \in A : p <= o}
      rule2 == IF empty THEN \E p \in A : s \in Present(R(p))
               ELSE \A p \in A : s \in Present(R(p))
      rule3 == \A p \in AR :
                 LET Ro == R(p)  u == Ro.dev[s] IN
                 /\ (\A q \in delta : AllowedC(u, q, BT[q], tree[q], Ro)) \/ (F /\ u \in Ro.adm)
                 /\ \A e \in D : /\ IsProposal(e.p)
                                 /\ ~\E y \in Counting(BS) : \E o \in cm[y].dec : o.p = e.p
                                 /\ \/ DecisionRule = "members"
                                    \/ IF e.v = "rejected" THEN u \in Deciders(e.p, p)
                                       ELSE u = Author(e.p)
                 /\ \A e \in RE : \/ u \in Ro.adm
                                  \/ e.k = "add" /\ e.p = u
                                  \/ e.k = "rem" /\ Ro.dev[e.d] = u
      rule4 == Change(RosterOf(BS), RE).ok
      ruleX == \/ ~DecisionMerge
               \/ \A p \in P \ A : /\ IsProposal(p)
                                   /\ \A a \in A : LET u == R(a).dev[s] IN
                                                   u \in Deciders(p, a) \/ (F /\ u \in R(a).adm)
                                   /\ Holds(p, BT, tree)
  IN [acc |-> A # {} /\ rule2 /\ rule3 /\ rule4 /\ ruleX, delta |-> delta]

New(P, s, kind, tree, D, RE, F, cb) ==
  LET j == Judge(P, s, tree, D, RE, F) IN
  [parents |-> P, signer |-> s, kind |-> kind, tree |-> tree, dec |-> D, redits |-> RE,
   force |-> F, acc |-> j.acc, delta |-> j.delta, cbase |-> cb, anc |-> Hist(P) \cup {Len(cm) + 1}]
CBase(p) == IF cm[p].acc THEN p ELSE cm[p].cbase

Init == /\ cm = << [parents |-> {}, signer |-> 0, kind |-> "genesis",
                    tree |-> [q \in CPaths |-> {}], dec |-> {}, redits |-> {}, force |-> FALSE,
                    acc |-> TRUE, delta |-> {}, cbase |-> 1, anc |-> {1}] >>
        /\ head = [d \in Honest |-> 1]
        /\ has = [d \in Honest |-> {1}]

Main(d) == Counting(HistSet({head[d]}))
Adopted(p, M) == \E y \in M : y # p /\ p \in cm[y].anc
Decided(p, M, v) == \E x \in M : \E e \in cm[x].dec : e.p = p /\ e.v = v
(* The state of a proposal on device d (04 §4.5), in this order. *)
State(p, d) ==
  LET M == Main(d) IN
  IF Adopted(p, M) THEN "accepted"
  ELSE IF Decided(p, M, "rejected") THEN "rejected"
  ELSE IF Decided(p, M, "withdrawn") THEN "withdrawn"
  ELSE IF \E o \in has[d] : IsProposal(o) /\ cm[o].parents = {p} THEN "superseded"
  ELSE "pending"

Room == Cardinality({x \in Ids : cm[x].kind \notin {"merge", "genesis"}}) < MaxCommits
Write(d, c) == /\ cm' = Append(cm, c)
               /\ head' = IF c.acc THEN [head EXCEPT ![d] = Len(cm) + 1] ELSE head
               /\ has' = [has EXCEPT ![d] = @ \cup {Len(cm) + 1}]
Overlay(T, E) == [q \in CPaths |-> IF \E e \in E : e[1] = q THEN (CHOOSE e \in E : e[1] = q)[2]
                                   ELSE T[q]]

(* Honest devices: pigeond's own actions. *)
Capture(d, q, del) ==
  LET x == head[d]  n == Len(cm) + 1 IN
  /\ "capture" \in Ops /\ Room /\ (del => cm[x].tree[q] # {})
  /\ Write(d, New({x}, d, "capture", Overlay(cm[x].tree, {<<q, IF del THEN {} ELSE {n}>>}),
                  {}, {}, FALSE, x))
Revise(d, p, q) ==
  /\ "revise" \in Ops /\ Room
  /\ p \in has[d] /\ IsProposal(p) /\ cm[p].signer = d /\ q \in PDiff(p) /\ State(p, d) = "pending"
  /\ Write(d, New({p}, d, "propose", Overlay(cm[p].tree, {<<q, {Len(cm) + 1}>>}), {}, {}, FALSE,
                  cm[p].cbase))
Pending(d, p) == /\ p \in has[d] /\ IsProposal(p) /\ State(p, d) = "pending"
                 /\ HistSet({p}) \subseteq cm[head[d]].anc   \* pigeond merges its base first
NeedsForce(u, x, p) == ~\A q \in ADiff(x, p) : AllowedC(u, q, cm[x].tree[q], AdoptTree(x, p)[q], R(x))
Adopt(d, p) ==
  LET x == head[d]
      c == New({x, p}, d, "adopt", AdoptTree(x, p), {}, {},
               ForceAdopt /\ NeedsForce(d, x, p) /\ d \in R(x).adm, x) IN
  /\ "adopt" \in Ops /\ Room /\ Pending(d, p) /\ d \in Deciders(p, x)
  /\ c.acc /\ Write(d, c)
Decide(d, p, v) ==
  LET x == head[d] IN
  /\ v \in Ops /\ Room /\ Pending(d, p)
  /\ IF v = "rejected" THEN d \in Deciders(p, x) ELSE d = Author(p)
  /\ Write(d, New({x}, d, v, cm[x].tree, {[p |-> p, v |-> v]}, {}, FALSE, x))
Transfer(d, o) ==
  LET x == head[d] IN
  /\ "transfer" \in Ops /\ Room /\ d \in R(x).adm /\ o # R(x).own
  /\ Write(d, New({x}, d, "roster", cm[x].tree, {}, {[k |-> "own", old |-> R(x).own, new |-> o]},
                  FALSE, x))
Remove(d) ==        \* the admin removes c's lost laptop and every other device of c it sees
  LET x == head[d]  Ro == R(x) IN
  /\ "remove" \in Ops /\ Room /\ d \in Ro.adm /\ 3 \in Present(Ro)
  /\ Write(d, New({x}, d, "roster", cm[x].tree, {},
                  {[k |-> "rem", d |-> e] : e \in {e \in Present(Ro) : Ro.dev[e] = 3}}, FALSE, x))
(* A merge brings an accepted change that main lacks; with "tip", a head
   signed by a device absent from main's roster is never merged (09 §9.7). *)
Brings(y, x) == \E z \in cm[y].anc : /\ cm[z].acc /\ z \notin cm[x].anc
                                     /\ (cm[z].delta # {} \/ cm[z].dec # {} \/ cm[z].redits # {})
Merge(d, y) ==
  LET x == head[d] IN
  /\ "merge" \in Ops
  /\ y \in has[d] /\ cm[y].acc /\ Brings(y, x)
  /\ Revocation = "tip" => cm[y].signer \in Present(R(x))
  /\ \/ /\ x \in cm[y].anc
        /\ head' = [head EXCEPT ![d] = y] /\ UNCHANGED <<cm, has>>
     \/ /\ x \notin cm[y].anc /\ d \in Present(R(x)) \cup Present(R(y))
        /\ Write(d, New({x, y}, d, "merge", TreeOf({x, y}), {}, {}, FALSE, x))
(* d fetches everything an honest peer e has, or a commit the attacker serves. *)
Fetch(d, e) ==
  /\ "fetch" \in Ops /\ e # d /\ ~(has[e] \subseteq has[d])
  /\ has' = [has EXCEPT ![d] = @ \cup has[e]] /\ UNCHANGED <<cm, head>>
Served(d, x) ==
  /\ "fetch" \in Ops /\ x \notin has[d] /\ cm[x].signer \in Attacker
  /\ has' = [has EXCEPT ![d] = @ \cup cm[x].anc] /\ UNCHANGED <<cm, head>>

(* The attacker: any parent, any edits, its own keys. *)
HRoom == Room /\ Cardinality({x \in Ids : cm[x].signer \in Attacker}) < MaxHostile
Edits(n) == {E \in SUBSET {<<q, v>> : q \in CPaths, v \in {{n}, {}}} :
               E # {} /\ \A e1, e2 \in E : e1[1] = e2[1] => e1 = e2}
HWrite(c) == cm' = Append(cm, c) /\ UNCHANGED <<head, has>>
HCommit(s, x) ==
  /\ "hcommit" \in Ops /\ HRoom
  /\ \E E \in Edits(Len(cm) + 1) :
       HWrite(New({x}, s, "hostile", Overlay(cm[x].tree, E), {}, {}, FALSE, CBase(x)))
HMerge(s, x, y) ==
  LET A == {p \in {x, y} : cm[p].acc}
      BT == Cat(Counting(IF BaseOf = "accepted" THEN HistSet(A) ELSE Hist({x, y}))) IN
  /\ "hmerge" \in Ops /\ HRoom /\ x < y
  /\ \E E \in {{}} \cup {{e} : e \in UNION Edits(Len(cm) + 1)} :
       HWrite(New({x, y}, s, "hostile", Overlay(BT, E), {}, {}, FALSE,
                  IF A = {} THEN 1 ELSE CHOOSE p \in A : TRUE))
HAdd(s, x) ==
  /\ "hadd" \in Ops /\ HRoom
  /\ HWrite(New({x}, s, "hostile", cm[x].tree, {}, {[k |-> "add", d |-> 4, p |-> 3]}, FALSE,
                CBase(x)))
HDecide(s, x, p) ==
  /\ "hdecide" \in Ops /\ HRoom /\ IsProposal(p)
  /\ \E v \in {"rejected", "withdrawn"} :
       HWrite(New({x}, s, "hostile", cm[x].tree, {[p |-> p, v |-> v]}, {}, FALSE, CBase(x)))

Next ==
  \/ \E d \in Honest :
       \/ \E q \in CPaths, del \in BOOLEAN : Capture(d, q, del)
       \/ \E p \in Ids, q \in CPaths : Revise(d, p, q)
       \/ \E p \in Ids : Adopt(d, p) \/ Decide(d, p, "rejected") \/ Decide(d, p, "withdrawn")
       \/ \E o \in Persons : Transfer(d, o)
       \/ Remove(d)
       \/ \E y \in Ids : Merge(d, y)
       \/ \E e \in Honest : Fetch(d, e)
       \/ \E x \in Ids : Served(d, x)
  \/ \E s \in Attacker, x \in Ids :
       \/ HCommit(s, x) \/ HAdd(s, x)
       \/ \E y \in Ids : HMerge(s, x, y)
       \/ \E p \in Ids : HDecide(s, x, p)

Spec == Init /\ [][Next]_vars
Live == Spec /\ \A d \in Honest : /\ WF_vars(\E e \in Honest : Fetch(d, e) \/ \E x \in Ids : Served(d, x))
                                  /\ WF_vars(\E y \in Ids : Merge(d, y))

(* ---- Safety ---- *)
L == Len(cm)
(* P4: every version in a device's catalogue entered through a commit whose
   change of that path its signer was allowed to make, under the roster of
   every accepted parent (or by an admin's Force). *)
AuthorisedAt(y, q) ==
  LET A == {p \in cm[y].parents : cm[p].acc}  old == TreeOf(A)[q] IN
  /\ A # {}
  /\ \A p \in A : LET Ro == R(p)  u == Ro.dev[cm[y].signer] IN
                  /\ cm[y].signer \in Present(Ro)
                  /\ AllowedC(u, q, old, cm[y].tree[q], Ro) \/ (cm[y].force /\ u \in Ro.adm)
AuthorisedContent ==
  \A d \in Honest : LET S == Main(d)  T == Cat(S) IN
    \A q \in CPaths : \A v \in T[q] :
      \E y \in S : v \in cm[y].tree[q] /\ q \in cm[y].delta /\ AuthorisedAt(y, q)
(* Every decision in a device's history was made by a decider (a rejection,
   judged on the decision's parents) or by the author (a withdrawal). *)
DecisionsByDeciders ==
  \A d \in Honest : \A x \in Main(d) : \A e \in cm[x].dec :
    \A p \in {p \in cm[x].parents : cm[p].acc} :
      LET u == R(p).dev[cm[x].signer] IN
      IF e.v = "rejected" THEN u \in Deciders(e.p, p) ELSE u = Author(e.p)
(* Each decider of a pending proposal can make a valid adoption of it. *)
MayAdopt(u, x, p) == \A q \in ADiff(x, p) : AllowedC(u, q, cm[x].tree[q], AdoptTree(x, p)[q], R(x))
                                           \/ (ForceAdopt /\ u \in R(x).adm)
DecidersCanAdopt ==
  \A d \in Honest : \A p \in Ids : Pending(d, p) =>
    \A u \in Deciders(p, head[d]) : MayAdopt(u, head[d], p)
(* A proposal is accepted only through a merge that holds its content. *)
AdoptionHoldsContent ==
  \A d \in Honest : \A p \in Ids : (IsProposal(p) /\ State(p, d) = "accepted") =>
    \E y \in Main(d), o \in Ids :
      /\ o \in cm[y].parents /\ IsProposal(o) /\ p \in cm[o].anc
      /\ \A q \in PDiff(o) : cm[o].tree[q] \subseteq cm[y].tree[q]
(* After the admin removes c's devices, nothing the removal had not seen from
   them reaches a catalogue except through an honest adoption. *)
RemovalsIn(S) == {r \in S : Removes(r, 3)}
Tainted3(x, S) == cm[x].signer = 3 /\ \E r \in RemovalsIn(S) : x \notin cm[r].anc
Tainted(x, S) ==
  \/ Tainted3(x, S)
  \/ /\ cm[x].signer = 4 /\ RemovalsIn(S) # {}
     /\ \A a \in {a \in S : \E e \in cm[a].redits : e.k = "add" /\ e.d = 4} : Tainted3(a, S)
RemovedSilenced ==
  \A d \in Honest : LET S == HistSet({head[d]})  T == TreeOf({head[d]}) IN
    \A q \in CPaths : \A v \in T[q] :
      Tainted(v, S) => \E y \in S : cm[y].kind = "adopt" /\ v \in cm[y].tree[q]
(* ... and the devices the removed one added unseen are not in the roster. *)
RemovalRevokesAdded ==
  \A d \in Honest : LET S == HistSet({head[d]}) IN
    \A r \in RemovalsIn(S) : R(head[d]).dev[4] # 0 =>
      \E a \in S \cap cm[r].anc : \E e \in cm[a].redits : e.k = "add" /\ e.d = 4
(* Laws on the newest commit: an honest tree is the catalogue of its
   history, and an honest merge is accepted. *)
TreeIsCatalogue == (cm[L].acc /\ cm[L].signer \in Honest) => cm[L].tree = TreeOf({L})
HonestMergesAccepted == (cm[L].kind = "merge") => cm[L].acc

(* For reading counterexamples (ALIAS View): the newest commit, heads, views. *)
View == [new |-> [id |-> L, kind |-> cm[L].kind, signer |-> cm[L].signer,
                  parents |-> cm[L].parents, tree |-> cm[L].tree, acc |-> cm[L].acc,
                  dec |-> cm[L].dec, redits |-> cm[L].redits, force |-> cm[L].force],
         head |-> head, has |-> has]

(* ---- Liveness (under Live's fairness) ---- *)
Known == UNION {has[d] : d \in Honest}
Converge == <>[](\A d \in Honest : TreeOf({head[d]}) = Cat(Counting(HistSet(Known))))
(* The same as a safety property. Every behaviour is finite: commits are
   bounded, and each fetch or merge grows what a device has or what its main
   holds. So under Live's fairness every behaviour ends in a quiet state, where
   no honest device can fetch or merge; there the trees must be the catalogue
   of everything known. *)
CanMerge(d, y) == /\ cm[y].acc /\ Brings(y, head[d])
                  /\ Revocation = "tip" => cm[y].signer \in Present(R(head[d]))
                  /\ head[d] \in cm[y].anc \/ d \in Present(R(head[d])) \cup Present(R(y))
Quiet == \A d \in Honest : /\ \A e \in Honest : has[e] \subseteq has[d]
                           /\ \A x \in Ids : cm[x].signer \in Attacker => x \in has[d]
                           /\ \A y \in has[d] : ~CanMerge(d, y)
ConvergedWhenQuiet ==
  Quiet => \A d \in Honest : TreeOf({head[d]}) = Cat(Counting(HistSet(Known)))
=============================================================================
