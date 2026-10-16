------------------------ MODULE BridgeSyncSystem ------------------------
(* Two bridge clients race accepted updates supplied by regular peers and
   decisions supplied by an authorized control peer. This is the native/DAV
   interface, NOT a native merge or network model: each client's daemon has its
   own accepted catalogue. Valid native edits and decisions arriving at one
   daemon are atomic inputs; signatures and immutable content are assumed.
   Path 1 needs approval; path 2 is writable. Client overlays are private.
   Whole-catalogue conditions apply to batches; single-path conditions rebase
   unrelated changes. An Applied save can outlive a failed refresh/response.
   Sessions may restart, retaining native saves and dropping only request state.
   No fairness or eventual delivery is asserted. Mutants weaken the two key
   interface guarantees. *)
EXTENDS Naturals, FiniteSets, TLC
CONSTANTS WeakConditions, LeakSuggestions, Scenario
Clients == {1, 2}
Paths == {1, 2}
Values == {0, 1}
NoOverlay == 3
Empty == [p \in Paths |-> 0]
NoPending == [p \in Paths |-> NoOverlay]
VARIABLES accepted, pending, pc, baseline, batch, target, value, saved,
          acknowledged, staleOverwrite, leak, lostResponse, refreshFailed,
          retries, completed, declined, responseView, pinned
vars == <<accepted, pending, pc, baseline, batch, target, value, saved,
          acknowledged, staleOverwrite, leak, lostResponse, refreshFailed,
          retries, completed, declined, responseView, pinned>>
View(c, a, o) == [files |-> [p \in Paths |-> IF o[c][p] = NoOverlay THEN a[p] ELSE o[c][p]],
                 diverging |-> {p \in Paths : o[c][p] # NoOverlay}]
Visible(c) == View(c, accepted[c], pending)
Init ==
  /\ accepted = [c \in Clients |-> Empty] /\ pending = [c \in Clients |-> NoPending]
  /\ pc = [c \in Clients |-> "idle"]
  /\ baseline = [c \in Clients |-> Visible(c)]
  /\ saved = baseline /\ acknowledged = [c \in Clients |-> FALSE]
  /\ batch = [c \in Clients |-> FALSE] /\ target = [c \in Clients |-> 1]
  /\ value = [c \in Clients |-> 0]
  /\ staleOverwrite = FALSE /\ leak = FALSE /\ lostResponse = FALSE
  /\ refreshFailed = FALSE /\ retries = [c \in Clients |-> 0]
  /\ completed = [c \in Clients |-> 0]
  /\ declined = {} /\ responseView = baseline
  /\ pinned = [c \in Clients |-> FALSE]
Read(c) ==
  /\ pc[c] = "idle" /\ completed[c] < 2
  /\ (Scenario = "retry" => c = 1)
  /\ baseline' = [baseline EXCEPT ![c] = Visible(c)]
  /\ pc' = [pc EXCEPT ![c] = "read"]
  /\ target' = [target EXCEPT ![c] = IF Scenario = "suggestion" THEN 1 ELSE 2]
  /\ value' = [value EXCEPT ![c] = 1]
  /\ batch' = [batch EXCEPT ![c] = Scenario = "batch"]
  /\ UNCHANGED <<accepted, pending, saved, acknowledged, staleOverwrite, leak,
                 lostResponse, refreshFailed, retries, completed, declined, responseView, pinned>>
Valid(c) == IF batch[c] THEN Visible(c) = baseline[c]
            ELSE Visible(c).files[target[c]] = baseline[c].files[target[c]]
Touched(c) == IF batch[c] THEN Paths ELSE {target[c]}
Save(c) ==
  /\ pc[c] = "read"
  /\ IF Valid(c) \/ WeakConditions THEN
       LET a == [p \in Paths |-> IF p \in Touched(c) /\ (p = 2 \/ LeakSuggestions)
                                 THEN value[c] ELSE accepted[c][p]]
           o == [pending EXCEPT ![c] = [p \in Paths |->
                   IF p \in Touched(c) /\ p = 1 THEN value[c] ELSE pending[c][p]]]
       IN /\ accepted' = [accepted EXCEPT ![c] = a] /\ pending' = o
          /\ saved' = [saved EXCEPT ![c] = View(c, a, o)]
          /\ pc' = [pc EXCEPT ![c] = "saved"]
          /\ staleOverwrite' = (staleOverwrite \/ ~Valid(c))
          /\ leak' = (leak \/ (1 \in Touched(c) /\ a[1] # accepted[c][1]))
          /\ declined' = IF 1 \in Touched(c) THEN declined \ {c} ELSE declined
     ELSE /\ pc' = [pc EXCEPT ![c] = "failed"]
          /\ UNCHANGED <<accepted, pending, saved, staleOverwrite, leak, declined>>
  /\ UNCHANGED <<baseline, batch, target, value, acknowledged, lostResponse,
                 refreshFailed, retries, completed, responseView, pinned>>
Refresh(c) ==
  /\ pc[c] = "saved"
  /\ pc' = [pc EXCEPT ![c] = IF Visible(c) = saved[c] THEN "refreshed" ELSE "failed"]
  /\ refreshFailed' = (refreshFailed \/ Visible(c) # saved[c])
  /\ pinned' = [pinned EXCEPT ![c] = Visible(c) = saved[c]]
  /\ UNCHANGED <<accepted, pending, baseline, batch, target, value, saved,
                 acknowledged, staleOverwrite, leak, lostResponse, retries, completed, declined, responseView>>
Response(c) ==
  /\ pc[c] \in {"saved", "refreshed", "failed"}
  /\ \E delivered \in BOOLEAN :
       /\ acknowledged' = [acknowledged EXCEPT ![c] = delivered /\ pc[c] = "refreshed"]
       /\ responseView' = [responseView EXCEPT ![c] = IF pc[c] = "refreshed" THEN saved[c] ELSE baseline[c]]
       /\ lostResponse' = (lostResponse \/ (~delivered /\ pc[c] # "failed"))
  /\ pc' = [pc EXCEPT ![c] = "done"]
  /\ UNCHANGED <<accepted, pending, baseline, batch, target, value, saved,
                 staleOverwrite, leak, refreshFailed, retries, completed, declined, pinned>>
Restart(c) ==
  /\ pc[c] \in {"done", "saved", "refreshed", "failed"} /\ completed[c] < 2
  /\ pc' = [pc EXCEPT ![c] = "idle"]
  /\ completed' = [completed EXCEPT ![c] = @ + 1]
  /\ retries' = [retries EXCEPT ![c] = IF acknowledged[c] THEN @ ELSE @ + 1]
  /\ acknowledged' = [acknowledged EXCEPT ![c] = FALSE]
  /\ pinned' = [pinned EXCEPT ![c] = FALSE]
  /\ UNCHANGED <<accepted, pending, baseline, batch, target, value, saved,
                 staleOverwrite, leak, lostResponse, refreshFailed, declined, responseView>>
NativeEdit ==
  \E c \in Clients, p \in Paths, v \in Values :
    /\ accepted' = [accepted EXCEPT ![c][p] = v]
    /\ UNCHANGED <<pending, pc, baseline, batch, target, value, saved,
                   acknowledged, staleOverwrite, leak, lostResponse, refreshFailed,
                   retries, completed, declined, responseView, pinned>>
Decide ==
  \E c \in Clients, approve \in BOOLEAN :
    /\ pending[c][1] # NoOverlay /\ c \notin declined
    /\ accepted' = IF approve THEN [accepted EXCEPT ![c][1] = pending[c][1]] ELSE accepted
    /\ pending' = IF approve THEN [pending EXCEPT ![c][1] = NoOverlay] ELSE pending
    /\ declined' = IF approve THEN declined ELSE declined \cup {c}
    /\ UNCHANGED <<pc, baseline, batch, target, value, saved, acknowledged,
                   staleOverwrite, leak, lostResponse, refreshFailed, retries, completed, responseView, pinned>>
Resolve ==
  \E c \in declined :
    /\ pending' = [pending EXCEPT ![c][1] = NoOverlay]
    /\ declined' = declined \ {c}
    /\ UNCHANGED <<accepted, pc, baseline, batch, target, value, saved, acknowledged,
                   staleOverwrite, leak, lostResponse, refreshFailed, retries, completed, responseView, pinned>>
Next == NativeEdit \/ Decide \/ Resolve \/ \E c \in Clients :
        Read(c) \/ Save(c) \/ Refresh(c) \/ Response(c) \/ Restart(c)
Spec == Init /\ [][Next]_vars
ConditionsHold == ~staleOverwrite
SuggestionsStayPrivate == ~leak
(* A successful response is tied to the saved catalogue, not whatever a later
   poll happens to see. Replication/decisions may immediately change it again. *)
AcknowledgedSaved == \A c \in Clients : acknowledged[c] => pinned[c] /\ responseView[c] = saved[c]
NoUncertainRetry == ~(completed[1] = 1 /\ retries[1] > 0 /\ lostResponse /\ acknowledged[1])
Small == IF Scenario = "retry" THEN completed[1] <= 1 /\ completed[2] = 0
         ELSE completed[1] = 0 /\ completed[2] = 0
=============================================================================
