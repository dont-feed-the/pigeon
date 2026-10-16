--------------------------- MODULE BridgeSync ---------------------------
(* DAV request contract shared by observed-trace replay. Observations are
   producer facts, not a proof of daemon durability or HTTP delivery. No
   intermediate catalogue, successful commit or delivered response is inferred.
   Native conditional-save and suggestion semantics are explored separately by
   BridgeSyncSystem. Zero denotes an absent opaque token. *)
EXTENDS Naturals, Sequences, FiniteSets
CONSTANTS Method, Resource, Target
EmptyFiles == <<>>
TokenAt(fs, p) == IF \E i \in 1..Len(fs) : fs[i].path = p
                 THEN fs[CHOOSE i \in 1..Len(fs) : fs[i].path = p].etag ELSE 0
FileTokenAt(fs, p) == IF \E i \in 1..Len(fs) : fs[i].path = p /\ ~fs[i].directory
                     THEN TokenAt(fs, p) ELSE 0
InitState == [pc |-> "ready", revision |-> 0, files |-> EmptyFiles,
              submitted |-> FALSE, changes |-> <<>>, saved |-> 0,
              suggested |-> 0, etag |-> 0, failure |-> 0]
Expected(s, e) ==
  IF Len(e.changes) = 1 THEN
    /\ e.conditioned /\ Len(e.expected) = 1
    /\ e.expected[1].path = e.changes[1].path
    /\ e.expected[1].etag = FileTokenAt(s.files, e.changes[1].path)
  ELSE ~e.conditioned /\ e.expected = <<>>
Step(s, e, t) ==
  CASE e.action = "View" ->
    /\ s.pc \in {"ready", "applied", "view"}
    /\ IF s.saved # 0 THEN e.required = s.saved
          ELSE e.required = 0 \/ e.required = s.revision
    /\ IF e.status = 0 THEN
         /\ e.revision # 0 /\ (e.required = 0 \/ e.required = e.revision)
         /\ t = [s EXCEPT !.pc = IF s.saved = 0 THEN "view" ELSE "refreshed",
                          !.revision = e.revision, !.files = e.files]
       ELSE /\ e.status \in 400..599
            /\ t = [s EXCEPT !.pc = "failed", !.failure = e.status]
  [] e.action = "Submit" ->
    /\ s.pc = "view" /\ ~s.submitted /\ Len(e.changes) > 0
    /\ e.revision = s.revision /\ Expected(s, e)
    /\ (Resource = "files" /\ Method = "PUT" =>
          Len(e.changes) = 1 /\ e.changes[1].kind = "put" /\ e.changes[1].path = Target)
    /\ t = [s EXCEPT !.pc = "waiting", !.submitted = TRUE, !.changes = e.changes]
  [] e.action = "ApplyReply" ->
    /\ s.pc = "waiting"
    /\ CASE e.kind = "applied" ->
         /\ e.revision # 0 /\ e.suggested \in 0..Len(s.changes)
         /\ t = [s EXCEPT !.pc = "applied", !.saved = e.revision, !.suggested = e.suggested]
       [] e.kind = "failure" ->
         /\ e.status \in 400..599
         /\ t = [s EXCEPT !.pc = "failed", !.failure = e.status]
       [] e.kind = "unknown" -> t = [s EXCEPT !.pc = "failed", !.failure = 503]
       [] OTHER -> FALSE
  [] e.action = "Written" ->
    /\ s.pc \in {"view", "refreshed"}
    /\ (s.submitted => s.pc = "refreshed")
    /\ e.etag # 0 /\ e.etag = TokenAt(s.files, e.path)
    /\ e.suggested = s.suggested
    /\ (Resource = "files" /\ Method = "PUT" => e.path = Target)
    /\ (s.submitted => \E i \in 1..Len(s.changes) :
                        s.changes[i].path = e.path /\ s.changes[i].kind = "put")
    /\ t = [s EXCEPT !.pc = "written", !.etag = e.etag]
  [] e.action = "Response" ->
    /\ s.pc \notin {"done", "waiting"}
    /\ (Resource = "files" /\ Method \in {"GET", "HEAD"} /\ e.status = 304 =>
          s.pc = "view" /\ e.etag # 0 /\ e.etag = FileTokenAt(s.files, Target))
    /\ IF e.status \in 200..299 THEN
         /\ s.pc # "failed"
         /\ (Resource = "files" => s.pc \notin {"ready"})
         /\ ((Resource = "files" /\ Method \in {"PUT", "MOVE", "COPY"}) \/ Resource = "final" =>
               s.pc = "written" /\ e.etag = s.etag /\ e.etag # 0)
         /\ ((Resource = "files" /\ Method = "PUT") \/ Resource = "final" => s.submitted)
         /\ (Resource = "files" /\ Method \in {"GET", "HEAD"} =>
               s.pc = "view" /\ e.etag # 0 /\ e.etag = FileTokenAt(s.files, Target))
         /\ (Resource = "files" /\ Method = "DELETE" /\ FileTokenAt(s.files, Target) # 0 =>
               s.submitted /\ \E i \in 1..Len(s.changes) :
                 s.changes[i].path = Target /\ s.changes[i].kind = "remove")
         /\ (s.submitted => s.saved # 0)
         /\ (s.submitted /\ Method \in {"PUT", "MOVE", "COPY"} =>
               s.pc = "written" /\ e.etag = s.etag /\ e.etag # 0)
         /\ (e.etag # 0 /\ s.submitted => s.pc = "written" /\ e.etag = s.etag)
         /\ (s.pc = "written" => e.etag = s.etag)
       ELSE /\ e.status \in 300..599
            /\ (s.pc = "failed" => e.status = s.failure)
    /\ t = [s EXCEPT !.pc = "done"]
  [] OTHER -> FALSE
=============================================================================
