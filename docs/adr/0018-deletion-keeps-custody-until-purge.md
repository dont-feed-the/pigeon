# 0018. Deleting a file never deletes its custody copies; only a purge does

- **Status:** Accepted, 13 October 2026, as a clarification from Guillaume
  (chat): laptops may delete what they own; custodians accept the deletion
  but keep restorable copies until a purge. The mechanism below is proposed
  by the agent and is to be checked by experiment E14.

## Context

The proposal separates three removals: *unlink* a path, *evict* a local
copy, *release* protection (purge). P3 requires that every deleted
protected version stays restorable until purge. In git-annex, deleting a
file is a commit; its content becomes *unused* in the current tree but is
still referenced by history. E9 showed that custodians must want history
(`anything` with `--all`), or old versions fall below K. git-annex refuses
to drop below `numcopies` without `--force`, so a purge, which removes every
copy, cannot happen through an ordinary drop.

## Decision

- A deletion by the path's owner is published as an ordinary commit;
  deletions of paths owned by someone else become proposals (ADR 0009).
- Custodians merge accepted deletions like any other commit and keep the
  content of every version not purged: their preferred content covers all
  of history, not only the current tree.
- A purge is a signed decision in `.pigeon/decisions/`, naming the released
  versions and the retention end. `pigeond` marks the released keys with
  git-annex metadata (`pigeon-released=<decision>`), which custodians'
  preferred content excludes once the retention has ended.
- After the retention, each custodian's `pigeond` drops the released keys
  locally with `--force`, the only forced drop pigeon performs, and records
  it. Remote `REMOVE` stays refused (ADR 0014).

## Consequences

- An `rm -rf` on a laptop costs nothing durable: it is a commit (E2/E4),
  revertible until purge.
- Custody capacity must cover retained history (ADR 0003).
- E14 must confirm that metadata-based preferred content stops re-fetching
  released keys from custodians that have not dropped them yet; otherwise
  `pigeond` keeps its own released-key list and filters its transfers.
