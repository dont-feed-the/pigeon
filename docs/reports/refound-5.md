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
- The existing ordinary-member/laptop in-place integration passed after
  the journal implementation. Full workspace checks and normal-build
  independent usability sessions are being run before marking the pull
  request ready. Their final results belong here before release.

The usability participants use isolated processes on one Linux host. The
user explicitly accepted agent participants as usability evidence and
instructed us to treat the members' dogfood gate as satisfied. This does
not stand in for cross-platform CI, seven nightly model runs or the soak.
