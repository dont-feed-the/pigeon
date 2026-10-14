# 0056. Every asserting experiment runs on Linux and macOS; a platform difference is shown, never hidden

- **Status:** Proposed, 14 October 2026, by the agent, for issue #10
  (Guillaume's review: "Align formal models and fault-injection coverage with
  the chosen git-annex architecture"). Applies ADR 0026 to the whole lab.

## Context

ADR 0026 lets an experiment join CI once its script asserts its results. Until
now the `lab` workflow ran E20, E21, E22 and E23; E12–E13, E14, E15, E15b,
E18 and E19 asserted but ran by hand, on Linux only. E19, the install against racing
saves, had never run on macOS, although 07 §7.5 names a macOS writer scan
(`libproc`) and ADR 0040 left it to E19.

Running them on GitHub's runners found:

- **The scripts leaned on the author's `~/.gitconfig`.** Without a git
  identity, git-annex's own commits to the `git-annex` branch fail
  (`git commit-tree`: empty ident), so `wanted`, `group` and `maxsize` were
  never recorded: E14's custodians wanted nothing, E15's sync made no merge.
  E15 also assumed `init.defaultBranch = main`. E21 and E22 already isolated
  git; the others did not.
- **Homebrew's git-annex has no `git-remote-p2p-annex`.** The binary
  dispatches on its name (`CmdLine/Multicall.hs`), but Homebrew installs only
  `git-annex`, so `p2p-annex::` remotes (ADR 0006) fail on macOS:
  "git: 'remote-p2p-annex' is not a git command" (E12).
- **On macOS a write through a shared mapping leaves the stat unchanged.**
  The mtime moves at `msync`, not at the write (Linux moves it at the first
  write). git's refresh then calls the file clean though its content is not
  the index's key, before and without any install (E19, every boundary, thin
  and not thin). No save is lost: the writer scan sees the mapping, and an
  aside whose content is not the old key is kept (ADR 0040).
- **The macOS writer scan works through `libproc`**, as Apple's
  `proc_listpidspath` walks it: `PROC_PIDLISTFDS` and `PROC_PIDFDVNODEPATHINFO`
  (open flag `FWRITE`) for descriptors, `PROC_PIDREGIONPATHINFO2` for
  file-backed regions with `VM_PROT_WRITE`. The share mode does not tell a
  shared mapping from a private one: a shared mapping held by one process
  reports `SM_PRIVATE`. One scan costs about 30–50 ms there (about 1 ms on
  Linux), and the name is absent during an install for a median of 32–50 ms
  (0.5 ms on Linux).
- **A `kill -9` during `git sparse-checkout set` leaves
  `.git/info/sparse-checkout.lock`**, and every later free fails until it is
  removed (E15b, once kills landed early; 08 §8.5 removed only `index.lock`).
- **A P2P fetch can hang forever.** git-annex's relay can send `CONNECTDONE`
  before the last of git's output: in `runRelayService` (`P2P/IO.hs`) the
  service's exit and its last output race for one `MVar`, and when the exit
  wins the tail of `git upload-pack` is dropped (no pack after the final
  `ACK`) and the client waits for it forever. It hung E12 on CI and locally,
  and 2 of about 100 fetches in a loop with a concurrent notice. remotedaemon
  runs its fetch inside the notice loop without a deadline (`P2PGeneric.hs`),
  so one such fetch silences that peer's change notices until it restarts.
- E15b's per-key loops took 7 minutes on Linux and over 12 on macOS; git-annex's
  batch modes (`--batch-keys`, `lookupkey --batch`, `contentlocation --batch`)
  run the same steps in 31 s and 66 s.
- Python's `mmap` keeps a duplicate descriptor, so E19's mapping trials were
  found through the descriptor, not the mapping.

## Decision

1. **`lab.yml` runs every asserting experiment** on ubuntu-24.04 (the pinned
   standalone build) and macos-latest (Homebrew's git-annex), as a matrix of
   four groups in parallel: E20 E21 E22; E12–E13 E14 E23; E15 E15b E18; E19.
2. **Every script isolates git**: `GIT_CONFIG_GLOBAL=/dev/null`,
   `GIT_CONFIG_NOSYSTEM=1`, a lab identity, and `-b main` where the branch is
   named, as `pigeond` itself runs git (E20).
3. **A check that a platform cannot meet is not dropped.** The script probes
   the platform property itself, prints `SKIP (platform)` with the reason and
   the count, and asserts what still matters. The first case is E19 on macOS:
   for a write through a mapping, "git never calls a changed file clean" holds
   only once the stat changes. The script checks that the file's stat is the
   one git recorded (so git's stat shortcut, not the install, explains it),
   then changes the stat and requires git to see the change. "No save is
   lost" and the scan checks stay unchanged.
4. **E19's mappings keep no descriptor** (a `mmap` through `ctypes`), and E19
   first checks that the scan sees another process's write handle and
   shared mapping and ignores a read handle.
5. **The macOS writer scan counts every writable file-backed mapping**,
   private ones included: a false alarm only defers the install (07 §7.5).
6. **On macOS `pigeond` provides `git-remote-p2p-annex`**, a link to the
   `git-annex` it uses, in its own directory, first in the `PATH` of its
   children (14 §14.3).
7. **`pigeond` removes a stale `info/sparse-checkout.lock`** at start, as it
   removes `index.lock`, since it is the only writer of the repository
   (08 §8.5).
8. **Every P2P git fetch or push of `pigeond` has a deadline**, after which
   its process group is killed and the fetch waits for the next notice or
   period; a remotedaemon whose fetch began (`SYNCING`) and never ended
   (`DONESYNCING`) is restarted (08 §8.2). E12 does the same and prints each
   retry as a note, so a hang is shown, not hidden. The race is to be
   reported upstream, with its fix: drain the output before `CONNECTDONE`
   ([16 §16.4](../spec/16-open-questions.md)).

## Consequences

- The lab takes about 10 minutes of wall time per push, the time of E19 on
  macOS. Per job, on Linux and on macOS: E20–E22 1.5 and 2.5–3.5 minutes,
  E12–E14 with E23 2.5 and 3, E15–E18 1 and 2, E19 4.5–5.5 and 7.5–10. A
  failure names its platform and group.
- A new Homebrew release can break the macOS jobs before any pigeon change;
  the log records its version (14 §14.3).
- On macOS, capture does not see a save made only through a shared mapping
  until the application syncs it or the stat otherwise changes. Pigeon does
  not lose it (the aside rule of ADR 0040), but does not capture it either
  until then. Applications that write documents through mappings usually
  `msync` for durability; the limit is stated in 07 §7.5 and
  [16 §16.4](../spec/16-open-questions.md).
- An install on macOS keeps the name absent for tens of milliseconds, the
  cost of one scan; installs of a batch share one scan (07 §7.5).

## Alternatives considered

- *Skip E19's mapping trials on macOS.* Rejected: it would hide the platform
  difference that matters most to capture.
- *Run macOS weekly only.* Rejected: macOS was the gap, and each experiment
  takes minutes.
- *Set the git identity in the workflow.* Rejected: the scripts must run the
  same anywhere, as `pigeond` must.
- *Give E12's fetches a longer timeout.* Rejected: a fetch that loses its
  pack never ends, so a longer wait only delays the failure, and `pigeond`
  needs the same deadline anyway.
