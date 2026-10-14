# 0057. The first milestone is a safety slice: one laptop, two custodians, no proposals or purge

- **Status:** Proposed, 14 October 2026, by the agent, for issue #17
  (Guillaume's review: "a small runnable vertical slice would make that study
  more concrete and concentrate effort on the behaviors that carry the safety
  promises").

## Context

The proposal (§7) asks for a two-week feasibility study; its first week
implements the roles and the daemon loops and runs fault injection, and
[13 §13.11](../spec/13-verification.md) states when v1 goes on. Most of what
carries the promises is a Proposed ADR (0039–0050) backed by git-annex
experiments, not by pigeon code: no part of pigeon has yet seen a file move
from *local* to *protected*. The specification also describes far more than
the promises need: proposals, purge, a web interface, telemetry over iroh,
dashboards, notifications, two languages and installers.

Issue #17 asks for a first runnable slice with an explicit scope, a
reproducible setup, a list of faults to exercise, disk-use figures and a
record of which promises it establishes.

## Decision

**The milestone.** The first code of v2 is the *safety slice*. Nothing
outside it is built until it passes; the deferrals below are about the order
of work, not about v1, which still meets every accepted ADR.

**What the slice has.** One group with one person, who is its admin: one
laptop (Linux on ext4, or macOS on APFS) and two Linux custodians on ext4,
K = 2 (ADR 0002). It has one repository and two folders. The crates of
[14 §14.6](../spec/14-platforms-and-packaging.md) that the slice needs are
`pigeon-core`, `pigeon-proto`, `pigeon-i18n`, `pigeon-git`, `pigeon-sys`,
`pigeond`, `pigeon` and `git-annex-p2p-iroh`. The loops of
[08 §8.1](../spec/08-custody-loops.md) that it runs are capture, accept,
install, place, repair (started by a removed custodian), free and keep, and
the settings check. Every device serves its peers through `pigeond`'s filter,
with the `CHECKPRESENT` probe on custodians (ADRs 0043, 0054), and install
keeps its journal (ADR 0055). The only commands are the following, each with `--json`
(ADR 0024):

- `create`, `invite --backup-machine`, `join --backup-machine`;
- `status [PATH] [--not-shared]`, `machines`, `history`, `restore`;
- `free`, `keep`, `remove-computer`;
- `recovery-sheet`, `recover`, `doctor`, `debug bundle`.

Messages come from the English Fluent bundle only (14 §14.8); the French
bundle and ADR 0028's completeness test come after the slice. Its diagnostics are the JSON logs of ADR 0022 and the trace logs of
ADR 0008.

**Supported content.** Regular files and folders, with the executable bit
(ADR 0044), and portable names (04 §4.4 rule 5). Every other kind is listed
as not shared, as ADR 0044 says. In the slice build, `create` and `join`
refuse any other file system and name the one they found: btrfs, XFS, ZFS,
exFAT, network shares and removable drives come after the slice.

**Deferred until the slice passes.**

- More people and more laptops: invitations of people, proposals and their
  refs (ADR 0009), conflicts between devices (ADR 0034), stewards and
  decisions (ADR 0038).
- Purge, erase and drop (ADR 0042, 08 §8.6–8.7), and the scrub.
- The local web interface (ADR 0021), notifications (ADR 0029), and the
  telemetry transport and its dashboards (ADR 0025).
- Packaging, installers and service units (ADR 0011, 14 §14.5).
- Relays and discovery beyond direct addresses and mDNS (ADR 0016).
- Adaptive limits, battery and metered probes (ADR 0037). The slice keeps
  only low CPU and I/O priority.
- The tuning of large and hot files (15 §15.5), and Windows (ADR 0005).

A deferred command answers the problem `not-in-this-build`; the slice holds
no partial code for it.

**The setup.** `lab/slice/` holds the harness and its README. A lab build
(`cargo build --features lab`) adds exactly two seams to `pigeon-sys`:

1. failure-domain fingerprints read from a file, since ADR 0049 rightly joins
   every repository of one host into one domain, and one host could never
   reach *protected*;
2. named crash points, so that `PIGEON_LAB_CRASH=point:n` aborts the process
   at the n-th crossing, as `kill -9` would.

A lab build says so in `doctor`, and marks its status reports
`lab = true`; a release build never counts such a report (ADR 0048). On one
Linux host the harness runs the laptop, and each custodian in its own mount
namespace with its own loop-mounted ext4 image. They connect through iroh
endpoints on 127.0.0.1, with direct addresses only. The **three-machine
run** uses release builds and real fingerprints on real hardware: a laptop,
once on Linux and once on macOS, and two Linux custodians on two machines.
Its procedure is in `lab/slice/README.md`.

**Scenarios.** Each is an asserting script in `lab/slice/`. Each writes a
trace log validated against the models that cover the steps it logs
(13 §13.3), and a disk-use series.

| Scenario | Shows | Criterion of #17 |
|---|---|---|
| S1 | a saved file moves from *local* to *published* to *protected*, each step caused by a recorded observation (ADR 0046); a symbolic link is listed as not shared | states from actual custody observations |
| S2 | `free` refuses while a version in the folder is not protected, then frees without a pointer file (ADR 0039) | refuse eviction before protection |
| S3 | `keep` and `restore` bring content back, byte for byte (SHA-256) | restore with matching content |
| S4 | one custodian offline: *published*, never *protected*; protected without action when it returns | offline custodian |
| S5 | a 1 GiB transfer killed on either side resumes; no partial object is counted | interrupted transfer |
| S6 | a save racing `keep`'s install wins (ADR 0040, as E19) | save during installation |
| S7 | a custodian's image destroyed: status falls to not safe; `remove-computer`, a new custodian joins, and repair restores K in ADR 0050's order | custodian loss |
| S8 | a full custodian image: admission refuses and says why (ADR 0050); a full laptop: a restore refuses before it starts and leaves no partial file | disk full |
| S9 | `pigeond` crashed at every named point of capture, install, place and free, then restarted: nothing lost or duplicated, and the states are recomputed | daemon restart |
| S10 | a fresh laptop recovers the group from the two custodians with the recovery sheet (ADR 0047); the keys, metadata and authority it needed are listed | recovery on a fresh machine |
| S11 | the steady and peak disk use of each device: the folders, `.git`, the annex objects, `.git/annex/tmp`, pending history (ADR 0041), pigeon's state and temporary files | disk use |

S9 runs 50 random crash schedules on each push and 1000 in `models-full`
(the ≥ 1000 of proposal §7).

**Laws first.** The parts of `pigeon-core` that the slice uses have their
laws (hegel-rust) before their drivers: states and headline, domain counting,
the install journal, the queue order and the roster fold.

**Passing.** The slice passes when, on Linux CI and in the three-machine
run, every scenario asserts, and no protected version is lost in any
schedule. Laptop disk use in the steady state must be at most 1.1 × the
content kept (13 §13.11); the peak is reported with its causes.

**The ledger.** `docs/reports/slice-1.md` lists each promise (P1–P6, 13
§13.10) and each Proposed ADR that the slice exercises. Each is marked
*established* (with the scenarios and validated traces that show it),
*conditional* (with the assumption it rests on), or *untested*. Evidence
from pigeon's own code enters the "now" column of 13 §13.10 only through
this ledger. Issue #17 closes when the ledger is published with the slice
passing.

## Consequences

- The work goes first to the code that carries the promises. Proposed ADRs
  0039–0050 get evidence from pigeon itself, not only from git-annex.
- The lab seams mean that Linux CI never sees real fingerprints reach
  *protected*. E22 and the three-machine run cover that; the seams are two,
  each named and absent from release builds.
- One host does not show real network faults beyond loopback: an offline
  custodian is a stopped endpoint. NAT, relays and Wi-Fi drops appear only in
  the three-machine run.
- Authority code runs with one admin only. Roster forks (ADR 0051) and
  decisions (ADR 0038) stay checked by the models and laws alone until the
  next slice.
- The slice is no release: the absent French messages, web interface and
  installers are deliberate, and the ledger says so.
