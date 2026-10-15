# Report: trust and durability, v0.2

Milestone M1 of the [roadmap](../roadmap.md), built on the safety slice of
[slice-1.md](slice-1.md). This report is not normative: the specification
and the ADRs say what pigeon does.

## What was built

| Item | What it does | Commits |
|---|---|---|
| Signed commits (ADR 0007, 04 §4.4) | every capture and roster commit carries an SSHSIG signature of its device key; a custodian judges each commit a peer's `main` adds, oldest first, against its parent's roster, and accepts up to the first it refuses; the refusal is a problem code (`commit-unsigned`, `commit-forged`, `commit-unknown-signer`, `commit-not-allowed`, `commit-malformed`) | d3e6299, 343ba88 |
| `clock.json` (11 §11.3) | `heard` times and the Lamport clock of trace events survive a restart; a newer or wider file is kept as it is | 58d80f8 |
| Place in queue order (08 §8.9, #13) | a custodian admits keys in queue order while they fit above its reserve, defers the rest, says `no-capacity`, and takes them once room returns | 1b098be, 13188f8 |
| Problem catalogue (ADR 0023) | `docs/problems.md` lists every problem code with its message, hint and when it is raised | d639e40 |
| Status reports (06 §6.3) | each device publishes a signed `status.toml` at `refs/pigeon/status/<device>`; peers fetch, judge and relay them; a laptop shows a custodian's `disk-full`, `no-capacity` or lost repository | 775d5a1, f5dddd6 |
| Versioned formats (ADR 0048, #15) | the roster, status and clock schemas are closed: another `format`, an unknown key, role or folder rule reads as *unknown* and the commit waits (`commit-waits`); `fixtures/formats/` holds golden files and 18 signed commit cases | 59c4992 |
| pigeon's git-annex (ADR 0069) | `packaging/install-git-annex` installs the patched build (or the stock one where it does not run) beside pigeon's `bin/`, and `pigeond` puts it first in its children's `PATH` | b5f62c6, 4fc76c6 |

## How it is checked

- **Scenarios** (13 §13.12): S12, a custodian short of room, and S13, a
  commit signed outside the group, which both custodians refuse with every
  commit after it. Both pass on both runners in slice run 38665002273
  (905353b), with S1–S11 and S9's 50 random schedules.
- **Laws** (13 §13.13), as `hegeltest` properties: signatures (`tests/sig.rs`),
  the authority rule (`tests/authority.rs`), admission (`tests/capacity.rs`),
  which status report counts (`tests/report.rs`, five laws), and the closed
  roster (`pigeond/tests/codecs.rs`).
- **Traces against TLA+** (13 §13.3): admission plans against `Admission`
  (`AdmitTrace`), and, new in this milestone, the authority rule's verdicts
  against `Authority`'s `Judge` (`AuthorityTrace`): random histories of
  signed commits with versions, roster edits and Forces, 20 per push and
  200 in models-full. Each mutant, with its last verdict reversed, is
  rejected.
- **Format fixtures** (13 §13.6): `crates/pigeond/tests/formats.rs` reads
  `fixtures/formats/` in its own `ci` step on both runners.
- **The installer**: `git-annex.yml`'s installer job installs pigeon's
  build into a fresh prefix and starts `pigeond` from it, with the stock
  git-annex first in the `PATH` it inherits; its status report names
  `10.20261006-pigeon1` on both runners (run 38665002199).

## What M1 found

- A graceful stop killed the git that held `.git/index.lock`, and the stale
  lock refused captures for three minutes (S9 random, macOS). pigeond now
  removes git lock files left unwritten for 60 s before each round
  (cb2474f, 11 §11.4).
- A custodian that was joining reported its repository lost before its
  roster copy held its own entry (46430b1).
- `ssh-key` brought dependencies that `scripts/check-core-deps` did not
  allow yet (0af392d).
- The installer's stock path met a standalone build that reports its commit
  in its version (4fc76c6).

## Known gaps

- Laptops do not judge commits: custodians do, before they fast-forward,
  and a laptop takes their `main`.
- A recovery's roster commit is signed by the custodian, where ADR 0047 has
  the sheet key sign it (13 §13.12, 16 §16.3).
- An unknown commit waits as `commit-waits` in `pigeon doctor`; the
  *Needs you* headline `update-needed`, on the laptop, needs the custodians'
  reports to carry it, which is a new status format. Mixed releases in one
  group, and `peer-too-old` / `peer-too-new` on ALPN, are not tested yet
  (#15 stays open for them).
- Merges and proposals wait as unknown until the catalogue merge (M2).
- Introduction times are not read yet (every key counts as introduced at
  0), so among keys with as many copies, admission skips the newest-first
  rule of 08 §8.9 and takes the smaller first.

## Release

v0.2.0 is the commit that adds this report, tagged once ci, lab, slice and
git-annex pass on it; v0.1.0 is 905353b.

## Next

Milestone M2: the install journal (#1–#3), the catalogue merge (#8) with
`conflicts.json`, invitations, roster forks (#6) and stewards (#7).
