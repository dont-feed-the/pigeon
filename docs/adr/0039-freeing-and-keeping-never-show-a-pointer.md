# 0039. Freeing and keeping a folder never show a pointer file: hide before dropping, stage before showing

- **Status:** Proposed, 13 October 2026, by the agent, for issue #2
  (Guillaume's review: "Prevent pointer files from becoming visible during
  folder eviction and restoration"), on the evidence of experiment E15b.
  Replaces the order of steps of ADR 0031; its other decisions stand.

## Context

ADR 0031 promises that a folder's files are complete or absent, never shown
as git-annex pointer files. Its order of steps breaks that promise between
steps. Freeing drops the content while the folder is still checked out, and
git-annex then replaces each unlocked file with its pointer until the folder
is hidden. Keeping shows the folder before `git annex smudge --update` fills
it, and git's checkout writes pointers first. E15 checked only the final
states. E15b (`lab/git-annex/ga-e15b-pointer-free-transitions.sh`) scans the
folder continuously during each sequence: with 100 files, ADR 0031's freeing
showed 44 887 pointer observations, and its keeping 1 306.

## Decision

- **Freeing folder F:** first hide the files of F whose versions are
  protected, with `git sparse-checkout` (non-cone patterns that exclude F
  and re-include its files that are not protected yet). Each file goes from
  complete to absent. Then drop their keys with a verified `git annex drop
  --key`, keeping keys that a kept path still uses. A file that becomes
  protected later is hidden, then dropped, the same way.
- **Keeping folder F:** `git annex get --key` every key of F first. Then
  write the complete files under `.git/pigeon/stage/F`, with the modes of
  their index entries, and move the staged folder into place with one
  `rename` that refuses to replace (`RENAME_NOREPLACE`, `RENAME_EXCL`).
  Then update the sparse patterns and clear the skip-worktree bits of F's
  index entries (`git update-index --no-skip-worktree`, `--refresh`,
  `git annex restage`), without any checkout. If F exists on disk (the
  person made a folder of that name meanwhile), the files are installed one
  by one with the install protocol, and colliding files become divergences.
- A file that git leaves in place when hiding (it differs from the index:
  someone saved it) is captured like any save; freeing resumes after.
  A folder with an unresolved divergence is not freed until it is resolved.
- Freeing and keeping hold F's lock in `pigeond`: capture does not scan F
  during the transition. The target is recorded first, in this device's
  `wanted` expression (ADR 0031), and both sequences are idempotent, so a
  restart reruns the transition toward the target before capture resumes. A
  stale `index.lock` left by a killed git process of `pigeond` is removed at
  start.
- Keeping is all or nothing per folder: if some keys cannot be fetched yet,
  F stays hidden and its row says how many files are still being received.

## Consequences

- E15b shows no pointer observed in either new sequence, every file
  complete or absent after `kill -9` at random instants (10 trials), and a
  clean `git status` after a rerun. It also shows that `git sparse-checkout`
  leaves a modified file in place with a warning, never deleting it.
- Hidden files are never interpreted as deletions: `git status` skips
  skip-worktree entries, and the staging directory is inside `.git`.
- Keeping needs free space for a full copy of F before it appears, and,
  without `annex.thin` or reflinks, twice F's size during the move (see
  issue #3 for the copy mode of laptops).
- ADR 0031's status gains "order of steps replaced by ADR 0039" once this
  ADR is accepted.
