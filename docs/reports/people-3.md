# Report: several laptops and several people, v0.3

Milestone M2 of the [roadmap](../roadmap.md), built on M1's
[trust-2.md](trust-2.md). This report is not normative: the specification
and the ADRs say what pigeon does.

## What was built

| Item | What it does | Commits |
|---|---|---|
| The catalogue merge (05 §5.2–5.4, #8) | `pigeon-core::catalogue::Engine` builds the catalogue of any set of accepted commits from contributions and the conflict events they froze: heads, versions, representatives, placement under portable conflict names, freezing | ee2e16f, 67ed889 |
| Merges judged and written (04 §4.4) | accept judges histories with merges, oldest first; custodians and laptops write the merge of two accepted heads as `catalogue(anc*)` | abfd6eb |
| Install (07 §7.5, #1–#3) | laptops accept like custodians and move each clean path to `main`'s version through the install journal; a save racing an install is a concurrent edit, kept in `conflicts.json` | a050bc3, 445956c |
| Invitations (09 §9.6) | laptops of a person of the roster, and new people, join by a one-time invitation; `pigeon join --name` names the computer | 1ae4fe0, 2afccf6 |
| Suggestions (04 §4.5, 07 §7.3, #7) | a change its person may not make becomes a proposal, kept as a ref and relayed by every device; deciders accept it (rule 7) or decline it (a decision commit); the author keeps their file as a divergence | 273b928, 7ef8b07, 230dae4, 343843e |
| Roster forks (04 §4.8, ADR 0051, #6) | a merge of different rosters carries their fold | 60aaf6f |
| Divergences (07 §7.6, ADR 0034) | `pigeon conflicts` lists them; `pigeon resolve` takes the group's version (withdrawing a pending proposal), keeps the person's as a copy, or suggests it again | 673312f |
| The judge for any parents (04 §4.4) | `pigeon-core::commit::judge` applies rules 2–7 to a commit of any number of parents, with its adoptions and decisions | 3dafe7d |
| Own files in add-only folders (ADR 0070) | a member may change an add-only file that only they wrote: the catalogue gives each slot's writers, and the authority rule, `Authority.tla` and S14 use them | 4c7e0d5 |
| Saves from stale memory (ADR 0071) | a save within ten seconds of an install of its path goes beside the installed version, under its conflict name, and the laptop's file diverges | bcf41cf |
| Renames (ADR 0072) | capture pairs a deleted file with a new one of the same content, else of similar content up to 1 MiB, and writes a `Pigeon-Renamed:` trailer; the merge moves a concurrent edit to the new name, and a mere move yields to it | ea51c42, 97231d6, 2ef9b30 |
| Trying it | `main` holds v2; the README tells how to install pigeon from source and try it, and `scripts/try-pigeon` runs two laptops and a backup machine on one computer; an invitation and a join say whether a computer or a backup machine joins | afe02cb, e0ba127, 2c0eff6 |

## How it is checked

- **Scenarios** (13 §13.12): S14 (two people, three laptops: files flow,
  a suggestion is refused to its author and accepted by the admin), S15 (a
  declined suggestion stays its author's; edits made apart keep both
  versions, neither laptop's file overwritten) and S16 (two laptops each
  admit a person while apart; the roster forks and converges). They run
  with S1–S13 in `slice.yml` on ubuntu-24.04 and macos-latest.
- **Laws** (13 §13.13), as `hegeltest` properties: the catalogue merge
  (`tests/catalogue.rs`: order, tree, kept, seen, names, authority, with
  judging in another order), proposals and adoptions (`tests/proposal.rs`),
  the authority rule with decisions (`tests/authority.rs`), the roster fold
  (`tests/roster.rs`), the rename trailer (`tests/authority.rs`);
  `models/catalogue/catalogue.py` checks every schedule of 22 cases, 12
  of them renames, and `tests/catalogue_oracle.rs` judges every commit
  the oracle reaches alike.
- **Traces against TLA+** (13 §13.3): the catalogue's verdicts, views and
  open conflicts against `Catalogue`'s `Record` (`CatalogueTrace`), with
  renames with and without trailers since ADR 0072 (a model that does not
  follow them rejects 5 histories of 20, one that does not yield 4 of 80),
  and
  the authority rule's verdicts, now with merges, adoptions and decisions,
  against `Authority`'s `Judge` (`AuthorityTrace`): 20 histories per push,
  200 in models-full, each mutant rejected.
- **pigeond's tests**: `people.rs` (two people, three laptops, a
  concurrent edit), `suggestions.rs`, `resolve.rs`, `install.rs`,
  `merge.rs` (a rename and a concurrent edit; the capture of renames).

## What M2 found

- TLC rejected 2 of models-full's 200 catalogue histories: the engine
  showed a version twice, at the copy a freeze gave it and again under a
  second name, once a copy was edited or a re-frozen side met the other.
  The fix is in the code, the oracle and `Catalogue.tla` (67ed889).
- The model was refined where the code was right: a conflict copy is a new
  file, a decision is never changed, an admin may force an adoption, a
  deletion is held unless the base changed the path (3dafe7d).
- A branch sees only its own caches and those of the default branch, so
  on `m2` CI ran the stock git-annex, whose P2P relay can end a session
  before its last data (macOS S9 timed out); `git-annex.yml` now runs there
  too, and pigeon's build (patch 0002) is used.
- S15 once read a file as absent on macOS: re-thinning, like any install,
  moves the file aside for an instant (07 §7.5); the lab now reads a
  missing file again for up to a second (a51bd58). The instant lasted
  about 1.2 s on the macOS runner, the time of the writer scan, which
  walked every memory region of every process; it now asks only for the
  regions backed by a file (`PROC_PIDREGIONPATHINFO2`, 7930f9c).
- A step that failed left its transition half done, as a crash does, but
  nothing recovered it before capture. On macOS an S9 schedule left an
  `index.lock` behind a crash; `free` then moved two files aside and
  stopped before their skip-worktree bits, and capture, once the lock was
  stale and removed, committed them as deleted. The round now recovers the
  journal whenever an entry has no end, and capture, install, free, keep
  and re-thinning wait meanwhile, the order `CaptureInstall.tla` checks
  (0225589, `pigeond/tests/unsettled.rs`, 07 §7.5).
- S11 once found the laptop silent for two minutes on macOS, with nothing
  logged; a git call still running after 20 seconds is now named in the
  log (`git-slow`, 727d259).
- Trying pigeon outside the lab found that `pigeon` looked for the
  daemon's socket under `$TMPDIR` on macOS, while `pigeond` puts it in
  `~/Library/Caches/pigeon/run` (11 §11.2): the lab and the tests give
  both `PIGEON_SOCKET`. The CLI and the helper now take the path from
  `pigeon-sys`, as the daemon does (ab596eb, 09 §9.3).
- Following renames needed four rules found by the oracle's schedules:
  a write follows a rename only when both build on one version; an event
  replaces what it saw where its own view put it; a pure rename yields to
  an edit that saw what it moved; a conflict event sees the renames that
  moved its heads (05 §5.2).

## Known gaps

- The model accepts adopting any refused commit; the code adopts a commit
  refused for folder rules alone, and lets others wait as unknown.
- A refused commit on a peer's `main` is not kept as a proposal (08 §8.2).
- A skipped roster change is only logged; revoked commits are not skipped.
- *Keep mine*, *Keep Paul's*, *Keep both* and `pigeon view` are not in it.
- The lab's Roster traces are not checked by TLC: pigeond logs a commit ID
  where `RosterTrace` reads the roster.
- ADR 0071 is checked by pigeond's tests and S14–S15, not by
  `CaptureInstall.tla`.
- Two identical renames made concurrently can leave an extra copy; a
  version moved into an add-only folder makes its author a writer there;
  a rename with a large edit, or of a file beyond 1 MiB, is still a
  deletion and an add (ADR 0072).
- The trace generator never reaches a conflict event that saw renames;
  the oracle's cases and the hegel laws do.

## Release

v0.3.0 is the commit that adds this report, tagged once ci, lab, slice and
models-full pass on it; v0.2.0 is d6c02d9. It also carries M3's first
steps: n0's relays and address lookup are on by default (ADR 0016,
c2944b6), while the lab and the tests stay on their own network, and
`reach.yml` runs a group between the Linux and the macOS runner over them
(f815360), which passed at its first run (38750098867).

## Next

Milestone M3: relays and discovery, packages, a release workflow, and
dogfood on the members' machines.
