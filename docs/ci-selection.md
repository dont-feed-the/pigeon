# Pull-request CI selection

Pull requests run the checks affected by their complete Git diff. Repository
rules and the CI selector run every time. Each selector job writes a table
of its decisions to the workflow summary. Stable **CI selection verified**,
**Lab selection verified** and **Bridge selection verified** checks require
every selected job in their workflow to succeed; an intentional exclusion
can be skipped. Failed, cancelled, missing or unexpectedly skipped selected
checks cannot pass them.

Existing main, release-tag, manual and scheduled workflows select all their
job groups. Periodic macOS validation explicitly requests full selection.
The release publication gate still requires every named Linux job to pass;
none of its required checks was weakened or made optional.

| PR changes | Checks selected, in addition to repository rules |
| --- | --- |
| Root README, handoff, changelog or Markdown under `docs/` | No expensive jobs |
| Installer, service scripts, packaging tests or Homebrew formula | Packaging |
| Formal models, configurations or proof tooling | Proofs, fast models and core/daemon/bridge trace validation |
| Bridge protocol models or their trace checkers | Their formal/trace checks and the real multi-peer Nextcloud integration |
| Bridge crate | Rust workspace, real Nextcloud integration and bridge traces |
| Other existing Rust crates | Rust workspace, daemon shards, core/daemon/bridge traces and real Nextcloud integration |
| Patched git-annex or its laboratory scripts | Runtime consumers above, stock git-annex laboratory and packaging |
| Build configuration, dependencies, CI workflows or selector | Full set |
| Unknown paths, unavailable history, empty diffs or a stale PR base | Full set |

Mixed changes select the union of their dependent groups. Specific check
scripts select their own groups; unrecognized tooling conservatively runs
everything. This table describes the current groups in `scripts/select-ci`;
new crates and paths remain full until their dependencies are classified.

## Required version bumps

Every PR updates the workspace version and lockfile. Treating those paths as
unconditional dependency changes would defeat selection. The selector ignores
them only when it can compare both complete TOML documents and prove that:

- The root workspace configuration is identical apart from a stable version
  bump. Invalid versions, prerelease transitions and decreases keep full checks.
- The workspace member names agree and inherit the workspace version.
- Every local workspace lock record follows that version exactly once.
- Every other lock field, dependency, registry version and checksum agrees.

A missing or malformed document, changed dependency or incomplete version bump
keeps full checks. The Rust jobs still run when the actual source changes.
Release-tag Rust checks remain unconditional through full event selection.

## Diff and validation evidence

Selection uses local Git objects for the PR's immutable base and head. The
head must contain that base; otherwise selection stays full until rebased.
The diff has no GitHub API file limit. NUL framing preserves unusual names;
renames inspect both the deleted source and added destination path.

Regression tests exercise real Git repositories, more than 350 changed
files, renames into documentation, mixed groups, malformed lockfiles,
dependency changes hidden alongside version bumps, registry packages sharing
a workspace name, missing history, stale bases and rejected skipped checks.
Actionlint checks the reusable workflow outputs and job dependencies.

Replaying PR #57's exact diff against `d441344` selects packaging alone,
after independently verifying its version-only bump. This is a checked
selection result, not yet a measured wall-time improvement. The initial
selector rollup itself changes CI policy and therefore runs the full set.
