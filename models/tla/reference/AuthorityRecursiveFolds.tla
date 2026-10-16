---------------------- MODULE AuthorityRecursiveFolds -----------------------
(* The pre-TLAPS Authority fold equations from c72e0ac. The equivalence check
   substitutes only these operators into the current model, so actions and
   safety predicates are shared. Keep this reference independent of rewrites. *)
EXTENDS Naturals, FiniteSets, Sequences
CONSTANTS Revocation, Genesis, Apply1(_, _), Change(_, _), Present(_), Unseen(_, _)
VARIABLE cm

(* BEGIN ApplyAll *)
RECURSIVE ApplyAll(_, _)
ApplyAll(r, E) == IF E = {} THEN r
                  ELSE LET e == CHOOSE x \in E : TRUE
                           n == Apply1(r.s, e)
                       IN ApplyAll([ok |-> r.ok /\ n.ok, s |-> n.s], E \ {e})
(* END ApplyAll *)

(* BEGIN FoldUpTo *)
RECURSIVE FoldUpTo(_, _, _, _)
FoldUpTo(S, C, Ro, i) ==
  IF i > Len(cm) THEN Ro
  ELSE FoldUpTo(S, C, IF i \in C THEN Change(Ro, cm[i].redits).s
                      ELSE IF i \in S THEN Change(Ro, {e \in cm[i].redits : e.k = "rem"}).s
                      ELSE Ro, i + 1)
(* END FoldUpTo *)

Fold(S, C) == FoldUpTo(S, C, Genesis, 1)

(* BEGIN CountUpTo *)
RECURSIVE CountUpTo(_, _, _)
CountUpTo(S, C, i) ==
  IF i > Len(cm) THEN C
  ELSE CountUpTo(S, IF /\ i \in S
                       /\ \/ i = 1
                          \/ /\ ~Unseen(i, S)
                             /\ \/ Revocation = "contentonly"
                                \/ cm[i].signer \in Present(Fold(S \cap (cm[i].anc \ {i}), C \cap cm[i].anc))
                    THEN C \cup {i} ELSE C, i + 1)
(* END CountUpTo *)
=============================================================================
