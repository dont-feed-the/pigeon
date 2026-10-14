# 0038. Deciders sign decisions on suggestions; authors withdraw their own

- **Status:** Proposed, 13 October 2026, by the agent, for issue #7
  (Guillaume's review: "Authorize suggestion decisions by non-admin owners
  and stewards"); revised 14 October 2026 for issue #10 on the evidence of
  the `Authority` model, absorbing the former ADR 0063. Refines ADR 0009 for
  `.pigeon/decisions/`, as ADR 0017 refined it for device entries.

## Context

ADR 0009 lets only admins change `.pigeon/`. Rejecting a suggestion writes
`.pigeon/decisions/<commit>.toml`, so a folder owner or steward who is not an
admin could accept a suggestion (the adoption merge changes only their
folder) but not reject one. The proposal gives decisions on suggestions to
the owner or the stewards of the folder, and ADR 0034 lets the author take
their own suggestion back. A decision must also name exactly what it decides:
suggestions form chains when the author saves again (one commit per
revision), and a suggestion found in a peer's history may touch folders with
different deciders.

The first version of this ADR judged deciders under the roster at the
proposal's parents. The `Authority` model ([13 §13.2](../spec/13-verification.md)),
with an honest admin a, b who owns a folder, and a faulty member c who signs
anything on any parent, found five problems with that version:

- **Deciders at the proposal's parents** (`Authority_proposal`). a suggests a
  change in b's folder, then gives that folder to himself; rule 3 of 04 §4.4
  rejects b's adoption, and a can adopt but not reject.
- **The proposal's own diff** (`Authority_mixed`), even under today's roster:
  when `main` has meanwhile changed a path the proposal also adds, adopting
  keeps both versions (04 §4.5) and touches what only admins may change.
- **Admins without Force** (`Authority_noforce`): rule 3 rejects an admin's
  adoption on folder rules unless it is a Force (04 §4.6).
- **A merge that adopts nothing** (`Authority_fakeadopt`, `Authority_base`):
  any member signs a clean merge of `main` with a proposal, keeping `main`'s
  tree; the proposal counts as accepted although its content never entered.
- **States that overlap**: a decided proposal can have a later revision, and
  a rejection can be concurrent with a withdrawal.

An adoption made on a `main` that lacks an accepted commit the proposal builds
on also brings that commit in, so its tree is not the catalogue of its history
(`TreeIsCatalogue`).

## Decision

1. **Deciders are judged at the decision.** For a decision on `main` *m*, the
   deciders of a proposal are the persons who may make the adoption's change
   on *m* (the diff between *m* and *m* with the proposal's change, 04 §4.5
   *Accept*), under R(*m*). If nobody may, the deciders are the admins, who
   adopt with `Pigeon-Force` (04 §4.6), shown as an admin override. Adoption
   and rejection use this one set, recomputed when `main` moves.
2. **Accepting** is a signed merge whose parents are `main` and the proposal
   commit (ADR 0009). By rule 7 of 04 §4.4, every parent outside A(c) must be
   a proposal, the signer must be one of its deciders under the roster of
   each accepted parent, and the merge's tree must hold the proposal's
   change; otherwise the merge is not accepted and is a proposal itself. The
   proposal commit stays a signed submission.
3. **Rejecting** writes `.pigeon/decisions/<commit>.toml` with
   `verdict = "rejected"`, signed by a decider. **Withdrawing** writes the
   same file with `verdict = "withdrawn"`, signed by a device of the
   suggestion's author.
4. A decision names one commit, so it applies to that revision only. A later
   revision of the same chain is pending until decided itself; adopting a
   revision adopts its ancestors in the chain.
5. **The state of a proposal** is the first of these that holds: accepted,
   rejected, withdrawn, superseded, pending. An adoption wins over a
   rejection (content is never thrown away silently), a rejection over a
   withdrawal, and any decision over a later revision.
6. `pigeond` decides only on a `main` that holds every accepted commit the
   proposal builds on, merging them first.
7. The rest of `.pigeon/` keeps ADR 0009's rule (admins), with ADR 0017's
   exception for device entries and ADR 0018's purge decisions.

## Consequences

- A non-admin owner or steward can decide every suggestion in their scope,
  and nothing else in `.pigeon/`. The authority rule gains one table of who
  may write which `.pigeon/` path (04 §4.3).
- In the `Authority` model, every decider of a pending proposal can make a
  valid adoption (`DecidersCanAdopt`), every decision was made by a decider
  or the author (`DecisionsByDeciders`), a proposal is accepted only through
  a merge that holds its content (`AdoptionHoldsContent`), and each version in
  a catalogue entered through a commit allowed to make it
  (`AuthorisedContent`); `Authority_decide` and `Authority_faulty` pass.
  Property tests cover owners, stewards, admins, unrelated members, stale
  revisions and suggestions spanning several folders.
- The deciders shown for a suggestion can change when `main` changes, for
  example after a folder changes owner; its page shows them as of now.
- A faulty or old client's merge that does not adopt a proposal stays a
  proposal: it takes nothing away.
- A suggestion touching folders of different owners can be decided only by
  the admins; capture avoids making one by splitting per set of deciders.

## Alternatives considered

- **The roster at the proposal's parents**, or the proposal's own diff under
  today's roster: each leaves a decider who cannot adopt.
- **Deciders frozen when the proposal is made**: the same failure, as soon as
  the folder changes owner.
- **A proposal is accepted when its content is in the catalogue**: a
  deletion, or a change that `main` made identically meanwhile, has no
  content to find. Requiring a decision merge is simpler to check.
