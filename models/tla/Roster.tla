------------------------------- MODULE Roster -------------------------------
(* Concurrent roster changes with disjoint admin sets (issue #6, spec 04 §4.7).
   Replicas are devices that commit offline on their own branch and merge any
   commit they have fetched; commits form a DAG and carry the roster written in
   their tree (Roster_fold, Roster_clock and Roster_fold3 check the design;
   the other Roster_*.cfg are negative tests).
   A change is authorised once, at commit time, under the roster of its parent.
   Persons 1..NP (1 = a and 2 = b are admins; 3 = c is not), device i belongs to
   person i for i <= NP, devices NP+1..ND are fresh keys to add. Mode:
     "fold"    : the roster of a commit is the fold of the changes of its
                 non-merge ancestors in canonical order (Kahn's algorithm, the
                 smallest (time, device, id) first); a change applies atomically
                 or is skipped, and skipped changes are reported (the design);
     "rebase"  : a merge replays the other side's changes onto its own roster
                 (negative test: merge order matters);
     "table"   : roster.toml merges per TOML table like a file and a clean
                 merge may be signed by anyone (spec 04 §4.4, 05 §5.2 before the
                 fix; negative test);
     "adr0009" : the same merge, but a merge of concurrent roster changes needs
                 a signer who is admin under both parents' rosters (ADR 0009;
                 negative test).
   SignerRule: "every" parent's roster must hold a merge's signer (rule 2 of
   04 §4.4), or "some" parent's. Removals: "atomic" skips a change's device
   removals with the rest of it; "always" applies them even when the rest of
   the change is skipped. Allowed: the kinds of change devices make. *)
EXTENDS Naturals, FiniteSets, Sequences
CONSTANTS NP, ND, NR, MaxCommits, Times, Mode, SignerRule, Removals, Allowed
Persons == 1..NP
Devices == 1..ND
Replicas == 1..NR
Fresh == (NP+1)..ND

VARIABLES cm, head, key
vars == <<cm, head, key>>

(* A roster: members, admins, device -> person (0 = absent), devices ever removed. *)
Genesis == [mem |-> Persons, adm |-> {1, 2},
            dev |-> [d \in Devices |-> IF d <= NP THEN d ELSE 0], gone |-> {}]
Valid(S) == /\ S.adm \cap S.mem # {} /\ S.adm \subseteq S.mem
            /\ \A d \in Devices : S.dev[d] # 0 => S.dev[d] \in S.mem
Present(S) == {d \in Devices : S.dev[d] # 0}

(* Edits. "admin" is a compare-and-set of a boolean, so it conflicts only when
   the person is gone; "add" applies if absent or equal; "rem" always applies;
   "remp" removes a person and every device of theirs present at that point. *)
Apply1(S, e) ==
  CASE e.k = "admin" -> IF e.p \notin S.mem THEN [ok |-> FALSE, s |-> S]
                        ELSE [ok |-> TRUE, s |-> [S EXCEPT !.adm =
                                IF e.new THEN @ \cup {e.p} ELSE @ \ {e.p}]]
    [] e.k = "add"   -> IF S.dev[e.d] = e.p THEN [ok |-> TRUE, s |-> S]
                        ELSE IF S.dev[e.d] = 0 /\ e.d \notin S.gone
                             THEN [ok |-> TRUE, s |-> [S EXCEPT !.dev[e.d] = e.p]]
                             ELSE [ok |-> FALSE, s |-> S]
    [] e.k = "rem"   -> [ok |-> TRUE, s |-> [S EXCEPT !.dev[e.d] = 0, !.gone = @ \cup {e.d}]]
    [] e.k = "remp"  -> LET ds == {d \in Devices : S.dev[d] = e.p} IN
                        [ok |-> TRUE, s |-> [mem |-> S.mem \ {e.p}, adm |-> S.adm \ {e.p},
                           dev |-> [d \in Devices |-> IF d \in ds THEN 0 ELSE S.dev[d]],
                           gone |-> S.gone \cup ds]]

RECURSIVE ApplyAll(_, _)
ApplyAll(r, E) == IF E = {} THEN r
                  ELSE LET e == CHOOSE x \in E : TRUE
                           n == Apply1(r.s, e)
                       IN ApplyAll([ok |-> r.ok /\ n.ok, s |-> n.s], E \ {e})
(* A whole change: applied atomically if every edit applies and the result is
   valid. Removing a device never makes a roster invalid, so with Removals =
   "always" a skipped change still removes its devices. *)
Change(S, E) ==
  LET r == ApplyAll([ok |-> TRUE, s |-> S], E)
      rems == {e \in E : e.k = "rem"}
      kept == IF Removals = "always" THEN ApplyAll([ok |-> TRUE, s |-> S], rems).s ELSE S
  IN [ok |-> r.ok /\ Valid(r.s), s |-> IF r.ok /\ Valid(r.s) THEN r.s ELSE kept]

(* The commit graph. cm[i] = [parents, signer, time, op, edits, merge, ros]. *)
Ids == 1..Len(cm)
RECURSIVE Anc(_)
Anc(c) == {c} \cup UNION {Anc(p) : p \in cm[c].parents}
NonMergeAnc(c) == {x \in Anc(c) : x # 1 /\ ~cm[x].merge}
Key(x) == <<cm[x].time, cm[x].signer, x>>
Before(u, v) == \/ u[1] < v[1]
                \/ (u[1] = v[1] /\ u[2] < v[2])
                \/ (u[1] = v[1] /\ u[2] = v[2] /\ u[3] < v[3])
(* Kahn's algorithm: among the commits none of whose remaining ancestors is
   left, the smallest key goes first; skipped changes are collected. *)
RECURSIVE FoldR(_, _)
FoldR(rest, acc) ==
  IF rest = {} THEN acc
  ELSE LET ready == {x \in rest : \A y \in rest \ {x} : y \notin Anc(x)}
           m == CHOOSE x \in ready : \A z \in ready \ {x} : Before(Key(x), Key(z))
           r == Change(acc.s, cm[m].edits)
       IN FoldR(rest \ {m}, [s |-> r.s, skipped |-> IF r.ok THEN acc.skipped
                                                   ELSE acc.skipped \cup {m}])
FoldFrom(S, set) == FoldR(set, [s |-> S, skipped |-> {}])
Fold(set) == FoldFrom(Genesis, set).s
Skipped(c) == FoldFrom(Genesis, NonMergeAnc(c)).skipped

(* Table merge of roster.toml (modes "table" and "adr0009"), three-way per
   person table and per device table; a conflict keeps the merging side. *)
Base(x, y) == LET common == Anc(x) \cap Anc(y) IN CHOOSE b \in common : \A o \in common : o <= b
Pick(b, x, y) == IF x = b THEN y ELSE x
TableMerge(B, X, Y) ==
  LET pt(S, p) == <<p \in S.mem, p \in S.adm>>
      pm(p) == Pick(pt(B, p), pt(X, p), pt(Y, p))
  IN [mem |-> {p \in Persons : pm(p)[1]}, adm |-> {p \in Persons : pm(p)[2]},
      dev |-> [d \in Devices |-> Pick(B.dev[d], X.dev[d], Y.dev[d])],
      gone |-> X.gone \cup Y.gone]

Init == /\ cm = << [parents |-> {}, signer |-> 0, time |-> 0, op |-> "genesis",
                    edits |-> {}, merge |-> FALSE, ros |-> Genesis] >>
        /\ head = [r \in Replicas |-> 1]
        /\ key = [r \in Replicas |-> r]          \* the device key a replica signs with

(* The changes person ps may sign under roster S: [op, edits]. A fresh key
   is added for one person only, though two inviters may add it concurrently. *)
FreshFor(n, q) == \A i \in Ids : \A e \in cm[i].edits : e.k = "add" /\ e.d = n => e.p = q
CanAdd(S, n, q) == S.dev[n] = 0 /\ n \notin S.gone /\ FreshFor(n, q)
Ops(S, ps) ==
  LET adm == ps \in S.adm IN
     {[op |-> "retire", E |-> {[k |-> "admin", p |-> ps, new |-> FALSE]}] :
        x \in (IF adm THEN {1} ELSE {})}
  \cup {[op |-> "demote", E |-> {[k |-> "admin", p |-> q, new |-> FALSE]}] :
        q \in (IF adm THEN S.adm \ {ps} ELSE {})}
  \cup {[op |-> "promote", E |-> {[k |-> "admin", p |-> q, new |-> TRUE]}] :
        q \in (IF adm THEN S.mem \ S.adm ELSE {})}
  \cup {[op |-> "add", E |-> {[k |-> "add", d |-> nq[1], p |-> nq[2]]}] :
        nq \in {nq \in Fresh \X S.mem : (adm \/ nq[2] = ps) /\ CanAdd(S, nq[1], nq[2])}}
  \cup {[op |-> "remove", E |-> {[k |-> "rem", d |-> d]}] :
        d \in {d \in Present(S) : adm \/ S.dev[d] = ps}}
  \cup {[op |-> "rotate", E |-> {[k |-> "rem", d |-> dn[1]],
                                 [k |-> "add", d |-> dn[2], p |-> S.dev[dn[1]]]}] :
        dn \in {dn \in Present(S) \X Fresh : (adm \/ S.dev[dn[1]] = ps)
                                             /\ CanAdd(S, dn[2], S.dev[dn[1]])}}
  \cup {[op |-> "remove-person", E |-> {[k |-> "remp", p |-> q]}] :
        q \in (IF adm THEN S.mem \ {ps} ELSE {})}

(* Replica r signs a change on its own head; rotating its own key moves it. *)
Commit(r, t) ==
  LET x == head[r]  S == cm[x].ros  ps == S.dev[key[r]] IN
  /\ Len(cm) < MaxCommits /\ ps # 0
  /\ \E o \in {o \in Ops(S, ps) : o.op \in Allowed} :
       LET n == Change(S, o.E) IN
       /\ n.ok
       /\ cm' = Append(cm, [parents |-> {x}, signer |-> key[r], time |-> t, op |-> o.op,
                            edits |-> o.E, merge |-> FALSE, ros |-> n.s])
       /\ head' = [head EXCEPT ![r] = Len(cm) + 1]
       /\ key' = IF o.op = "rotate" /\ \E e \in o.E : e.k = "rem" /\ e.d = key[r]
                 THEN [key EXCEPT ![r] = CHOOSE d \in Devices :
                         \E e \in o.E : e.k = "add" /\ e.d = d]
                 ELSE key

Concurrent(x, y) == x \notin Anc(y) /\ y \notin Anc(x)
SignerOK(d, x, y) ==
  LET inx == cm[x].ros.dev[d] # 0  iny == cm[y].ros.dev[d] # 0
      both == cm[x].ros # cm[Base(x, y)].ros /\ cm[y].ros # cm[Base(x, y)].ros
      ps == IF inx THEN cm[x].ros.dev[d] ELSE cm[y].ros.dev[d]
  IN /\ IF SignerRule = "every" THEN inx /\ iny ELSE inx \/ iny
     /\ (Mode = "adr0009" /\ both) => ps \in cm[x].ros.adm /\ ps \in cm[y].ros.adm
(* The roster a merge writes; parents x (the merging side) and y. *)
MergeRoster(x, y) ==
  CASE Mode = "fold"   -> Fold(NonMergeAnc(x) \cup NonMergeAnc(y))
    [] Mode = "rebase" -> FoldFrom(cm[x].ros, NonMergeAnc(y) \ NonMergeAnc(x)).s
    [] OTHER           -> TableMerge(cm[Base(x, y)].ros, cm[x].ros, cm[y].ros)

(* Replica r merges commit y, any commit it has fetched: another replica's
   head now or earlier. *)
Merge(r, y) ==
  LET x == head[r] IN
  /\ \/ /\ x \in Anc(y) /\ x # y                  \* fast-forward: no commit
        /\ head' = [head EXCEPT ![r] = y] /\ UNCHANGED cm
     \/ /\ Concurrent(x, y) /\ Len(cm) < MaxCommits /\ SignerOK(key[r], x, y)
        /\ cm' = Append(cm, [parents |-> {x, y}, signer |-> key[r], time |-> 0, op |-> "merge",
                             edits |-> {}, merge |-> TRUE, ros |-> MergeRoster(x, y)])
        /\ head' = [head EXCEPT ![r] = Len(cm) + 1]
  /\ UNCHANGED key

Next == \/ \E r \in Replicas, t \in Times : Commit(r, t)
        \/ \E r \in Replicas, y \in Ids : Merge(r, y)

Spec == Init /\ [][Next]_vars

(* The commit graph only grows and commits never change, so each invariant
   about commits is checked on the newest one, L: every commit was the newest
   in some earlier state. *)
L == Len(cm)
(* Every commit's roster keeps an admin. *)
AdminRemains == cm[L].ros.adm \cap cm[L].ros.mem # {}
(* A device removed by an accepted change is absent from every descendant. *)
RemovalWins == \A x \in Anc(L) : \A e \in cm[x].edits : e.k = "rem" => cm[L].ros.dev[e.d] = 0
(* The same for a person and their devices. *)
PersonRemovalWins == \A x \in Anc(L) : \A e \in cm[x].edits :
                       e.k = "remp" => e.p \notin cm[L].ros.mem
(* Each tree's roster is the fold of its history, so commits with the same
   non-merge ancestors have the same roster, whatever the merge order. *)
TreeIsFold == cm[L].ros = Fold(NonMergeAnc(L))
Agreement == \A x \in Ids : NonMergeAnc(x) = NonMergeAnc(L) => cm[x].ros = cm[L].ros
(* A change is skipped only for a concurrent change: never in a linear history. *)
SkipsNeedConcurrency == \A x \in Skipped(L) : \E y \in NonMergeAnc(L) : Concurrent(x, y)
(* Any two concurrent heads can be merged by some device of the group, unless
   neither side has a device left (then only recovery helps, ADR 0047). *)
MergeAlwaysPossible ==
  \A r, s \in Replicas :
    LET x == head[r]  y == head[s] IN
    (Concurrent(x, y) /\ Present(cm[x].ros) \cup Present(cm[y].ros) # {})
      => \E d \in Devices : SignerOK(d, x, y)
(* Witness: a merge of a's and b's concurrent self-retirements (expected violated). *)
Retires(x, p) == cm[x].op = "retire" /\ \E e \in cm[x].edits : e.p = p
NoConcurrentRetirement == ~\E m \in Ids : \E x, y \in NonMergeAnc(m) :
                            Retires(x, 1) /\ Retires(y, 2) /\ Concurrent(x, y)
=============================================================================
