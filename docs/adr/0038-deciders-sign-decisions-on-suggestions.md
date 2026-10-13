# 0038. Deciders sign decisions on suggestions; authors withdraw their own

- **Status:** Proposed, 13 October 2026, by the agent, for issue #7
  (Guillaume's review: "Authorize suggestion decisions by non-admin owners
  and stewards"). Refines ADR 0009 for `.pigeon/decisions/`, as ADR 0017
  refined it for device entries.

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

## Decision

- The **deciders** of a suggestion (a proposal commit) are the persons who
  may make every change of its diff under the roster at its parents. If
  nobody may make them all, the admins are its deciders.
- **Accepting** is the adoption merge of ADR 0009: a signed merge whose
  parents are `main` and the proposal commit, valid when its diff against
  `main` is allowed for its signer. The proposal commit stays a signed
  submission; the merge is the authorised change.
- **Rejecting** writes `.pigeon/decisions/<commit>.toml` with
  `verdict = "rejected"`. It is valid when signed by a decider of that
  commit, under the roster at the decision's parents.
- **Withdrawing** writes the same file with `verdict = "withdrawn"`, valid
  when signed by a device of the suggestion's author.
- A decision names one commit, so it applies to that revision only. A later
  revision of the same chain is pending until decided itself; adopting a
  revision adopts its ancestors in the chain.
- An adoption wins over a concurrent rejection of the same commit: content is
  never thrown away silently.
- The rest of `.pigeon/` keeps ADR 0009's rule (admins), with ADR 0017's
  exception for device entries and ADR 0018's purge decisions.

## Consequences

- A non-admin owner or steward can decide every suggestion in their scope,
  and nothing else in `.pigeon/`.
- The authority rule gains one table of who may write which `.pigeon/` path
  (specification, 04 §4.3), with property tests over owners, stewards,
  admins, unrelated members, stale revisions and suggestions spanning several
  folders.
- A suggestion touching folders of different owners can be decided only by
  the admins; capture avoids making one by splitting per set of deciders.
