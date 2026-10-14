# 0051. Authority is a fold of signed changes: roster forks merge without an admin, and deciders sign decisions on suggestions

- **Status:** Proposed, 14 October 2026, by the agent, for issues #6
  (Guillaume's review: "Resolve concurrent roster updates with disjoint
  remaining admin sets") and #7 ("Authorize suggestion decisions by non-admin
  owners and stewards"), on the evidence of the `Roster` and `Authority`
  models. Revised 14 October 2026 for issue #10, absorbing the former ADRs
  0062 (revocation) and 0063 (a decision is a merge), and at Guillaume's
  request (chat), absorbing the former ADR 0038 (decisions on suggestions).
  Replaces the merge rule of ADR 0009 for `roster.toml`, and refines ADR
  0009's rule that only admins change `.pigeon/`; the roster stays in the
  tree.

## Context

pigeon has no server (ADR 0001): every device judges every commit by itself,
offline, from signatures and the roster in the tree (ADRs 0007, 0009). Three
problems came out of the review and the models:

- **Forks deadlock.** ADR 0009 says a merge of concurrent roster changes needs
  an admin valid under both parents' rosters. Alice and Bob are the admins;
  offline, each gives up the role. Both commits are authorized, and no device
  is an admin under both results. The specification's two other answers (a
  per-table TOML merge, or keeping one side and proposing the other) either
  leave no admin, or hold back an authorized change such as the removal of a
  stolen laptop; and neither is a function of the history once three
  branches meet.
- **A removed device keeps signing.** A commit is judged at its parents, so a
  stolen laptop can sign on an old parent after its removal, and add a fresh
  key for its person. The `Authority` model shows that a merge policy does
  not stop it (`Authority_tip`, `Authority_contentonly`).
- **Decisions on suggestions.** A folder owner or steward who is not an admin
  could adopt a suggestion in their folder but not reject one, since that
  writes `.pigeon/decisions/`. Judging deciders at the proposal's parents,
  or by the proposal's own diff, leaves deciders who cannot adopt, and a
  clean merge that adopts nothing could count as an acceptance
  (`Authority_proposal`, `Authority_mixed`, `Authority_fakeadopt`).

How authority is computed is the most expensive thing to change later: every
device must compute the same roster from the same history, forever, or groups
split.

## Options

1. **Serialize roster changes** through an online authority (a machine, or a
   quorum of admins). No forks; but no change is possible offline, and a lost
   authority blocks the group.
2. **Allow forks and keep ADR 0009's signer rule.** The example deadlocks.
3. **Allow forks and merge the file three-way, with repairs.** The result
   depends on the order of merges.
4. **Allow forks; authority is a fold of signed changes in a canonical
   order.** A merge only recomputes it, so it needs no authority.

## Decision

Option 4.

1. **Changes.** Each accepted commit carries a roster change: the edits
   (`set` a field from the value its author saw, `add` an entry, `remove
   device`, `remove person`) between its parents' roster and its tree. It is
   authorized once, when the commit is judged under its parents' rosters
   (04 §4.4), and never judged again.
2. **The fold.** The roster in force after a commit applies the changes of
   all its accepted ancestors in **canonical order** (topological, ties by
   recorded author time, device ID, commit ID). A change applies atomically
   or is **skipped**: when a `set` finds a value its author did not see, or
   when the result would be invalid (no admin left, an unknown person, a
   removed device or UUID back). Removals always apply, even in a skipped
   change. Skipped changes are listed and shown to the admins and the author,
   who can apply them again; they are never lost silently.
3. **Merges.** `roster.toml` is never merged as a file: a clean merge has no
   change, and any device of any parent's roster may sign it.
4. **Revocation.** Where a history is read (the fold, the catalogue), a commit
   signed by a device that the history removes, and that the removal had not
   seen, does not count; only its removals apply. Nothing is deleted: its
   change is shown as a suggestion of its person.
5. **Decisions on suggestions are signed by their deciders.** The deciders
   of a proposal are the persons who may make the adoption's change on the
   `main` it is decided on, under that `main`'s roster; if nobody may, the
   admins, with `Pigeon-Force`. Accepting is a signed merge of `main` with the
   proposal whose tree holds the proposal's change; any other merge stays a
   proposal. Rejecting writes `.pigeon/decisions/<commit>.toml` signed by a
   decider; the author's own devices may withdraw it the same way. A decision
   names one revision. Adoption wins over rejection, rejection over
   withdrawal. The rest of `.pigeon/` keeps ADR 0009's rule (admins), with
   ADR 0017's exception for device entries.

The specification carries the rules in full: [04 §4.3–4.8](../spec/04-roster-and-authority.md)
(the authority rule, proposals, Force, the fold, revocation),
[05 §5.2](../spec/05-catalogue-and-names.md) (the catalogue reads the same
non-revoked commits) and [02](../spec/02-guarantees.md) (INV-Revoke).

## Consequences

- No roster change needs an online authority. The trade-off: a change made on
  a fork may be skipped when the forks meet, and its author redoes it. In the
  example the earlier retirement applies, the other is skipped and shown.
- Every device computes the same roster for the same set of commits, whatever
  the order it received or merged them (`Roster` model; laws in 13 §13.4).
- A removed device can still make accepted commits on a branch that has not
  seen its removal (02 §2.4), and they stop counting where the branches meet:
  the catalogue is a function of a set of commits, but no longer monotone,
  and install treats a revoked change as a deletion by the group; the content
  stays in history and with the custodians.
- A non-admin owner or steward decides every suggestion in their scope and
  nothing else in `.pigeon/`. The deciders shown for a suggestion follow the
  current `main`; a suggestion spanning folders of different owners is
  decided by the admins.
- A recorded author time in the past moves a change earlier in the order; time
  only orders, never validity (`Roster_clock`).
- If the admins lose their keys, ADR 0047 applies; no signature check is
  relaxed for it.

## Alternatives considered

- **An online authority or a threshold of admins**: v1 has no server and no
  quorum (ADR 0001).
- **Union of the admin sets on conflict**, or **last writer wins per field**:
  one brings back a demoted admin and needs a rule per field, the other
  silently overwrites a concurrent decision.
- **A CRDT library** for the roster: the fold is one, with commits as
  operations; a library would add a format beside git, which ADR 0009
  rejected.
- **A merge policy or a time cut-off** against removed devices: the model lets
  the change in through a device that has not seen the removal, and author
  times are the signer's own claim.
- **Deciders frozen at the proposal**, or judged by its own diff: the folder
  changes owner and a decider can no longer adopt.
- **A proposal is accepted once its content is in the catalogue**: a deletion,
  or a change `main` made identically, has no content to find.
