# M5 restore safety evidence

Status: implementation evidence for M5, 16 October 2026. This report does
not declare the milestone complete.

A restore publishes its ordinary signed main or proposal reference before
requesting unavailable contents. The request survives restart, does not alter
the index while contents are unavailable, and can finish after a copy arrives.
It creates a new reference even when its bytes appeared in older history;
an earlier erasure does not cover that new reference.

The cold history scan yields the worktree lock. Reading a new index after
that scan could mistake an intervening captured save for a file the restore
may replace. Restore now retains its original file identity, size, mtime,
link count and mode through staging and the move aside. A changed file waits
for a new explicit restore. An already completed exact restore is recognized
without writing over it. Missing-content retries use the initial index rather
than the latest history entry, which already includes the pending request.

A member's index stays on main while its saved local version is a suggestion.
Restore verifies that the exact own proposal ref and its pointer entry still
retain those bytes before replacing them. Binding the restored suggestion
uses only a stable stat verified against the requested bytes and mode; a
later save of different contents cannot inherit its stat accidentally.

## Checked interleavings

`restore::guard::tests` runs real Git, git-annex, capture and restore drivers:

- A newer private save captured during the yielded interval remains visible,
  indexed and retained after reopening the repository.
- A deletion captured in that interval remains deleted and indexed.
- An executable-bit change remains visible and indexed with unchanged bytes.
- An unchanged original file restores successfully; different bytes or mode
  cannot produce a verified suggestion-binding stat.

`tests/restore_missing.rs` drives real daemon/CLI requests for absent file
contents, an absent folder snapshot and a member's signed suggestion. It
checks durable requests, unchanged existing bytes and index, restart and
explicit retries after supplying a copy. The member retry also makes a
private save first and verifies its retained annex object after restoration.
Fixture-only forced removal creates missing contents without claiming a
retention month elapsed or bypassing the production erasure gate.

Existing file-time, deleted-folder, executable-mode, install, erasure and
scrub regressions remain part of validation. These controlled interleavings
and the browser returning-peer reports do not establish post-seal partition
coverage, complete `Purge` implementation traces or longitudinal retention.
Those remain separate M5 evidence requirements.
