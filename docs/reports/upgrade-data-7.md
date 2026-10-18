# M7: native historical-data upgrade checkpoint

> Mission: test real old-program data, with exact program identities and byte proofs.

This checkpoint does not close the complete upgrade matrix. It exercises one
Linux laboratory laptop, two paths of different sizes, retained historical
objects, subsequent capture and historical restore. The mixed-release
checkpoint below extends this to a laptop and backup machine; every published
minor, original release archives and final release-candidate qualification
remain separate work.

## Format retirement is explicit

[ADR 0075](../adr/0075-one-invitation-per-group-shared-as-a-line.md) says:
“No group older than this decision is supported: none was in use.” It
retires the format-1 roster, rather than authorizing a reader to reinterpret
it as format 2. Source inspection records format 1 at v0.1.0 (`905353b`),
v0.2.0 (`d6c02d9`), v0.3.0 (`74d55da`), v0.4.0 (`fdf6e0f`) and
v0.5.0 (`d8ede32`). That inventory is not a native upgrade result for each.
The first published v0.5 minor, v0.5.13, uses format 2, as does v0.6.1.

The retired-format case requires *Needs you*, unchanged original catalogue
and working bytes, and independent hashes of every retained object. It
reports `retired-format-preserved; not an automatic upgrade`. Only the
supported-format case can report `supported-format-upgrade-passed`.

## Reproducible procedure

`scripts/lab-soak prepare --checkout OLD_CHECKOUT --bin NEW_DIRECTORY`
builds a clean historical checkout without editing it. The current tool can
therefore freeze old sources that predate the runner itself. It records the
Git source, lab feature and SHA-256 of all three programs. Relative Cargo
target paths are resolved against the selected checkout. A changed source,
dirty checkout or changed program invalidates its receipt.

`scripts/check-upgrade` takes two frozen binary directories and a new root
and report. It first uses the old daemon and CLI to create two files and
capture a second notes version. Both pointer trees and every retained object
are independently checked before stopping the old daemon. A private original
repository snapshot keeps durable files and symlinks, omitting ephemeral
Unix sockets only. The new daemon starts on the same configuration and data.
Working bytes, modes, the original accepted head and retained hashes must
remain correct. Supported cases then capture a new save and restore version
1 through the normal CLI, proving current pointers and all four retained
objects again. The executable photo mode is checked on supported releases.
The retired v0.1 baseline uses ordinary files that it can actually capture.

```sh
scripts/lab-soak prepare --checkout /absolute/old-checkout --bin /absolute/old-bin
scripts/lab-soak prepare --checkout /absolute/current-checkout --bin /absolute/new-bin
scripts/check-upgrade --old-bin /absolute/old-bin --new-bin /absolute/new-bin \
  --root /absolute/new-fixture --report /absolute/new-report.json
```

Use `--retired-format` only for the explicitly retired format 1. A different
format fails the requested check. Existing roots and reports are refused;
interruptions fail and stop the peers. Reports include the harness and
observer checksums, exact build manifests and the distinct verdict. These
are finite lab builds, not elapsed soak or resource-budget evidence.

## Original observations and fixture corrections

Initial manual pilots on v0.5.13 and v0.6.1 data preserve their old heads,
working files and all three original objects with target lab source
`265b3c34474d44c1a72774c7d30c9bfff201252c`. Both then capture a new save,
restore the first notes version and independently verify all four retained
objects. A separate v0.1 pilot preserves its three objects and reports
*Needs you*. These precede the reusable check and its executable-mode cases.

The first repository-snapshot attempt stops before upgrade because it tries
to copy an ephemeral annex Unix socket as an ordinary file. The snapshot
now excludes only sockets. Another pilot mistakenly gives a commit hash to
restore's time option; the corrected fixture uses an absolute file path and
`--version 1`. These are fixture corrections, not product failures.

The first reusable retired-format case seeds an executable probe that v0.1
records as 100644. Current capture observes that old pending mode change,
so the original-head assertion fails; the save helper also inherits a
restrictive umask. All three content hashes remain correct. Its private
archive is 78,704 bytes, SHA-256
`20b5674749f79a0fae10956b25c02b6baa609f8f7729c8546b981b144fddc1a1`.
The corrected retired baseline uses ordinary modes; supported cases retain
the executable probe. Atomic saves set their intended permissions explicitly,
and the old baseline's working bytes and catalogue modes must agree before
an upgrade is attempted. No original observation is overwritten.

## Current-source validation

The reusable cases target frozen lab source
`080a1b78274de8a9f6c99bc6ade5f564bd228b0d` (v0.6.35).
All seven native cases pass with the same reusable harness and observer:

| Old tag | Exact source | Format | Result | Retained object proofs |
| --- | --- | --- | --- | --- |
| v0.1.0 | `905353b368e7a91096225eb67509dc68bbe9ac2c` | 1 | preserved; Needs you | 3 |
| v0.2.0 | `d6c02d90e01e76bb11d218df2d581c083bf25d53` | 1 | preserved; Needs you | 3 |
| v0.3.0 | `74d55da137525cd88f9334b42a089d6c1f940615` | 1 | preserved; Needs you | 3 |
| v0.4.0 | `fdf6e0fb41ed611f9db0b44ea0c51c1d72c36f3f` | 1 | preserved; Needs you | 3 |
| v0.5.0 | `d8ede326c86c9140420a6b7a833fe63457346766` | 1 | preserved; Needs you | 3 |
| v0.5.13 | `2101edbacace261bd932bfa52e1514faf402329d` | 2 | new save and oldest restore pass | 4 |
| v0.6.1 | `be2e4ffa3be8e651becf711842f460a0ff15aac2` | 2 | new save and oldest restore pass | 4 |

Their harness SHA-256 is
`e639897b590d5f10fc0758453a256fbcaf3c959afc8661b5e222de464355eee5`.
The final preparer uses Cargo-reported executable paths, including configured
target directories. Its actual v0.6.35 preparation produces byte-identical
checksums for all three programs already exercised above; default-directory
artifacts cannot replace the selected build. These observations use rebuilt
historical Rust programs with the current pinned git-annex, rather than
executing each original release archive. The complete upgrade gate remains open.

All 139 Python contracts pass, including real Git pointer trees, different
object sizes, missing historical objects, same-size historical corruption,
executable-mode disagreement, frozen-program checksum drift and a restrictive
umask. CI selection uses an exact list for standalone evidence tools whose
Python contracts always run in repository rules. Mixed Rust changes retain
runtime jobs; workflow/selector changes and all non-PR events remain full.
The selector change itself requires full PR validation. This does not count
as a native measurement or complete the every-minor upgrade gate.

## Integrated release validation

PR #105, head `16ca8204b0ae3bc7cb54cdb52771b2594078af3d`, integrates current main `080a1b78274de8a9f6c99bc6ade5f564bd228b0d`
before validation. CI 39121096409, lab 39121096339 and bridge
39121096388 pass. v0.6.35 is published and independently
verified before this candidate. Local validation passes 139 script contracts,
seven native historical-data checks with exact frozen programs, zero-warning
all-target workspace Clippy and repository checks. The final preparer returns
byte-identical target programs using Cargo-reported artifact paths. Fresh
v0.6.36 tag gates and independent archive/install checks remain required.
The broader upgrade/security audit and actual resource/elapsed gates stay open.

## Mixed-release laptop and backup checkpoint

Frozen v0.6.41 lab source `c08b5274cc99a5e89fee4c7bf2c952ce69f159a5`
adds two-peer checks against old supported-format sources. Four initial
controlled probes pass: v0.5.13 and v0.6.1, with either the laptop or the
backup machine upgraded first. The reusable `scripts/check-upgrade-peers`
then runs the same matrix with explicit identity validation and recorded
harness/observer hashes. All four matching native cases pass, each proving
four retained objects on both peers. All 153 script contracts pass, including
identity ambiguity, invitation omission, optimized-Python safety checks,
original-report preservation and cleanup-failure verdicts.

Each case creates its group with the old programs, joins an old backup and
proves two different-sized files, executable modes and a second notes
version on both peers. It stops just one peer, keeps a durable original
repository snapshot, starts the new program on its existing home/data,
and checks the original accepted head, public device/person/role bindings
and annex UUID. A new save must cross the mixed-release pair before the
remaining peer upgrades. Both upgraded peers then retain all four unique
objects and restore the oldest notes version through the normal CLI.
Every current pointer, retained object and working byte/mode is independently
checked; no custody claim is substituted for those proofs.

```sh
scripts/check-upgrade-peers --old-bin /absolute/old-bin --new-bin /absolute/new-bin \
  --root /absolute/new-fixture --report /absolute/new-report.json --first backup
```

Use `--first laptop` for the opposite order. Both binary directories require
unchanged frozen lab manifests. Existing roots/reports are refused. Retired
format 1 is not reinterpreted as supported. Public identity receipts omit
the invitation. Interrupted runs and cleanup failures cannot leave a passing
verdict. Each fixture has two files and four retained versions, using rebuilt
historical programs with the current pinned annex. These results do not cover
every published patch, actual old release archives, status-quorum semantics,
all mixed-version operations, other platforms or the final RC. Resource and
real elapsed gates remain open.
