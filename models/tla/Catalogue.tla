------------------------------ MODULE Catalogue ------------------------------
(***************************************************************************)
(* The deterministic merge of the catalogue (spec 05 §5.2), over abstract  *)
(* slots: what models/catalogue/catalogue.py (the oracle) computes, less   *)
(* its names.  A slot is a sequence of strings: <<p>> for a path p, and    *)
(* Append(x, o) for the conflict copy of slot x that commit o represents,  *)
(* so names never collide and folding is out of scope.  An event is a      *)
(* contribution, or a conflict event that a contribution froze; it writes  *)
(* slots, each with a content (None for a deletion) and the commit it      *)
(* represents (step 4).  Events are named by keys, so that one conflict    *)
(* event frozen twice is one event.  Record judges one commit: its base is *)
(* the catalogue of its accepted parents' states; its change is the slots  *)
(* whose content differs from its tree; it is clean when nothing differs,  *)
(* a proposal when its person may not change a slot (Owned slots are       *)
(* Alice's, Root slots add-only with Alice as steward), else a             *)
(* contribution, which freezes every unsettled slot of its base (step 6).  *)
(* CatalogueTrace replays pigeon-core's catalogue::Engine against Record.  *)
(***************************************************************************)
EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS Owned, Root

None == "none"

VARIABLES ev, cs
vars == <<ev, cs>>

(* An event: [key, past (keys), w (writes [s, c, o]), res (resolved slots),
   fx (the slot a conflict event froze, else <<>>)].  A judged commit:
   [id, acc, st (the keys of its state), rank].  The operators below take
   the events X and the judged commits C they look in. *)
Ev(X, k) == CHOOSE e \in X : e.key = k
Rank(C, o) == (CHOOSE c \in C : c.id = o).rank
Less(a, b) == \E i \in 1..4 : a[i] < b[i] /\ \A j \in 1..(i - 1) : a[j] = b[j]

WS(X, k) == {w.s : w \in Ev(X, k).w}
At(X, k, x) == CHOOSE w \in Ev(X, k).w : w.s = x
Writers(X, S, x) == {k \in S : x \in WS(X, k)}
(* Step 2: the writers of x that no other writer of x has seen. *)
Heads(X, S, x) == {k \in Writers(X, S, x) : \A f \in Writers(X, S, x) : k \notin Ev(X, f).past}
Slots(X, S) == UNION {WS(X, k) : k \in S}
(* Step 3: the heads' values; a deletion is none. *)
Cands(X, S, x) == {w \in {At(X, k, x) : k \in Heads(X, S, x)} : w.c # None}
Best(C, W) == CHOOSE w \in W : \A u \in W : ~Less(Rank(C, u.o), Rank(C, w.o))
(* Step 4: each distinct content, with its earliest head. *)
Reps(X, C, S, x) == LET W == Cands(X, S, x) IN {Best(C, {w \in W : w.c = v}) : v \in {u.c : u \in W}}
Live(X, S) == {x \in Slots(X, S) : Cands(X, S, x) # {}}
(* Whether a conflict event of S froze x with o's version at its copy. *)
Copied(X, S, x, o) == \E k \in S : Ev(X, k).fx = x /\ \E u \in Ev(X, k).w : u.s = Append(x, o) /\ u.o = o
(* Step 5: the first version keeps the slot, every other one its copy,
   unless a conflict event put it there: the copy shows it already, or what
   a person who saw it saved there. *)
Shown(X, C, S, x) ==
  LET R == Reps(X, C, S, x)
      f == Best(C, R)
  IN {[s |-> x, c |-> f.c, o |-> f.o]} \cup
     {[s |-> Append(x, w.o), c |-> w.c, o |-> w.o] : w \in {u \in R \ {f} : ~Copied(X, S, x, u.o)}}
View(X, C, S) == UNION {Shown(X, C, S, x) : x \in Live(X, S)}
Tree(X, C, S) == {[s |-> v.s, c |-> v.c] : v \in View(X, C, S)}
Unsettled(X, C, S) == {x \in Live(X, S) : Cardinality(Shown(X, C, S, x)) > 1}
(* 05 §5.3: a conflict is open while unsettled, or frozen and neither
   written nor resolved by a contribution that saw it. *)
Closed(X, S, f) == \E k \in S : k.k = "c" /\ f \in Ev(X, k).past
                               /\ (Ev(X, f).fx \in Ev(X, k).res \/ WS(X, k) \cap WS(X, f) # {})
Open(X, C, S) == Unsettled(X, C, S) \cup {Ev(X, f).fx : f \in {f \in S : f.k = "f" /\ ~Closed(X, S, f)}}

CKey(id) == [k |-> "c", id |-> id, x |-> <<>>, h |-> {}, w |-> {}]
(* Step 6: the conflict event that keeps x's versions where S shows them. *)
Freeze(X, C, S, x) ==
  LET H == Heads(X, S, x)
      V == Shown(X, C, S, x)
  IN [key |-> [k |-> "f", id |-> "", x |-> x, h |-> H, w |-> {[s |-> v.s, c |-> v.c] : v \in V}],
      past |-> UNION {{h} \cup Ev(X, h).past : h \in H}, w |-> V, res |-> {}, fx |-> x]

May(x, added, alice) == IF x[1] \in Owned THEN alice
                        ELSE IF x[1] \in Root THEN added \/ alice ELSE TRUE

ValueIn(T, x) == IF \E t \in T : t.s = x THEN (CHOOSE t \in T : t.s = x).c ELSE None

(* The verdict on commit r = [id, parents, rank, alice, tree, res], the
   events it adds, its state, and the judged commits with it. *)
Step(r) ==
  LET A == {c \in cs : c.id \in r.parents /\ c.acc}
      B == UNION {c.st : c \in A}
      bt == Tree(ev, cs, B)
      D == {x \in {t.s : t \in bt \cup r.tree} : ValueIn(bt, x) # ValueIn(r.tree, x)}
      R == {x \in r.res : x \in {t.s : t \in bt} /\ May(x, FALSE, r.alice)}
      F == {Freeze(ev, cs, B, x) : x \in Unsettled(ev, cs, B)}
      E == [key |-> CKey(r.id), past |-> B \cup {f.key : f \in F},
            w |-> {[s |-> x, c |-> ValueIn(r.tree, x), o |-> r.id] : x \in D}, res |-> R, fx |-> <<>>]
      G == [key |-> CKey(r.id), past |-> {}, w |-> {[s |-> t.s, c |-> t.c, o |-> r.id] : t \in r.tree},
            res |-> {}, fx |-> <<>>]
      v == CASE r.parents = {} -> [kind |-> "contribution", new |-> {G}, st |-> {G.key}]
             [] A = {} -> [kind |-> "refused", new |-> {}, st |-> {}]
             [] D = {} /\ R = {} -> [kind |-> "clean", new |-> {}, st |-> B]
             [] \E x \in D : ~May(x, ValueIn(bt, x) = None, r.alice) ->
                  [kind |-> "proposal", new |-> {}, st |-> B]
             [] OTHER -> [kind |-> "contribution",
                          new |-> {f \in F : ~\E e \in ev : e.key = f.key} \cup {E},
                          st |-> B \cup {f.key : f \in F} \cup {E.key}]
  IN [kind |-> v.kind, new |-> v.new, st |-> v.st, X |-> ev \cup v.new,
      C |-> cs \cup {[id |-> r.id, acc |-> v.kind \in {"clean", "contribution"}, st |-> v.st,
                      rank |-> r.rank]}]

Record(r) == LET s == Step(r) IN ev' = s.X /\ cs' = s.C

Init == ev = {} /\ cs = {}
=============================================================================
