# 0064. pigeon-core's first algebras fix the readings of the spec that their laws need

- **Status:** Proposed, 14 October 2026, by the agent, for issue #17
  (Guillaume, in chat: "work on a first version of the implementation even if
  not all details are figured out yet"). Refines the readings of 04 §4.8,
  06 §6.2 and §6.5, 07 §7.5, 12 §12.3 and 15 §15.2 for the code; ADR 0012's
  method, ADR 0049's domains and ADR 0055's journal are unchanged.

## Context

13 §13.12 asks for the laws of the parts of `pigeon-core` the safety slice
uses before their drivers: states and headline, domain counting, the install
journal, the queue order and the roster fold. Writing them as code and as
hegel-rust properties (`hegeltest` 0.49, native Rust, no server) left
choices the spec does not make, and found one reading of 06 §6.2 under which
its own law fails.

- *Domains of H.* Read as the connected components of H alone, a new
  custodian holding a key can join two domains of H through hardware it
  shares with both, and the version falls from *protected* to *published*:
  more observations lower the state, against the law of 06 §6.2. The
  property found it only beyond 2000 cases; an example test pins it.
- *A staged file before `begin`.* A crash between steps 2 and 3 of 07 §7.5
  leaves a staged file that no journal entry names; nothing removed it.
- *Names.* 12 §12.3 listed `CaptureInstall` actions (`Save`,
  `InstallStep`, `Resolve`) that the model does not define, while asking a
  test to check that the names match.

## Decision

1. **Domains** (06 §6.2). The failure domains partition every custodian of
   the roster; D is the set of domains holding a member of H. With
   `min_sites` = 1 sites ask nothing; above it, only declared sites count.
2. **Reasons** (06 §6.4). A version that is not protected takes the first
   that applies: `same-domain` or `same-site` when |H| ≥ K; then
   `repository-lost`, `custodian-offline`, `too-few-custodians`,
   `group-unreachable`, and `sending` otherwise. A *local* version has one
   too.
3. **Headline** (06 §6.5). A *protected* version whose time is older than
   `stale_after` is *Not checked recently*; a *missing* one
   (`content-missing`) is *Working*, then *Not safe yet* after the grace
   period; an *erased* one adds nothing.
4. **Queue** (15 §15.2). Among protect items with equal custody domains,
   fewer laptop copies go first; every class breaks its remaining ties on
   its other fields (key, path, peer), so the order is total and equal items
   are identical.
5. **Journal** (07 §7.5). A released aside is journalled `released`;
   recovery removes staged files that no entry names; an ended entry's
   recovery only removes its staged file.
6. **Roster fold** (04 §4.8). The device removals of a skipped change are
   its `remove device` edits; a skipped `remove person` removes nothing, as
   the `Roster` model's `Removals = "always"`. The fold carries the devices
   ever removed with their `annex_uuid`. `may_sign`: an admin signs any
   change; a device of person P adds and removes P's devices and sets their
   capacity, domain and site; a recovery device only adds and removes its
   person's devices; nobody removes their own person. A clean merge may be
   signed by a device of some parent's roster.
7. **Trace names** (12 §12.3). Each `pigeon-core::trace` enum, behind the
   feature `tla-trace`, has one variant per action of its model, named as
   the model's operator; for `CaptureInstall` these are the model's actions.
   The NDJSON is written by hand, so `pigeon-core` keeps no dependency.
8. **Tests.** Each law runs at least 1000 hegel cases; a law that needs a
   rare input also gets a targeted generator and an example test. The
   database directory `.hegel/` is not committed.

## Consequences

- 13 §13.13 lists each law with its module, test and the model it mirrors.
- The monotonicity of 06 §6.2 holds for any hardware layout, at the cost of
  computing domains over the whole roster rather than over H.
- Reasons, headline levels and the queue are deterministic functions, so
  two devices with the same observations say the same thing.
- Changing a reading here means changing a law, which a test then checks.
