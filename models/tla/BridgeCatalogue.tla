------------------------- MODULE BridgeCatalogue -------------------------
(* The Nextcloud bridge's read transaction. A native peer supplies immutable
   revision-pinned pages. A restart discards every old page; a post-save read
   stays pinned. Publish returns exactly the accumulated metadata, once, only
   after a terminal page. RPC failure cannot publish a partial catalogue.
   Paths are order-preserving identities; etags are opaque content identities.
   Unicode portability, hashing, daemon durability and HTTP are outside this
   model. WeakRevision is a deliberate regression checked by the bad config. *)
EXTENDS Naturals, Sequences, FiniteSets
CONSTANTS Required, WeakRevision
NoCursor == [present |-> FALSE, revision |-> 0, after |-> 0]
NoPage == [revision |-> 0, files |-> <<>>, next |-> NoCursor]
InitState == [pc |-> "ready", revision |-> Required, files |-> <<>>,
              cursor |-> [present |-> Required # 0, revision |-> Required, after |-> 0], attempt |-> 1,
              page |-> NoPage, failure |-> 0, seen |-> {}]
Last(p) == IF Len(p.files) = 0 THEN 0 ELSE p.files[Len(p.files)].path
Valid(s) ==
  LET p == s.page IN
  /\ p.revision > 0 /\ Len(p.files) <= 1000
  /\ (WeakRevision \/ s.revision = 0 \/ p.revision = s.revision)
  /\ \A i \in 1..Len(p.files) : p.files[i].path > s.cursor.after
  /\ \A i \in 1..(Len(p.files)-1) : p.files[i].path < p.files[i+1].path
  /\ (p.next = NoCursor \/ (Len(p.files) > 0 /\ p.next.revision = p.revision
                            /\ p.next.after = Last(p)))
Step(s, e, t) ==
  CASE e.action = "Request" ->
    /\ s.pc = "ready" /\ e.cursor = s.cursor
    /\ t = [s EXCEPT !.pc = "waiting"]
  [] e.action = "Reply" ->
    /\ s.pc = "waiting"
    /\ t = IF e.status = 0 THEN [s EXCEPT !.pc = "page", !.page = e.page]
            ELSE [s EXCEPT !.pc = "failure", !.failure = e.status]
  [] e.action = "Continue" ->
    /\ s.pc = "page" /\ Valid(s) /\ s.page.next # NoCursor
    /\ e.cursor = s.page.next /\ e.files = s.files \o s.page.files
    /\ t = [s EXCEPT !.pc = "ready", !.revision = s.page.revision,
                     !.cursor = e.cursor, !.files = e.files,
                     !.seen = @ \cup {s.page.revision}]
  [] e.action = "Publish" ->
    /\ s.pc = "page" /\ Valid(s) /\ s.page.next = NoCursor
    /\ e.revision = s.page.revision /\ e.files = s.files \o s.page.files
    /\ t = [s EXCEPT !.pc = "done", !.revision = e.revision, !.files = e.files,
                     !.seen = @ \cup {s.page.revision}]
  [] e.action = "Retry" ->
    /\ s.pc = "failure" /\ s.failure = 412 /\ Required = 0 /\ s.attempt < 3
    /\ t = [InitState EXCEPT !.attempt = s.attempt+1]
  [] e.action = "Abort" ->
    /\ (s.pc = "page" /\ e.status = 503)
        \/ (s.pc = "failure" /\ e.status =
              IF s.failure = 412 /\ Required = 0 THEN 503 ELSE s.failure)
    /\ t = [s EXCEPT !.pc = "aborted"]
  [] OTHER -> FALSE

(* Explore all page orderings, revision races and two retries over two paths.
   Replies need not be well formed: the bridge must never publish them. *)
F(p, r) == [path |-> p, etag |-> r, size |-> p, diverging |-> FALSE]
PagesFor(r) == {[revision |-> r, files |-> fs, next |-> c] :
  fs \in {<<>>, <<F(1,r)>>, <<F(2,r)>>,
                          <<F(1,r),F(2,r)>>, <<F(2,r),F(1,r)>>},
  c \in {NoCursor, [present |-> TRUE, revision |-> 1, after |-> 1], [present |-> TRUE, revision |-> 2, after |-> 2]}}
Pages == UNION {PagesFor(r) : r \in {1, 2}}
VARIABLE state
vars == <<state>>
Init == state = InitState
Next ==
  \/ Step(state, [action |-> "Request", cursor |-> state.cursor], state')
  \/ \E p \in Pages : Step(state, [action |-> "Reply", status |-> 0, page |-> p], state')
  \/ \E code \in {412, 503} : Step(state, [action |-> "Reply", status |-> code], state')
  \/ Step(state, [action |-> "Continue", cursor |-> state.page.next,
                  files |-> state.files \o state.page.files], state')
  \/ Step(state, [action |-> "Publish", revision |-> state.page.revision,
                  files |-> state.files \o state.page.files], state')
  \/ Step(state, [action |-> "Retry"], state')
  \/ \E code \in {412, 503} : Step(state, [action |-> "Abort", status |-> code], state')
Spec == Init /\ [][Next]_vars
Consistent == Cardinality(state.seen) <= 1
Pinned == Required = 0 \/ state.pc # "done" \/ state.revision = Required
Bounded == state.attempt \in 1..3
=============================================================================
