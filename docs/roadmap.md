# Roadmap to v1.0

v1.0 meets every accepted ADR, on Linux and macOS laptops and Linux backup
machines (ADRs 0005, 0032, [13 §13.12](spec/13-verification.md)). This file
orders the work and says when each part counts as done; what each part does
is in the [specification](spec/) and the [ADRs](adr/). It is not normative.

The order follows one rule: first what keeps data safe, then what three
people need to use pigeon on their own machines, then what people who did
not build it need, then deletion, then polish (Guillaume, 15 October:
the interface before purge and scrub).

## How a milestone ends

- Its exit checks pass in CI on `ubuntu-24.04` and `macos-latest` (ADR 0026),
  and its scenarios have left `slice::BLOCKED`.
- Its laws run as property tests and its traces are checked against the TLA+
  modules (ADRs 0008, 0012).
- It is tagged `v0.N.0` and reported in `docs/reports/`.
- A command it does not cover still answers `not-in-this-build`.

## M0. The safety slice: v0.1

One person, one laptop, two custodians on a LAN, K = 2 (issue #17).
Exit: S1–S11 pass on both runners; `docs/reports/slice-1.md`. Done:
v0.1.0, at 905353b.

## M1. Trust and durability: v0.2

- Roster and capture commits are signed and checked on accept (ADR 0007,
  [04 §4.4](spec/04-roster-and-authority.md)).
- `heard` times and the trace clock are kept on disk in `clock.json`
  ([11](spec/11-local-state.md)).
- Place follows the queue order, with admission and reserved space
  ([08 §8.9](spec/08-custody-loops.md), issue #13).
- The problem catalogue `docs/problems.md` (ADR 0023) and the
  `refs/pigeon/status` reports ([06](spec/06-status.md)) exist.
- Replicated formats carry versions and have fixtures from this release on
  (ADR 0048, [13 §13.6](spec/13-verification.md), issue #15), so that later
  upgrades keep data.
- pigeon runs a pinned git-annex carrying the fixes for the P2P removal lock
  and the relay race (`third_party/git-annex/`). ADR 0069, which records
  this decision, supersedes ADR 0020's official builds. Its build and its
  proof run in CI (`git-annex.yml`), and `packaging/install-git-annex`
  installs it beside pigeon's `bin/`, checked end to end on both runners.

Exit:

- an unsigned or forged commit is refused (a new scenario);
- the format fixtures are checked in CI;
- the slice is still green.

Report: `docs/reports/trust-2.md`.

## M2. Several laptops and several people: v0.3

- Laptops install incoming versions through the install journal
  ([07 §7.5](spec/07-capture-and-install.md), issues #1, #2, #3).
- The catalogue merge follows its laws ([05](spec/05-catalogue-and-names.md),
  issue #8). Conflicts are written to `conflicts.json` (ADRs 0034, 0035).
- People are invited, and proposals are refs (ADRs 0009, 0017).
- Roster forks merge (ADR 0051, issue #6).
- Stewards decide suggestions (issue #7), and add-only folders work.

Exit:

- scenarios with two people and three laptops pass;
- concurrent edits become conflicts and never overwrite (ADR 0034);
- concurrent roster changes converge;
- the merge traces validate against TLA+.

## M3. Reach and installation: v0.4, the dogfood alpha

- Relays and n0 discovery are on by default; an own relay is optional
  (ADR 0016).
- Packages ([14 §14.4](spec/14-platforms-and-packaging.md),
  [14 §14.5](spec/14-platforms-and-packaging.md)):
  - for people on Linux, a tarball, an install script and a systemd user
    unit;
  - for backup machines, a tarball and a system unit;
  - on macOS, a Homebrew tap and a launchd agent.
- A release workflow builds from a tag, with SHA-256 checksums.

Exit:

- a scenario between the two runners passes over a relay;
- install and uninstall are tested in CI;
- Mario, Guillaume and Laurent run pigeon on their own machines.

## M4. For people: v0.5

- `pigeon-web` (ADR 0021) shows plain words and one headline status
  (ADR 0027).
- Notifications and a launcher (ADR 0029).
- The French bundle (ADR 0028).
- Telemetry to an optional collector, and the two dashboards (ADR 0025).
- The remaining commands but purge and erase (M5): none other answers
  `not-in-this-build` any more.
- Recovery is complete, including refounding and export without pigeon
  (ADR 0047, [09 §9.9](spec/09-network-and-joining.md)).

Exit: a usability test with people who did not build pigeon, which sets the
thresholds of [06 §6.7](spec/06-status.md).

## M5. Deletion and scrub: v0.6

- Purge is reference-aware and handles restores and returning peers
  ([08 §8.6](spec/08-custody-loops.md), ADR 0018, issues #4, #5). Erase
  ships with it, and no command answers `not-in-this-build` any more.
- The scrub runs every 30 days in the idle I/O class
  ([08 §8.8](spec/08-custody-loops.md)).
- `Purge` traces are validated ([13 §13.10](spec/13-verification.md)).

Exit: the purge laws hold, the model checks, and scenarios of a purge racing
a restore and of an offline peer returning pass.

## M6. Resources and content: v0.7

- Adaptive limits, priorities and settings (ADR 0037,
  [15](spec/15-resource-use.md)).
- Large and hot files, within the budgets of
  [15 §15.7](spec/15-resource-use.md).
- The other file systems ([14 §14.2](spec/14-platforms-and-packaging.md),
  issue #14).

Exit: the budgets are measured in CI.

## M7. Release candidate, then v1.0

- A conformance table maps each accepted ADR to the code and the test that
  shows it.
- Every planned law and trace validation is done
  ([13 §13.10](spec/13-verification.md), issues #9, #10).
- models-full stays green on both runners for seven nights in a row.
- The lab passes a 72-hour soak.
- Every v0.N upgrades with its data (ADR 0048).
- Joining and signing get a security review (ADRs 0007, 0017).
- The user guide, man pages and completions are written.
- `v1.0.0-rc.1` is tagged. `v1.0.0` follows after two weeks of dogfood with
  no data loss and no unexplained *Needs you*.

## After v1.0

These come in v1.1:

- native Windows (ADR 0005);
- the status icon (ADR 0029);
- macOS backup machines and NAS packages (ADR 0032);
- phones as viewers;
- short codes with a PAKE (ADR 0017);
- a placement planner (ADR 0003);
- per-file freeing (ADR 0031).

## Decisions people owe

These questions of [16 §16.4](spec/16-open-questions.md) block a milestone:

- rename tracking blocks M2 (own files in add-only folders: ADR 0070;
  saves from stale memory: ADR 0071);
- the status thresholds block M4;
- expiring declined suggestions blocks M5.
