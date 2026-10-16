# Report: recovery continuity

This 0.4.6 candidate continues M4 recovery after
[recovery-paper-5.md](recovery-paper-5.md). It does not complete the M4
dashboards or the later release gates.

## What changes

A surviving ordinary member can refound a group whose folder rules named
people who no longer belong to it. Owned folders pass to that survivor;
add-only boundaries remain and absent stewards are removed. Paper keys
cannot refound. Group settings and the surviving computer's identity stay.

An old laptop or bare backup can accept a refounded invitation in its
existing repository. A separate, private staging repository verifies the
welcome and each signed archive link without exposing old local objects
or accepting incoming content. A rejected welcome does not change the
old repository or configuration. After verification, a durable local
journal records completion before the old transport stops and new history
is imported. Startup replays it without the inviter or another redemption.
Annex UUID, token and existing content are reused. The signed archive tip
is canonical; other old local tips and proposals remain separately archived.

Invitations are bound to their issuing group and the first authenticated
device. That device can resume a lost welcome, subject to current role,
UUID and removal checks. A ticket cannot become valid for a refounded
group just because its private invitation file survived. Unused legacy
records without a group must be reissued. No invitation secret is stored
in the transition journal.

## Evidence

- Both pure algebras have four laws with 1000 generated cases each.
  Refounding includes ordinary members, backup roles, owned folders,
  add-only folders and preserved settings.
- Local replay regressions reject malformed, future and unrelated intents,
  preserve other configuration entries, archive old proposals, and avoid
  resetting a later index after completion.
- Eight real SIGKILL scenarios passed: before main changes, after main,
  after index and after configuration, for both self-refounding and a bare
  backup joining in place. Backup replay ran with its inviter offline and
  retained existing object bytes and inodes. Laptop files retained their
  bytes and inodes. The scenarios completed in 33.84 seconds on Linux.
- Real QUIC invitation regressions passed in 48.03 seconds: interrupted
  first-use persistence, lost-welcome retry, changed role/UUID, another
  key's reuse, revoked membership, unbound legacy records and old-group
  tickets.
- The complete workspace checks passed after updating the lab registry
  to distinguish ordinary loop faults from recovery-workload faults.
  Following the dirty-index repair, the ordinary-member integration passed
  in 7.12 seconds, all eight real SIGKILL cases passed again in 30.83
  seconds, and the dirty binary/untracked-file regression passed. The web
  tests cover keeping an action result separate from Home's live updates.
- An independent participant completed 45 normal-build tasks, including
  refounding twice as an ordinary member, current origin metadata, a fresh
  dirty-folder rejoin, exact binary/CRLF edits and drafts, object reuse,
  nested history, read-only views and restores. The initial join stranded
  dirty files because `read-tree -m` refused the changed pointer; index-only
  `read-tree --reset` fixed both startup replay and a fresh join. User files
  are not checked out or replaced by that operation.
- Another participant tested admin-owned and member-owned backups. Both
  retained UUID, token, all three object byte sequences/inodes, an extra
  accepted local old tip, the exact signed canonical archive and protection.
  Unrelated populated targets were refused unchanged and terminal declines
  cancelled safely.
- The French browser participant verified confirmation and CSRF checks,
  old-ticket refusal, and a G1/G2/G3 signed-link mismatch. Full old repository,
  refs, annex and configuration fingerprints were unchanged on refusal.
  The final normal build also retained dirty tracked and untracked bytes.
  Its six-second retest confirmed specific errors stay visible: Home's
  EventSource had previously overwritten action titles.

## Follow-up access and wording work

The backup participant found two access gaps: `history` rejects paths in a
bare backup, and default offline `export --deleted` omits archived deleted
files. Explicit export at the canonical old archive recovered the exact
deleted bytes in both cases, so the content remains intact. The follow-up
archive-export change addresses those paths before M4 can be complete.
The browser confirmation still includes a terminal `[y/N]` suffix, and
restore's result should distinguish a local version from an accepted group
change. The CLI exposes no owned-folder setup flow; those rules were
covered by the algebra tests, rather than claimed as a participant task.

The usability participants use isolated processes on one Linux host. The
user explicitly accepted agent participants as usability evidence and
instructed us to treat the members' dogfood gate as satisfied. This does
not stand in for cross-platform CI, seven nightly model runs or the soak.
