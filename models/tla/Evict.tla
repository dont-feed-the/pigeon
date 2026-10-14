------------------------------- MODULE Evict -------------------------------
(***************************************************************************)
(* Freeing and keeping a folder on a laptop (spec 08 §8.5, 07 §7.8,       *)
(* 09 §9.4; ADRs 0031, 0046), issue #10.                                   *)
(*                                                                         *)
(* One laptop holds folders F = {a, b} and G = {c}; c has the same content *)
(* (key) as a. The person keeps G and switches F between kept and freed.   *)
(* A content is its key. Files, asides, staged copies and annex objects    *)
(* are inodes; a write through a handle changes its inode, wherever it is  *)
(* linked, or nowhere. The person saves (atomic replace, or in place       *)
(* through a handle opened earlier), opens and closes files at any         *)
(* instant, pigeond being up or down; the group changes F's files; a       *)
(* custodian may lose a copy; pigeond may crash, leaving git's             *)
(* info/sparse-checkout.lock behind.                                       *)
(*                                                                         *)
(* What the model takes from git and git-annex 10.20261006:                *)
(*  - `git sparse-checkout set` unlinks a hidden file that matches the     *)
(*    index, and leaves one that differs (E15, E24).                       *)
(*  - git-annex's drop of key k replaces every file the keys database      *)
(*    associates with k, if unmodified (inode cache, else a hash), by a    *)
(*    pointer file with the old mtime: removeAnnex and resetpointer in     *)
(*    Annex/Content.hs, depopulatePointerFile in                           *)
(*    Annex/Content/PointerFile.hs, isUnmodifiedLowLevel in                *)
(*    Annex/Content/Presence/LowLevel.hs (E24).                            *)
(*  - the drop counts a copy only if it can be locked (spec 09 §9.4), so it    *)
(*    is verified at the instant it runs (AnnexDrop).                      *)
(*  - git clears the skip-worktree bit of a file present on disk           *)
(*    (sparse.expectFilesOutsideOfPatterns = false), so capture sees a     *)
(*    file the person creates in a hidden folder.                          *)
(* Each name in Off removes one safeguard; its configuration must fail.    *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets

CONSTANTS
    K,            \* custody copies that protect a key (ADR 0002)
    Custodians,   \* the custodians
    MaxSaves,     \* saves by the person
    MaxGroup,     \* new versions of F's files from the group
    MaxCrash,     \* crashes of pigeond
    MaxLoss,      \* custody copies lost
    MaxFlips,     \* times the person switches F between kept and freed
    Start0,       \* "kept" or "freed": F at the start
    Edit,         \* the paths the person saves
    Off           \* safeguards removed, see Safeguard below

F == {"a", "b"}
G == {"c"}
Paths == F \cup G
None == 0
PTR == 0             \* the data of a pointer file
PTRINO == 1          \* the inode of every pointer file: they are alike
MaxC == 2 + MaxSaves + MaxGroup
Contents == 1..MaxC

(* Safeguards, each the subject of a negative configuration:              *)
(*  "aside"    hide by moving the file aside, scanning for writers and     *)
(*             checking it (spec 07 §7.5's steps), not by git's unlink         *)
(*  "guard"    a free drops no key that a path outside freed, hidden       *)
(*             folders uses                                                *)
(*  "target"   that guard counts the paths of every folder whose target    *)
(*             is keep, staged and moved-in files too, not visible ones    *)
(*             only                                                        *)
(*  "prune"    the drop of old versions (spec 07 §7.8) has the same guard      *)
(*  "lock"     a drop is verified when it runs, not trusted from the       *)
(*             status report seen when the file was hidden                 *)
(*  "lockfile" a stale info/sparse-checkout.lock is removed at start       *)
(*  "stage"    keep stages complete files and shows them without a         *)
(*             checkout (spec 08 §8.5), instead of ADR 0031's order            *)
(*  "stale"    keep checks a moved-in file against the index before it     *)
(*             shows it, and restages it if main has moved                 *)
(*  "copy"     capture adds a copy (annex.thin=false, spec 07 §7.8)            *)
(*  "journal"  capture skips a path whose file is aside                    *)
On(s) == s \notin Off

VARIABLES
    wt,        \* path -> inode of its file, None if absent
    aside,     \* path -> inode moved aside by a hide, None if none
    akey,      \* path -> the key the hide expected
    proved,    \* path -> the hide was checked and recorded
    stg,       \* path -> inode staged by keep, None if none
    stkey,     \* path -> key of the staged or moved-in file (journal)
    obj,       \* key -> inode of its annex object, None if absent
    data,      \* inode -> content (PTR for the pointer inode)
    nexti,     \* next free inode
    nextc,     \* next fresh content
    hnd,       \* the inode the person holds open for writing, or None
    idx,       \* path -> key in the index
    skip,      \* path -> skip-worktree bit
    hist,      \* path -> keys of its versions this laptop knows
    cust,      \* key -> custodians holding a lockable copy
    seen,      \* key -> protected, as the last status report said
    cap,       \* contents this laptop captured
    personal,  \* contents the person wrote
    owed,      \* contents that must survive: every capture, latest saves
    recs,      \* inodes kept under a recovered name
    target,    \* F's target: "keep" or "free" (the wanted expression)
    up,        \* pigeond runs
    lockfile,  \* a stale info/sparse-checkout.lock
    crashes, losses, flips, saves, groups, opens,
    unsafe     \* a key was dropped while fewer than K copies were locked

vars == << wt, aside, akey, proved, stg, stkey, obj, data, nexti, nextc,
           hnd, idx, skip, hist, cust, seen, cap, personal, owed, recs,
           target, up, lockfile, crashes, losses, flips, saves, groups,
           opens, unsafe >>
files == << wt, aside, akey, proved, stg, stkey, obj, data, nexti, recs >>
annex == << idx, skip, hist, cap >>
world == << cust, seen, personal, owed, nextc, hnd, saves, groups, opens >>
ctl == << target, up, lockfile, crashes, losses, flips, unsafe >>

Protected(k) == Cardinality(cust[k]) >= K
Present(k) == obj[k] # None /\ data[obj[k]] = k
File(p) == wt[p] \notin {None, PTRINO}
Hidden(p) == skip[p] /\ wt[p] = None
Ext1(c) == [i \in 1..nexti |-> IF i = nexti THEN c ELSE data[i]]
Ext2(c, d) == [i \in 1..(nexti + 1) |->
                 IF i = nexti THEN c ELSE IF i = nexti + 1 THEN d ELSE data[i]]

(* The person overwrites inode i: an earlier save of theirs that capture  *)
(* never took is theirs to lose.                                          *)
Overwrite(i) == IF i \notin {None, PTRINO} /\ data[i] \in personal \ cap
                THEN owed \ {data[i]} ELSE owed

-----------------------------------------------------------------------------
(* The person, at any instant.                                            *)

(* An application replaces p atomically; it has closed the old file.    *)
Replace(p) ==
    /\ saves < MaxSaves /\ (wt[p] = None \/ hnd # wt[p])
    /\ wt' = [wt EXCEPT ![p] = nexti]
    /\ data' = Ext1(nextc)
    /\ nexti' = nexti + 1
    /\ owed' = Overwrite(wt[p]) \cup {nextc}
    /\ personal' = personal \cup {nextc}
    /\ nextc' = nextc + 1
    /\ saves' = saves + 1
    /\ UNCHANGED << aside, akey, proved, stg, stkey, obj, recs, annex,
                    cust, seen, hnd, groups, opens, ctl >>

(* An application opens p for writing (at most MaxSaves times).           *)
Open(p) ==
    /\ hnd = None /\ File(p) /\ opens < MaxSaves
    /\ hnd' = wt[p]
    /\ opens' = opens + 1
    /\ UNCHANGED << files, annex, cust, seen, personal, owed, nextc, saves,
                    groups, ctl >>

WriteHandle ==
    /\ hnd # None /\ saves < MaxSaves
    /\ data' = [data EXCEPT ![hnd] = nextc]
    /\ owed' = Overwrite(hnd) \cup {nextc}
    /\ personal' = personal \cup {nextc}
    /\ nextc' = nextc + 1
    /\ saves' = saves + 1
    /\ UNCHANGED << wt, aside, akey, proved, stg, stkey, obj, nexti, recs,
                    annex, cust, seen, hnd, groups, opens, ctl >>

Close ==
    /\ hnd # None
    /\ hnd' = None
    /\ UNCHANGED << files, annex, cust, seen, personal, owed, nextc, saves,
                    groups, opens, ctl >>

ChooseFree ==
    /\ target = "keep" /\ flips < MaxFlips
    /\ target' = "free" /\ flips' = flips + 1
    /\ UNCHANGED << files, annex, world, up, lockfile, crashes, losses,
                    unsafe >>

ChooseKeep ==
    /\ target = "free" /\ flips < MaxFlips
    /\ target' = "keep" /\ flips' = flips + 1
    /\ UNCHANGED << files, annex, world, up, lockfile, crashes, losses,
                    unsafe >>

-----------------------------------------------------------------------------
(* The group and the custodians.                                          *)

(* A new version of p from another device, protected there. A hidden or  *)
(* moved-in path changes in the index only (07 §7.5); a visible, clean    *)
(* one is installed (spec 07 §7.5, modelled by CaptureInstall).               *)
GroupChange(p) ==
    /\ up /\ groups < MaxGroup /\ p \in F /\ aside[p] = None
    /\ \/ /\ skip[p]
          /\ UNCHANGED << wt, obj, data, nexti >>
       \/ /\ ~skip[p] /\ File(p) /\ data[wt[p]] = idx[p] /\ hnd # wt[p]
          /\ obj' = [obj EXCEPT ![nextc] = nexti]
          /\ wt' = [wt EXCEPT ![p] = nexti + 1]
          /\ data' = Ext2(nextc, nextc)
          /\ nexti' = nexti + 2
    /\ idx' = [idx EXCEPT ![p] = nextc]
    /\ hist' = [hist EXCEPT ![p] = @ \cup {nextc}]
    /\ cust' = [cust EXCEPT ![nextc] = Custodians]
    /\ seen' = [seen EXCEPT ![nextc] = TRUE]
    /\ nextc' = nextc + 1
    /\ groups' = groups + 1
    /\ UNCHANGED << aside, akey, proved, stg, stkey, recs, skip, cap,
                    personal, owed, hnd, saves, opens, ctl >>

(* A custodian receives k, from this laptop or from another custodian.    *)
Upload(k, u) ==
    /\ u \notin cust[k]
    /\ \/ up /\ k \in cap /\ Present(k)
       \/ cust[k] # {}
    /\ cust' = [cust EXCEPT ![k] = @ \cup {u}]
    /\ UNCHANGED << files, annex, seen, personal, owed, nextc, hnd, saves,
                    groups, opens, ctl >>

Lose(k, u) ==
    /\ losses < MaxLoss /\ u \in cust[k]
    /\ cust' = [cust EXCEPT ![k] = @ \ {u}]
    /\ losses' = losses + 1
    /\ UNCHANGED << files, annex, seen, personal, owed, nextc, hnd, saves,
                    groups, opens, target, up, lockfile, crashes, flips,
                    unsafe >>

(* A status report tells the laptop whether k is protected.               *)
Observe(k) ==
    /\ seen[k] # Protected(k)
    /\ seen' = [seen EXCEPT ![k] = Protected(k)]
    /\ UNCHANGED << files, annex, cust, personal, owed, nextc, hnd, saves,
                    groups, opens, ctl >>

-----------------------------------------------------------------------------
(* pigeond.                                                               *)

(* F's lock: capture does not scan F while a hide is aside or a keep is   *)
(* not finished (08 §8.5).                                                *)
Busy == \/ On("journal") /\ \E q \in F : aside[q] # None
        \/ target = "keep" /\ \E q \in F : skip[q]

Capture(p) ==
    /\ up /\ File(p) /\ (p \in F => ~Busy)
    /\ On("journal") => aside[p] = None
    /\ LET c == data[wt[p]] IN
       /\ c # idx[p]
       /\ ~(stkey[p] # None /\ c = stkey[p])   \* pigeon's own staged file
       /\ IF On("copy")
          THEN /\ obj' = [obj EXCEPT ![c] = nexti]
               /\ data' = Ext1(c)
               /\ nexti' = nexti + 1
          ELSE /\ obj' = [obj EXCEPT ![c] = wt[p]]   \* annex.thin=true
               /\ UNCHANGED << data, nexti >>
       /\ idx' = [idx EXCEPT ![p] = c]
       /\ hist' = [hist EXCEPT ![p] = @ \cup {c}]
       /\ cap' = cap \cup {c}
    /\ skip' = [skip EXCEPT ![p] = FALSE]
    /\ stkey' = [stkey EXCEPT ![p] = None]
    /\ UNCHANGED << wt, aside, akey, proved, stg, recs, world, ctl >>

(* A path absent on disk and not hidden is captured as a deletion.        *)
CaptureDelete(p) ==
    /\ up /\ wt[p] = None /\ ~skip[p] /\ idx[p] # None
    /\ On("journal") => (aside[p] = None /\ (p \in F => ~Busy))
    /\ idx' = [idx EXCEPT ![p] = None]
    /\ UNCHANGED << files, skip, hist, cap, world, ctl >>

(* Free, step 3: hide a file whose version the status says is protected. *)
Hide(p) ==
    /\ up /\ target = "free" /\ p \in F /\ File(p) /\ aside[p] = None
    /\ idx[p] # None /\ seen[idx[p]] /\ data[wt[p]] = idx[p]
    /\ IF On("aside")
       THEN /\ aside' = [aside EXCEPT ![p] = wt[p]]
            /\ akey' = [akey EXCEPT ![p] = idx[p]]
            /\ wt' = [wt EXCEPT ![p] = None]
            /\ UNCHANGED skip
       ELSE /\ ~lockfile                      \* git unlinks the clean file
            /\ wt' = [wt EXCEPT ![p] = None]
            /\ skip' = [skip EXCEPT ![p] = TRUE]
            /\ UNCHANGED << aside, akey >>
    /\ UNCHANGED << proved, stg, stkey, obj, data, nexti, recs, idx, hist,
                    cap, world, ctl >>

(* The writer scan and the check: a writer, or a file that is not the     *)
(* expected one, goes back (or under a recovered name if p was made       *)
(* again); otherwise git sets the bit with nothing to unlink.             *)
HideFinish(p) ==
    /\ up /\ ~lockfile /\ aside[p] # None /\ ~proved[p]
    /\ IF hnd = aside[p] \/ data[aside[p]] # akey[p]
       THEN /\ IF wt[p] = None
               THEN /\ wt' = [wt EXCEPT ![p] = aside[p]]
                    /\ UNCHANGED recs
               ELSE /\ recs' = recs \cup {aside[p]}
                    /\ UNCHANGED wt
            /\ aside' = [aside EXCEPT ![p] = None]
            /\ UNCHANGED << proved, skip >>
       ELSE /\ proved' = [proved EXCEPT ![p] = TRUE]
            /\ skip' = [skip EXCEPT ![p] = (wt[p] = None)]
            /\ UNCHANGED << wt, aside, recs >>
    /\ UNCHANGED << akey, stg, stkey, obj, data, nexti, idx, hist, cap,
                    world, ctl >>

(* After the keep period, a proved aside is hashed and released.          *)
Release(p) ==
    /\ up /\ aside[p] # None /\ proved[p] /\ hnd # aside[p]
    /\ recs' = IF data[aside[p]] = akey[p] THEN recs ELSE recs \cup {aside[p]}
    /\ aside' = [aside EXCEPT ![p] = None]
    /\ proved' = [proved EXCEPT ![p] = FALSE]
    /\ UNCHANGED << wt, akey, stg, stkey, obj, data, nexti, annex, world,
                    ctl >>

(* Hidden for a drop. Without "target", the skip-worktree bit alone, as   *)
(* 08 §8.5 words it: a moved-in file whose bit is still set counts.       *)
HiddenD(p) == IF On("target") THEN Hidden(p) ELSE skip[p]

(* A key no folder needs: every path using it is hidden in a folder       *)
(* whose target is free.                                                  *)
Guard(k) ==
    \A q \in Paths : idx[q] = k =>
        HiddenD(q) /\ (On("target") => (q \in F /\ target = "free"))

Verified(k) == IF On("lock") THEN Protected(k) ELSE seen[k]

(* git-annex's drop: the object goes, and every unmodified file the keys  *)
(* database associates with k (the index paths) becomes a pointer.        *)
DropEffect(k) ==
    /\ obj' = [obj EXCEPT ![k] = None]
    /\ wt' = [q \in Paths |-> IF idx[q] = k /\ File(q) /\ data[wt[q]] = k
                              THEN PTRINO ELSE wt[q]]
    /\ unsafe' = (unsafe \/ ~Protected(k))
    /\ UNCHANGED << aside, akey, proved, stg, stkey, data, nexti, recs,
                    annex, world, target, up, lockfile, crashes, losses,
                    flips >>

(* Free, step 4: drop the key of a hidden file.                           *)
DropFree(k) ==
    /\ up /\ target = "free" /\ obj[k] # None /\ Verified(k)
    /\ \E p \in F : idx[p] = k /\ HiddenD(p)
    /\ On("guard") => Guard(k)
    /\ DropEffect(k)

(* spec 07 §7.8: once protected, the old versions this laptop holds go.      *)
Prune(k) ==
    /\ up /\ obj[k] # None /\ Verified(k)
    /\ \E p \in Paths : k \in hist[p] /\ idx[p] # k
    /\ On("prune") => Guard(k)
    /\ DropEffect(k)

(* Keep, step 1: fetch every current key of F's hidden files.            *)
Get(k) ==
    /\ up /\ target = "keep" /\ ~Present(k) /\ cust[k] # {}
    /\ \E p \in F : idx[p] = k /\ (Hidden(p) \/ wt[p] = PTRINO)
    /\ obj' = [obj EXCEPT ![k] = nexti]
    /\ data' = Ext1(k)
    /\ nexti' = nexti + 1
    /\ UNCHANGED << wt, aside, akey, proved, stg, stkey, recs, annex,
                    world, ctl >>

AllPresent == \A q \in F : Hidden(q) => Present(idx[q])

(* Keep, step 2: stage a complete copy (all or nothing).                  *)
Stage(p) ==
    /\ up /\ On("stage") /\ target = "keep" /\ p \in F /\ Hidden(p)
    /\ stg[p] = None /\ AllPresent
    /\ stg' = [stg EXCEPT ![p] = nexti]
    /\ stkey' = [stkey EXCEPT ![p] = idx[p]]
    /\ data' = Ext1(idx[p])
    /\ nexti' = nexti + 1
    /\ UNCHANGED << wt, aside, akey, proved, obj, recs, annex, world, ctl >>

(* Keep, step 3: move the staged files in, refusing to replace.           *)
MoveIn(p) ==
    /\ up /\ target = "keep" /\ stg[p] # None
    /\ \A q \in F : Hidden(q) => stg[q] # None
    /\ IF wt[p] = None
       THEN /\ wt' = [wt EXCEPT ![p] = stg[p]]
            /\ UNCHANGED stkey
       ELSE /\ stkey' = [stkey EXCEPT ![p] = None]   \* the person's file stays
            /\ UNCHANGED wt
    /\ stg' = [stg EXCEPT ![p] = None]
    /\ UNCHANGED << aside, akey, proved, obj, data, nexti, recs, annex,
                    world, ctl >>

(* A moved-in file that main has passed since it was staged, unchanged.   *)
Stale(p) == stkey[p] # None /\ stkey[p] # idx[p] /\ File(p)
            /\ data[wt[p]] = stkey[p]

(* Keep, step 4: clear the bit, without a checkout.                       *)
Unhide(p) ==
    /\ up /\ ~lockfile /\ target = "keep" /\ p \in F /\ skip[p] /\ File(p)
    /\ On("stale") => ~Stale(p)
    /\ skip' = [skip EXCEPT ![p] = FALSE]
    /\ stkey' = [stkey EXCEPT ![p] = None]
    /\ UNCHANGED << wt, aside, akey, proved, stg, obj, data, nexti, recs,
                    idx, hist, cap, world, ctl >>

(* A stale moved-in file is pigeon's own: no writer, unchanged; it goes,  *)
(* and keep stages the current key.                                       *)
Restage(p) ==
    /\ up /\ On("stale") /\ skip[p] /\ Stale(p) /\ hnd # wt[p]
    /\ wt' = [wt EXCEPT ![p] = None]
    /\ stkey' = [stkey EXCEPT ![p] = None]
    /\ UNCHANGED << aside, akey, proved, stg, obj, data, nexti, recs, annex,
                    world, ctl >>

(* Keep was given up: staged copies go.                                  *)
Unstage(p) ==
    /\ up /\ target = "free" /\ stg[p] # None
    /\ stg' = [stg EXCEPT ![p] = None]
    /\ stkey' = [stkey EXCEPT ![p] = None]
    /\ UNCHANGED << wt, aside, akey, proved, obj, data, nexti, recs, annex,
                    world, ctl >>

(* ADR 0031's order (without "stage"): show with a checkout, which writes *)
(* a pointer for absent content, then get and `smudge --update`.          *)
ShowCheckout(p) ==
    /\ up /\ ~On("stage") /\ ~lockfile /\ target = "keep" /\ p \in F
    /\ Hidden(p)
    /\ skip' = [skip EXCEPT ![p] = FALSE]
    /\ IF Present(idx[p])
       THEN /\ wt' = [wt EXCEPT ![p] = nexti]
            /\ data' = Ext1(idx[p])
            /\ nexti' = nexti + 1
       ELSE /\ wt' = [wt EXCEPT ![p] = PTRINO]
            /\ UNCHANGED << data, nexti >>
    /\ UNCHANGED << aside, akey, proved, stg, stkey, obj, recs, idx, hist,
                    cap, world, ctl >>

Smudge(p) ==
    /\ up /\ wt[p] = PTRINO /\ Present(idx[p])
    /\ wt' = [wt EXCEPT ![p] = nexti]
    /\ data' = Ext1(idx[p])
    /\ nexti' = nexti + 1
    /\ UNCHANGED << aside, akey, proved, stg, stkey, obj, recs, annex,
                    world, ctl >>

Crash ==
    /\ up /\ crashes < MaxCrash
    /\ up' = FALSE /\ crashes' = crashes + 1
    /\ lockfile' \in {lockfile, TRUE}      \* killed inside `git sparse-checkout`
    /\ UNCHANGED << files, annex, world, target, losses, flips, unsafe >>

(* At start pigeond removes a stale lock (it is the repository's only     *)
(* writer), then reruns the transition toward the target.                 *)
Restart ==
    /\ ~up
    /\ up' = TRUE
    /\ lockfile' = IF On("lockfile") THEN FALSE ELSE lockfile
    /\ UNCHANGED << files, annex, world, target, crashes, losses, flips,
                    unsafe >>

-----------------------------------------------------------------------------

Init ==
    /\ wt = IF Start0 = "kept" THEN [p \in Paths |-> CASE p = "a" -> 4
                                                     [] p = "b" -> 5
                                                     [] p = "c" -> 6]
            ELSE [p \in Paths |-> IF p = "c" THEN 6 ELSE None]
    /\ aside = [p \in Paths |-> None]
    /\ akey = [p \in Paths |-> None]
    /\ proved = [p \in Paths |-> FALSE]
    /\ stg = [p \in Paths |-> None]
    /\ stkey = [p \in Paths |-> None]
    /\ obj = [k \in Contents |-> CASE k = 1 -> 2
                                  [] k = 2 /\ Start0 = "kept" -> 3
                                  [] OTHER -> None]
    /\ data = [i \in 1..6 |-> CASE i = 1 -> PTR
                               [] i \in {2, 4, 6} -> 1
                               [] OTHER -> 2]
    /\ nexti = 7
    /\ nextc = 3
    /\ hnd = None
    /\ idx = [p \in Paths |-> IF p = "b" THEN 2 ELSE 1]
    /\ skip = [p \in Paths |-> p \in F /\ Start0 = "freed"]
    /\ hist = [p \in Paths |-> {idx[p]}]
    /\ cust = [k \in Contents |-> IF k \in {1, 2} THEN Custodians ELSE {}]
    /\ seen = [k \in Contents |-> k \in {1, 2}]
    /\ cap = {}
    /\ personal = {}
    /\ owed = {1, 2}
    /\ recs = {}
    /\ target = IF Start0 = "kept" THEN "keep" ELSE "free"
    /\ up = TRUE
    /\ lockfile = FALSE
    /\ crashes = 0 /\ losses = 0 /\ flips = 0 /\ saves = 0 /\ groups = 0
    /\ opens = 0
    /\ unsafe = FALSE

Pigeond ==
    \/ \E p \in Paths : \/ Capture(p) \/ CaptureDelete(p) \/ Hide(p)
                        \/ HideFinish(p) \/ Release(p) \/ Stage(p)
                        \/ MoveIn(p) \/ Unhide(p) \/ Restage(p) \/ Unstage(p)
                        \/ ShowCheckout(p) \/ Smudge(p)
    \/ \E k \in Contents : DropFree(k) \/ Prune(k) \/ Get(k)
    \/ Restart

Next ==
    \/ Pigeond
    \/ \E p \in Edit : Replace(p) \/ Open(p)
    \/ \E p \in Paths : GroupChange(p)
    \/ WriteHandle \/ Close \/ ChooseFree \/ ChooseKeep \/ Crash
    \/ \E k \in Contents, u \in Custodians : Upload(k, u) \/ Lose(k, u)
    \/ \E k \in Contents : Observe(k)

Spec == Init /\ [][Next]_vars

(* For fingerprints: an inode nothing links or holds is garbage, and its  *)
(* data cannot matter again, since new inodes are fresh.                  *)
Used == {wt[p] : p \in Paths} \cup {aside[p] : p \in Paths}
        \cup {stg[p] : p \in Paths} \cup {obj[k] : k \in Contents}
        \cup recs \cup {hnd}
View == << wt, aside, akey, proved, stg, stkey, obj,
           [i \in DOMAIN data |-> IF i \in Used THEN data[i] ELSE PTR],
           nexti, nextc, hnd, idx, skip, hist, cust, seen, cap, personal,
           owed, recs, target, up, lockfile, crashes, losses, flips, saves,
           groups, opens, unsafe >>

(* Liveness assumptions: pigeond restarts and keeps working, uploads and  *)
(* status reports arrive, and the person closes what they open. Prune is  *)
(* not assumed: it may wait.                                              *)
Fairness ==
    /\ \A p \in Paths : /\ WF_vars(Capture(p)) /\ WF_vars(Hide(p))
                        /\ WF_vars(HideFinish(p)) /\ WF_vars(Release(p))
                        /\ WF_vars(Stage(p)) /\ WF_vars(MoveIn(p))
                        /\ WF_vars(Unhide(p)) /\ WF_vars(Restage(p))
                        /\ WF_vars(Unstage(p)) /\ WF_vars(ShowCheckout(p))
                        /\ WF_vars(Smudge(p))
    /\ \A k \in Contents : /\ WF_vars(DropFree(k)) /\ WF_vars(Get(k))
                           /\ WF_vars(Observe(k))
                           /\ \A u \in Custodians : WF_vars(Upload(k, u))
    /\ WF_vars(Restart) /\ WF_vars(Close)

LiveSpec == Spec /\ Fairness

-----------------------------------------------------------------------------
(* Safety.                                                                *)

Linked == {data[wt[p]] : p \in {q \in Paths : File(q)}}
          \cup {data[aside[p]] : p \in {q \in Paths : aside[q] # None}}
          \cup {data[stg[p]] : p \in {q \in Paths : stg[q] # None}}
          \cup {data[i] : i \in recs}

(* No captured version and no latest save is lost: each is in a file, an  *)
(* aside, a staged or recovered copy, an intact annex object, or a       *)
(* custodian.                                                             *)
NoLoss == \A c \in owed : c \in Linked \/ Present(c) \/ cust[c] # {}

(* No folder ever shows a pointer file (spec 08 §8.5).                        *)
NoPointer == \A p \in Paths : wt[p] # PTRINO

(* A laptop drops a key only while K custodians hold it (ADR 0046).       *)
DropsProtected == ~unsafe

(* Capture commits only what the person wrote, never pigeon's own file.  *)
CaptureOnlySaves == cap \subseteq personal

(* Nobody deletes, so no path may be captured as deleted.                *)
NoDeletion == \A p \in Paths : idx[p] # None

-----------------------------------------------------------------------------
(* Liveness, under Fairness.                                              *)

(* Freed: F's files are hidden, nothing aside or staged, and their keys   *)
(* gone unless G uses them.                                               *)
Freed == \A p \in F : /\ Hidden(p) /\ aside[p] = None /\ stg[p] = None
                      /\ \/ idx[p] = None \/ obj[idx[p]] = None
                         \/ \E q \in G : idx[q] = idx[p]
Kept == \A p \in F : ~skip[p] /\ File(p) /\ stg[p] = None

FreeLive == (target = "free") ~> (Freed \/ target = "keep")
KeepLive == (target = "keep") ~> (Kept \/ target = "free")
=============================================================================
