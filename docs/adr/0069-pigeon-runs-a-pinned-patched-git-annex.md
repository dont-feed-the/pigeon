# 0069. pigeon runs a pinned, patched git-annex, built by its own CI

- **Status:** Accepted, 14 October 2026, by Guillaume (chat: "fork
  git-annex to fix the bugs; we can report upstream later"). Supersedes
  ADR 0020.

## Context

ADR 0020 took git-annex from its official builds: the standalone build on
Linux, Homebrew on macOS. pigeon's experiments then found two bugs in the
server side of git-annex's P2P protocol, which pigeon's peers talk to:

- **E23:** `CHECKPRESENT` answers with `inAnnex` (`P2P/Annex.hs`, line
  129), which ignores a removal lock. Two custodians that drop one key at
  the same time each count the other's copy, which leaves fewer than K
  copies, or none.
- **E12, E25:** `runRelayService` (`P2P/IO.hs`, line 398) can send
  `CONNECTDONE` before the end of git's output. The fetch then hangs, and a
  hung fetch inside `remotedaemon` silences that peer's change notices.

pigeon works around both: its serving filter answers `CHECKPRESENT` by
taking `LOCKCONTENT` (09 §9.4), every P2P fetch and push has a deadline,
and a stuck `remotedaemon` is restarted (16 §16.4). The workarounds cover
only the paths pigeon thought of, cost a round trip per check and a
restart per hang, and every new use of git-annex's P2P protocol must
remember them. Neither bug is reported upstream yet, and a fix there would
take a release cycle.

The alternative is to keep the stock builds and the workarounds alone. It
is simpler to ship, but leaves the bugs in what pigeon runs.

## Decision

- pigeon runs the git-annex release pinned in `third_party/git-annex/VERSION`
  (10.20261006), fetched from Hackage at a pinned sha256, with the patches
  in `third_party/git-annex/patches/`:
  - 0001 answers `CHECKPRESENT` as `inAnnexSafe` does, so a copy locked
    for removal is absent;
  - 0002 sends `CONNECTDONE` once, after all of git's output;
  - a last patch marks the version `10.20261006-pigeon1`.
- pigeon's CI builds it (`git-annex.yml`) on the runners that stand for
  pigeon's platforms. CI proves each patch against the stock build (E23,
  E25), and runs the slice on pigeon's build.
- Installers put pigeon's build in pigeon's own directory, first in the
  `PATH` of `pigeond`'s children:
  - on Linux, with the system's git (ADR 0010's 2.38 or later), since the
    standalone build's `git` puts its own git-annex first; the C library
    pigeon's build needs (glibc 2.39) comes with a git new enough;
  - on macOS, instead of Homebrew's git-annex.
- Where no build of pigeon's exists yet (14 §14.3), `pigeond` runs the
  stock git-annex at its minimum version. The workarounds stay, so the
  stock build remains safe. `pigeond` still checks versions at start-up,
  as ADR 0020 decided.
- 0001 and 0002 go upstream with their reproducers. A patch is dropped at
  the first pin of a release that contains its fix.

## Consequences

- Moving to a new git-annex release means refreshing the patches and a
  cold CI build, about an hour, besides the contract tests of ADR 0004.
- pigeon distributes a modified AGPL program. The patches carry git-annex's
  licence, and each release names the tarball and patches it was built
  from and keeps them available. `git annex version` names the build, and
  so does `pigeon status` (06 §6.3).
- Two git-annex builds stay in use until every platform has pigeon's.
  CI's negative control keeps asserting the stock bugs, which tells when
  the workarounds can go.
- pigeon's macOS build of git-annex is signed and notarised with pigeon's
  binaries (14 §14.5).
