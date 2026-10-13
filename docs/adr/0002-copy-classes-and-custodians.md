# 0002. Copy classes: untrusted thin laptops, semitrusted custodians, K custody copies

- **Status:** Accepted, 12 October 2026. Laurent accepted the custodian
  specification in chat; recorded in the proposal of 12 October 2026. It
  replaces the earlier *diogene* node.

## Context

A laptop's working copy must cost about one copy of disk space, so it is a
thin hard link (`annex.thin`), and an in-place edit destroys it (E8). It
cannot count toward durability. Draft 1 relied on *diogene*, a node that
keeps everything; in git-annex terms that is a `trusted` repository, whose
copies are counted without verification, which the review warns against.

## Decision

- **Laptops**: unlocked files, `annex.thin`, `untrust`,
  `annex.autocommit=false`, `annex.synccontent=false`. They never count toward K.
- **Custodians**: members' machines dedicated to storage, in group
  `custodian`, holding locked, read-only objects, never exposed to
  applications. They are `semitrusted`, never `trusted`, so every drop
  verifies them. They count toward K.
- Group-wide `numcopies = mincopies = K`, default K = 2.
- A version is *protected* when at least K distinct custodians hold
  verified copies of it.

## Consequences

- K = 2 means two custodians, not a laptop and a backup. A laptops-only group
  gets no protection and is told so.
- Evicting a laptop copy waits until the version is protected (E9).
- Custodians need operators, quotas and a replacement plan.
