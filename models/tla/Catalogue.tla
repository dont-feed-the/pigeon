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
(* A contribution's renames (ADR 0072) move the writes of concurrent       *)
(* events that build on the version renamed (Target), and a pure rename    *)
(* yields to a version moved through it that saw what it moved (Yields).   *)
(* CatalogueTrace replays pigeon-core's catalogue::Engine against Record.  *)
(***************************************************************************)
EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS Owned, Root

None == "none"

VARIABLES ev, cs
vars == <<ev, cs>>

(* An event: [key, past (keys), w (writes [s, c, o]), res (resolved slots),
   fx (the slot a conflict event froze, else <<>>), rn (renames [x, y, p,
   seen]: old slot, new slot, whether the content stayed, and the events
   whose versions it moved)].  A judged commit: [id, acc, st (the keys of
   its state), rank].  The operators below take the events X and the
   judged commits C they look in. *)
Ev(X, k) == CHOOSE e \in X : e.key = k
Rank(C, o) == (CHOOSE c \in C : c.id = o).rank
Less(a, b) == \E i \in 1..4 : a[i] < b[i] /\ \A j \in 1..(i - 1) : a[j] = b[j]
Best(C, W) == CHOOSE w \in W : \A u \in W : ~Less(Rank(C, u.o), Rank(C, w.o))
WS(X, k) == {w.s : w \in Ev(X, k).w}

(* Renames (ADR 0072).  The renames of S, each with its event k. *)
Rn(X, S) == UNION {{[k |-> k, x |-> n.x, y |-> n.y, p |-> n.p, seen |-> n.seen] : n \in Ev(X, k).rn} : k \in S}
Earliest(C, N) == CHOOSE n \in N : \A m \in N : ~Less(Rank(C, m.k.id), Rank(C, n.k.id))
Conc(X, r, e) == r # e /\ r \notin Ev(X, e).past /\ e \notin Ev(X, r).past
(* Rename r builds on the version of x that e's write replaces: an event
   that e saw, or a rename its write followed, wrote x before r. *)
Shared(X, r, e, x, used) == \E w \in Ev(X, r).past : (w \in Ev(X, e).past \/ w \in used) /\ x \in WS(X, w)
Knew(X, e, x) == \E p \in Ev(X, e).past : \E n \in Ev(X, p).rn : n.x = x
Saw(X, e, y) == \E p \in Ev(X, e).past : y \in WS(X, p)
(* Where e's write at slot at goes, among the renames R: on through the
   earliest concurrent rename of at that builds on its version, while e saw
   no rename of at, nor writes or saw a write of the new slot. *)
RECURSIVE Follow(_, _, _, _, _, _, _)
Follow(X, C, R, e, at, used, seen) ==
  LET N == {n \in R : n.x = at /\ Conc(X, n.k, e) /\ Shared(X, n.k, e, at, used)}
      n == Earliest(C, N)
  IN IF Knew(X, e, at) \/ N = {} THEN [s |-> at, u |-> used]
     ELSE IF n.y \in seen \/ n.y \in WS(X, e) \/ Saw(X, e, n.y) THEN [s |-> at, u |-> used]
     ELSE Follow(X, C, R, e, n.y, used \cup {n.k}, seen \cup {n.y})
(* A rename's deletion of its old slot stays. *)
Target(X, C, R, e, x) == IF \E n \in Ev(X, e).rn : n.x = x THEN [s |-> x, u |-> {}]
                         ELSE Follow(X, C, R, e, x, {}, {x})
RECURSIVE Reach(_, _)
Reach(R, T) == LET U == T \cup {n.y : n \in {n \in R : n.x \in T}} IN IF U = T THEN T ELSE Reach(R, U)

(* The effective writes of S's events, renames followed: [s, c, o, from (the
   slot written), u (the renames followed)]. *)
EffOf(X, C, S, R) == [k \in S |-> {LET t == Target(X, C, R, k, w.s) IN
                                     [s |-> t.s, c |-> w.c, o |-> w.o, from |-> w.s, u |-> t.u] :
                                   w \in Ev(X, k).w}]
EWS(Eff, k) == {w.s : w \in Eff[k]}
EAt(Eff, k, x) == CHOOSE w \in Eff[k] : w.s = x
(* w saw e's write of slot s and wrote where, in w's view, it was. *)
Over(X, C, R, w, e, s) ==
  e \in Ev(X, w).past /\ Target(X, C, {n \in R : n.k \in Ev(X, w).past}, e, s).s \in WS(X, w)
(* Step 2: the writers of x that nothing replaced, among ws. *)
HeadsOf(X, C, R, Eff, S, ws, x) ==
  {e \in ws : LET s == EAt(Eff, e, x).from IN
              ~\E w \in {k \in S : WS(X, k) \cap Reach(R, {s}) # {}} : w # e /\ Over(X, C, R, w, e, s)}
(* A pure rename r's write of x yields to a version that came there through
   r and saw every version r moved. *)
Yields(X, Eff, r, e, x) ==
  /\ r \in EAt(Eff, e, x).u /\ EAt(Eff, e, x).c # None
  /\ \E n \in Ev(X, r).rn : n.y = x /\ n.p /\ n.seen \subseteq Ev(X, e).past

(* The catalogue of the state S (steps 2 to 5): its heads h and yielded
   renames g by slot, effective writes, view, unsettled slots, the versions
   each unsettled slot shows, and open conflicts (05 §5.3). *)
Present(X, C, S) ==
  LET R == Rn(X, S)
      Eff == EffOf(X, C, S, R)
      Sl == UNION {EWS(Eff, k) : k \in S}
      Hd == [x \in Sl |->
               LET ws == {k \in S : x \in EWS(Eff, k)}
                   h0 == HeadsOf(X, C, R, Eff, S, ws, x)
                   gone == {r \in h0 : \E e \in h0 : Yields(X, Eff, r, e, x)}
               IN [h |-> IF gone = {} THEN h0 ELSE HeadsOf(X, C, R, Eff, S, ws \ gone, x), g |-> gone]]
      (* Step 3: the heads' values; a deletion is none. *)
      Cd(x) == {w \in {EAt(Eff, k, x) : k \in Hd[x].h} : w.c # None}
      (* Step 4: each distinct content, with its earliest head. *)
      Rp(x) == LET W == Cd(x) IN {Best(C, {w \in W : w.c = v}) : v \in {u.c : u \in W}}
      Lv == {x \in Sl : Cd(x) # {}}
      FS(k) == (CHOOSE w \in Eff[k] : w.from = Ev(X, k).fx).s
      (* Whether a conflict event of S froze x (where renames took it) with
         o's version at a copy. *)
      Cp(x, o) == \E k \in S : Ev(X, k).fx # <<>> /\ FS(k) = x
                               /\ \E u \in Ev(X, k).w : u.s # Ev(X, k).fx /\ u.o = o
      (* Step 5: the first version keeps the slot, every other one its copy,
         unless a conflict event put it there: the copy shows it already, or
         what a person who saw it saved there. *)
      Sh(x) == LET Rx == Rp(x)
                   f == Best(C, Rx)
               IN {[s |-> x, c |-> f.c, o |-> f.o]} \cup
                  {[s |-> Append(x, w.o), c |-> w.c, o |-> w.o] : w \in {u \in Rx \ {f} : ~Cp(x, u.o)}}
      Un == {x \in Lv : Cardinality(Sh(x)) > 1}
      (* 05 §5.3: a conflict is open while unsettled, or frozen and neither
         written nor resolved by a contribution that saw it; a pure rename's
         write that yielded does not count. *)
      Kept(k) == {y \in EWS(Eff, k) : ~(y \in DOMAIN Hd /\ k \in Hd[y].g)}
      Closed(f) == \E k \in S : k.k = "c" /\ f \in Ev(X, k).past
                                /\ (FS(f) \in Ev(X, k).res \/ Kept(k) \cap Kept(f) # {})
  IN [heads |-> Hd, eff |-> Eff, view |-> UNION {Sh(x) : x \in Lv}, unsettled |-> Un,
      shown |-> [x \in Un |-> Sh(x)],
      open |-> Un \cup {FS(f) : f \in {f \in S : f.k = "f" /\ ~Closed(f)}}]

CKey(id) == [k |-> "c", id |-> id, x |-> <<>>, h |-> {}, w |-> {}]
(* Step 6: the conflict event that keeps x's versions where P, the
   catalogue of S, shows them.  It saw the heads and the renames that moved
   them to x. *)
Freeze(P, X, x) ==
  LET H0 == P.heads[x].h
      H == H0 \cup UNION {EAt(P.eff, h, x).u : h \in H0}
      V == P.shown[x]
  IN [key |-> [k |-> "f", id |-> "", x |-> x, h |-> H, w |-> {[s |-> v.s, c |-> v.c] : v \in V}],
      past |-> UNION {{h} \cup Ev(X, h).past : h \in H}, w |-> V, res |-> {}, fx |-> x, rn |-> {}]

(* The versions a rename of x moves: those of the heads of x that hold one,
   a pure rename standing for the versions it moved in turn. *)
Moved(P, X, x) ==
  IF x \notin DOMAIN P.heads THEN {}
  ELSE UNION {LET pr == {n \in Ev(X, h).rn : n.y = x /\ n.p} IN
                IF pr = {} THEN {h} ELSE (CHOOSE n \in pr : TRUE).seen :
              h \in {h \in P.heads[x].h : EAt(P.eff, h, x).c # None}}

May(x, added, alice) == IF x[1] \in Owned THEN alice
                        ELSE IF x[1] \in Root THEN added \/ alice ELSE TRUE

ValueIn(T, x) == IF \E t \in T : t.s = x THEN (CHOOSE t \in T : t.s = x).c ELSE None

(* The verdict on commit r = [id, parents, rank, alice, tree, res, rn], the
   events it adds, its state, and the judged commits with it.  A rename
   counts when it deletes a slot of the base and adds one the base lacks. *)
Step(r) ==
  LET A == {c \in cs : c.id \in r.parents /\ c.acc}
      B == UNION {c.st : c \in A}
      P == Present(ev, cs, B)
      bt == {[s |-> v.s, c |-> v.c] : v \in P.view}
      bs == {t.s : t \in bt}
      ts == {t.s : t \in r.tree}
      D == {x \in bs \cup ts : ValueIn(bt, x) # ValueIn(r.tree, x)}
      R == {x \in r.res : x \in bs /\ May(x, FALSE, r.alice)}
      F == {Freeze(P, ev, x) : x \in P.unsettled}
      N == {[x |-> n.x, y |-> n.y, p |-> ValueIn(bt, n.x) = ValueIn(r.tree, n.y), seen |-> Moved(P, ev, n.x)] :
            n \in {n \in r.rn : n.x \in bs /\ n.y \in ts /\ n.x \notin ts /\ n.y \notin bs}}
      E == [key |-> CKey(r.id), past |-> B \cup {f.key : f \in F},
            w |-> {[s |-> x, c |-> ValueIn(r.tree, x), o |-> r.id] : x \in D}, res |-> R, fx |-> <<>>, rn |-> N]
      G == [key |-> CKey(r.id), past |-> {}, w |-> {[s |-> t.s, c |-> t.c, o |-> r.id] : t \in r.tree},
            res |-> {}, fx |-> <<>>, rn |-> {}]
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
