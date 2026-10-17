------------------------ MODULE CaptureInstallTrace -------------------------
(* Trace refinement of real driver transitions to CaptureInstall. The log
   constrains inode identities, bytes, keys, signed storage, journal flags,
   writer observations, keep sets and Git's cache-clean result. The model
   supplies unobserved clocks and ghost history; it never supplies outputs
   to the implementation. Each operation's before and after must be one
   driver step, with bounded person/time steps within the observation pair.
   Environment steps explain gaps between operations; they never replace
   a missing driver action. *)
EXTENDS DaemonDriver, Sequences, CaptureInstallCodeLog
CONSTANTS TraceLog, EnvBudget
VARIABLES l, budget, segment
ToSet(s) == {s[i] : i \in DOMAIN s}
Ev == TraceLog[l]
Present(v) == {v.p, v.aside, v.stage} \ {0}
AsKept(v) == { [i |-> x.i, e |-> x.e] : x \in ToSet(v.kept) }
Files(v) == ToSet(v.files)
Writers(v) == {x.i : x \in {y \in Files(v) : y.known /\ y.w}}
Clear(v) == {x.i : x \in {y \in Files(v) : y.known /\ ~y.w}}
Bytes(v) == {x.c : x \in Files(v)} \ {0}
Projection == [at |-> at, kept |-> kept, fc |-> fc, key |-> idx.key,
               clean |-> Clean, cur |-> cur, store |-> store,
               hnd |-> hnd, hid |-> hid, jr |-> jr, div |-> div, hidden |-> hidden, operation |-> operation,
               stats |-> [i \in 0..MaxIno |-> Stat(i)]]
Match(p, v) ==
  /\ p.at = [p |-> v.p, aside |-> v.aside, stage |-> v.stage]
  /\ p.kept = AsKept(v)
  /\ \A x \in Files(v) : p.fc[x.i] = x.c
  (* Git may conservatively leave stat data empty until refresh. A real
     positive clean claim must be justified by the model's cache rule. *)
  /\ p.key = v.index /\ (v.clean => p.clean) /\ p.cur = v.current
  /\ p.store = ToSet(v.signed)
  /\ Writers(v) \subseteq p.hnd /\ p.hnd \cap Clear(v) = {}
  /\ p.hidden = v.hidden
  /\ (v.open /\ v.operation # "" => p.operation = v.operation)
  /\ p.jr.open = v.open
  /\ (v.open => /\ p.jr.proved = v.proved /\ p.jr.new = v.new /\ p.jr.exp = v.expected)
  /\ (v.open => /\ ((p.jr.st = p.stats[v.aside]) = v.proof_matches)
                 /\ ((p.jr.st = p.stats[v.p]) = v.path_matches))
  /\ p.div.k = v.divergence /\ p.div.key = v.mine
(* A hidden writer may change a retained inode after proof. Check the
   bytes displaced at proof, and let NoLoss govern those later writes. *)
Safety == DriverSafety /\ \A d \in disp : d.c = d.e

(* A trace may begin at a checkpoint, not at the group's genesis. The
   observed state fixes the checkpoint; invariants are checked there too. *)
TraceInit ==
  LET v == TraceLog[1].vars.before
      ids == (Present(v) \cup {x.i : x \in ToSet(v.kept)} \cup {v.expected_inode}) \ {0}
      contents == Bytes(v) \cup ToSet(v.signed) \cup {v.index, v.current, v.new, v.expected, v.mine}
  IN
  /\ l = 1 /\ budget = 0 /\ segment = "gap"
  /\ hidden = v.hidden
  /\ operation = IF v.open /\ v.operation # "" THEN v.operation ELSE TraceLog[1].vars.kind
  /\ fc = [i \in Inodes |-> IF \E x \in Files(v) : x.i = i
                            THEN (CHOOSE x \in Files(v) : x.i = i).c ELSE 0]
  /\ fm = [i \in Inodes |-> 0] /\ fresh = {}
  /\ at = [p |-> v.p, aside |-> v.aside, stage |-> v.stage]
  /\ kept = AsKept(v) /\ orph = {} /\ punl = {}
  /\ hnd = Writers(v) /\ hid = {}
  /\ idx = [key |-> v.index, st |-> IF v.clean THEN Stat(v.p) ELSE NoStat, racy |-> FALSE]
  /\ cur = v.current /\ store = ToSet(v.signed)
  /\ saved = Bytes(v) \ store /\ dropped = {}
  /\ div = [k |-> v.divergence, key |-> v.mine, st |-> Stat(v.p)]
  /\ pc = IF v.open THEN "down" ELSE "idle"
  /\ jr = [open |-> v.open, new |-> v.new, exp |-> v.expected,
           st |-> IF v.expected_inode = 0 THEN NoStat ELSE IF v.proof_matches THEN Stat(v.aside)
                   ELSE IF v.path_matches THEN Stat(v.p) ELSE <<v.expected_inode, 1>>,
           tmp |-> IF v.stage # 0 THEN v.stage ELSE v.p, res |-> FALSE, proved |-> v.proved]
  /\ deferred = FALSE /\ who = IF v.p = 0 THEN IF v.index # Absent /\ ~v.hidden THEN "person" ELSE "pigeon"
                            ELSE IF fc[v.p] = v.index THEN "pigeon" ELSE "person"
  /\ pdel = (v.p = 0 /\ v.index # Absent /\ ~v.hidden) /\ gap = {} /\ disp = {}
  /\ n = [act |-> 0, grp |-> 0, crash |-> 0, res |-> 0,
           ino |-> IF ids = {} THEN 0 ELSE CHOOSE i \in ids : \A j \in ids : j <= i,
           cont |-> CHOOSE c \in contents : \A d \in contents : d <= c]
  /\ Match(Projection, v) /\ Safety

Common(step) == step /\ SameControl
VerifiedRestore(v, requested) ==
  /\ requested # Absent /\ requested \in ToSet(v.signed) /\ v.p # 0
  /\ \E f \in Files(v) : f.i = v.p /\ f.c = requested
Driver == CASE Ev.action = "Capture" ->
                 (CaptureWith(Ev.vars.before.allowed) /\ hidden' = FALSE /\ UNCHANGED operation)
                 \/ ConcurrentCapture(Ev.vars.before.recent, Ev.vars.before.allowed) \/ AdoptPresent
            [] Ev.action = "Start" ->
                 CASE Ev.vars.kind = "install" -> StartInstall
                   [] Ev.vars.kind = "resolve" -> StartResolve
                   [] Ev.vars.kind = "free" -> StartFree
                   [] Ev.vars.kind = "keep" -> StartKeep
                   [] Ev.vars.kind = "thin" -> StartThin
                   [] Ev.vars.kind = "restore" -> StartRestore(Ev.vars.request,
                         IF Ev.vars.before.p = 0 THEN Absent ELSE Ev.vars.before.index)
                   [] OTHER -> FALSE
            [] Ev.action = "VerifyRestore" ->
                 /\ Ev.vars.kind = "restore" /\ pc = "idle" /\ ~jr.open
                 /\ VerifiedRestore(Ev.vars.before, Ev.vars.request)
                 /\ VerifiedRestore(Ev.vars.after, Ev.vars.request)
                 /\ Ev.vars.request \in store /\ Ev.vars.request # Absent
                 /\ at.p # 0 /\ fc[at.p] = Ev.vars.request
                 /\ UNCHANGED driverVars
            [] Ev.action = "TakeGroup" -> StartResolve
            [] Ev.action = "MoveAside" -> Common(MoveAside)
            [] Ev.action = "Scan" -> Common(Scan)
            [] Ev.action = "Check" -> Common(CheckWith(operation # "thin"))
            [] Ev.action = "RenameIn" -> Common(RenameIn)
            [] Ev.action = "Index" ->
                 CASE operation = "free" -> HideIndex
                   [] operation = "keep" -> KeepIndex
                   [] operation = "resolve" -> ResolveIndex
                   [] OTHER -> Common(Index)
            [] Ev.action = "Done" -> IF operation \in {"thin", "restore"} THEN UnindexedDone ELSE Common(Done)
            [] Ev.action = "Release" -> Common(Release)
            [] Ev.action = "Recover" -> Common(Recover) \/ (~jr.open /\ pc = "idle" /\ UNCHANGED driverVars)
            [] Ev.action = "RecoverEnd" -> DriverRecoverEnd
            [] Ev.action = "Cleanup" -> UNCHANGED driverVars
            [] Ev.action = "Adopt" -> AdoptPresent
            [] Ev.action = "Conflict" -> NoticeConflict
            [] Ev.action = "Resolve" -> ResolveTable \/ ReSuggest
            [] Ev.action = "Withdraw" -> Withdraw
            [] Ev.action = "Decide" -> Common(Decide) \/ EndDivergence \/ UNCHANGED driverVars
            [] Ev.action = "Reconcile" -> NoticeConflict \/ EndDivergence \/ Common(Decide) \/ UNCHANGED driverVars
            [] OTHER -> FALSE
(* Capture can discover no change, or only refresh a conservative cache.
   A failed operation can have made no abstract change. Neither case can
   synthesize an omitted install, proof, index update, or journal end. *)
DriverStep == /\ (Ev.action \in {"Start", "TakeGroup", "Release"} => ~Ev.vars.before.pending)
              /\ \/ Driver
                 \/ Ev.action = "Capture" /\ (Common(Refresh) \/ UNCHANGED driverVars)
                 \/ ~Ev.vars.ok /\ UNCHANGED driverVars
(* Only explore handles that can explain a remaining observation. Removing
   irrelevant opens/closes prunes the search without admitting new steps. *)
FutureWrite(i) == \E k \in l..Len(TraceLog), side \in {"before", "after"} :
                   \E x \in Files(TraceLog[k].vars[side]) : x.i = i /\ x.c # fc[i]
NeededOpen(v) ==
  /\ Open
  /\ \/ at.p \in Writers(v) \ hnd /\ hnd' = hnd \cup {at.p} /\ UNCHANGED hid
     \/ at.p \notin hid /\ FutureWrite(at.p) /\ hid' = hid \cup {at.p} /\ UNCHANGED hnd
NeededClose(v) == \E i \in (hnd \ Writers(v)) \cup {j \in hid : ~FutureWrite(j)} :
                    Close /\ hnd' = hnd \ {i} /\ hid' = hid \ {i}
PersonStep(v) ==
  \/ \E x \in Files(v) : x.i \in Linked /\ fc[x.i] # x.c
                         /\ (x.i = at.p \/ x.i \in hnd \cup hid)
                         /\ WriteValue(x.i, x.c)
  \/ \E x \in Files(v) : x.i \in Linked /\ v.open /\ fc[x.i] = x.c
                         /\ ((x.i = v.p /\ ((jr.st = Stat(x.i)) # v.path_matches))
                              \/ (x.i = v.aside /\ ((jr.st = Stat(x.i)) # v.proof_matches)))
                         /\ (x.i = at.p \/ x.i \in hnd \cup hid) /\ WriteValue(x.i, x.c)
  \/ v.p # 0 /\ at.p # v.p /\ ReplaceValue((CHOOSE x \in Files(v) : x.i = v.p).c)
  \/ v.p = 0 /\ Delete
  \/ NeededOpen(v)
  \/ NeededClose(v)
  \/ Tick
EnvironmentStep(v) ==
  \/ PersonStep(v)
  \/ \E c \in (ToSet(v.signed) \ store) \cup {v.current} : VersionValueWith(c, FALSE)
  \/ v.clean /\ Refresh
  \/ Retry
  \/ Ev.action = "Recover" /\ Crash
Environment ==
  LET v == Ev.vars.before IN
  \/ ~v.hidden /\ VisibleBit
  \/ EnvironmentStep(v) /\ SameControl
TraceNext == l <= Len(TraceLog) /\
  \/ /\ segment = "gap" /\ budget < EnvBudget /\ Environment /\ Safety'
     /\ budget' = budget + 1 /\ UNCHANGED <<l, segment>>
  \/ /\ segment = "gap" /\ Match(Projection, Ev.vars.before)
     /\ segment' = "before" /\ budget' = 0 /\ UNCHANGED <<driverVars, l>>
  (* The person can act while an asynchronous driver call is in flight.
     Exactly one driver action still lies between the two observations;
     only person/time steps can interleave within that interval. *)
  \/ /\ segment \in {"before", "after"} /\ budget < EnvBudget
     (* A crash pair ends at the next process's first checkpoint. Git's
        index read during that restart can clear skip-worktree for a
        visible path. This only clears the bit; it cannot set free's bit,
        refresh the key, or replace a missing journal/driver action. *)
     /\ ((PersonStep(Ev.vars.after) /\ SameControl)
          \/ (Ev.vars.crashed /\ VisibleBit)) /\ Safety'
     /\ budget' = budget + 1 /\ UNCHANGED <<l, segment>>
  \/ /\ segment = "before" /\ DriverStep /\ Safety'
     /\ (Ev.vars.ok /\ ~Ev.vars.crashed /\ Ev.action # "Cleanup" => pc' = Ev.vars.phase)
     /\ segment' = "after" /\ UNCHANGED <<l, budget>>
  \/ /\ segment = "after" /\ Match(Projection, Ev.vars.after)
     /\ (Ev.vars.ok /\ ~Ev.vars.crashed /\ Ev.action # "Cleanup" => pc' = Ev.vars.phase)
     /\ l' = l + 1 /\ budget' = 0 /\ segment' = "gap" /\ UNCHANGED driverVars
TraceSpec == TraceInit /\ [][TraceNext]_<<driverVars, l, budget, segment>>
(* A witness-search shortcut only. Every eager step is a TraceNext step;
   prefer matching checkpoints and omit hidden writer handles. A driver
   that can already explain its output need not wait for person steps.
   Rejection here is inconclusive and must retry the full TraceSpec. *)
EagerTraceNext ==
  /\ TraceNext
  /\ hid' = {}
  /\ (segment = "gap" /\ Match(Projection, Ev.vars.before) /\ ENABLED DriverStep => segment' = "before")
  /\ (segment = "before" /\ ENABLED (DriverStep /\ Match(Projection', Ev.vars.after)) => segment' = "after")
  /\ (segment = "after" /\ Match(Projection, Ev.vars.after) => l' = l + 1)
EagerTraceSpec == TraceInit /\ [][EagerTraceNext]_<<driverVars, l, budget, segment>>
TraceNotConsumed == l <= Len(TraceLog)
=============================================================================
