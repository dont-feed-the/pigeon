# 0051. The roster is a fold of its changes in a canonical order; roster forks merge without an admin

- **Status:** Proposed, 14 October 2026, by the agent, for issue #6
  (Guillaume's review: "Resolve concurrent roster updates with disjoint
  remaining admin sets"). Replaces the merge rule of ADR 0009 for
  `roster.toml`; the roster stays in the tree.

## Context

ADR 0009 keeps the roster in the tree, and says that a merge of concurrent
roster changes needs an admin signature valid under both parents' rosters.
An honest group can leave no such signer. Alice and Bob are the admins.
Offline, Alice gives up her admin role, which leaves Bob. Independently, Bob
gives up his, which leaves Alice. Both commits are authorized, and no device
is an admin under both results.

The specification had two other answers, and they disagree. 04 §4.7 merges
`roster.toml` per TOML table: the two retirements touch different tables,
so the merge is clean and leaves no admin. 05 §5.2 keeps the whole file of
the first representative and makes the other side a proposal to the admins.
That drops an authorized change, and it can hold back the removal of a stolen
laptop until an admin approves it again. Neither is a function of the
accepted history alone once three branches meet: a three-way merge depends on
which pairs are merged first.

## Options

1. **Serialize roster changes** through an online authority: a designated
   machine or a quorum of admins. Forks cannot happen. But no roster change
   is possible offline, and a lost authority blocks the group.
2. **Allow forks, and keep ADR 0009's signer rule.** The example deadlocks.
3. **Allow forks, and merge the file three-way with repairs.** The result
   depends on the order of merges.
4. **Allow forks; the roster is a fold of its changes in a canonical order.**
   A merge only recomputes it, so it needs no authority.

## Decision

Option 4.

**Changes.** Each accepted commit *c* carries a roster **change** δ(c): the
difference between the roster its accepted parents give (below) and the
`roster.toml` of its tree. A clean merge has none. A decision merge carries
the change of the proposal it adopts (04 §4.5). A change is a list of edits:

- `set` a field of an entry, from the value the author saw to a new value;
- `add` an entry (a person, a device, a folder rule);
- `remove device` D;
- `remove person` P, which also removes P's devices and P from steward lists.

δ(c) is authorized once, when *c* is judged, under the rosters of its
parents (04 §4.4). It is never judged again.

**The fold.** R(c), the roster in force for the children of *c*, applies the
changes of *c* and of all its accepted ancestors, in **canonical order**:
topological, and among commits whose ancestors are all placed, the smallest
(recorded author time, device ID, commit ID) first. That is the order of
representatives of 05 §5.2. A change applies atomically, or not at all:

- a `set` applies if the field still has the value its author saw, or
  already has the new one; otherwise it **conflicts**;
- an `add` applies if the entry is absent or already equal; otherwise it
  conflicts;
- a `remove device` always applies, and is a no-op if the device is already
  gone; a `remove person` removes the devices the person has at that point;
- the change is **skipped** if one of its edits conflicts, or if the roster
  after it would be invalid (04 §4.2): no admin left, a device of an unknown
  person, a folder rule naming an unknown person, or a device ID or
  `annex_uuid` that the history already removed;
- a skipped change still removes the devices it removes, a person's devices
  included. Removing a device never makes a roster invalid, and a revocation
  must not depend on the rest of its commit.

For a linear history this is the same as applying each commit in turn. The
change of *c* itself was computed against its parents' fold, so it always
applies in R(c). The tree of every commit holds R(c), which validation
checks.

**Merges.** The roster of a merge is the fold: `merge` (05 §5.2) does not
merge `roster.toml` as a file. A clean merge has no roster change, so a
device of the roster of any of its parents may sign it (04 §4.4, rule 2),
even a device that the other parent removed: the merge adds nothing, and
the fold removes that device from the merge's roster. The `Roster` model
finds that requiring the signer in every parent's roster leaves two branches
that removed each other's devices with no one to merge them
(`Roster_every`). A merge that writes anything else
in `roster.toml` carries that difference as its change, judged like any
other.

**Skipped changes are shown, never lost silently.** The fold also lists the
changes it skipped, with the reason, a pure function of the history. The
admins and the author see *Needs you*: "Alice and Bob gave up their admin
role at the same time. One admin must remain: Bob stays admin." *Apply
again* writes a new change, judged under the roster of now. In the example,
the canonical order applies the earlier retirement; the other is skipped.

**What forks keep.** A content commit is judged by the rosters of its
parents (04 §4.4, locality), so a roster change on one branch never rejects
an offline contribution that the other branch allowed. Removals win: a device
removed on one branch is absent from every roster that includes that branch,
even when the rest of its change is skipped.
Additions can conflict and be skipped; they are then shown and can be redone.

**Recovery.** If the admins lose their keys, the paths of ADR 0047 apply
(recovery sheets, refounding). No signature check is relaxed for it.

## Consequences

- No roster change needs an online authority. The trade-off: a change made on
  a fork may be skipped when the forks meet, and its author redoes it.
- The roster at a commit is a function of the set of its accepted ancestors.
  Every device computes the same R(c), whatever the order in which it
  received or merged the branches.
- A removed device can still make accepted commits on a branch that has not
  yet seen its removal. This was already so (02 §2.4: a stolen laptop until an
  admin removes it), and it ends where the branches meet.
- A recorded author time in the past puts a change early in the order, so it
  wins conflicts on its fields: the later change finds a value it did not see.
  Time only orders, as in 05 §5.2; `Roster_clock` checks that a skewed clock
  changes which change applies, never validity.
- A change that applied on its branch can be skipped at the merge: in the
  example Bob's retirement is undone. Later commits on his branch stay valid,
  since each was judged at its parents. Likewise, when a concurrent person
  removal leaves no admin (Alice removes Bob while Bob removes Alice), the
  later one is skipped (`Roster_person`): removing a person is best effort
  across forks, removing a device is not. A skipped key rotation still
  removes the old key but does not add the new one, which is then redone.
- R is the fold from genesis, not "my roster plus the other side's changes":
  the canonical order of a union does not extend either side's order, and
  replaying gives different rosters depending on who merges
  (`Roster_rebase`). Computing R costs a walk of the roster-changing commits,
  which are few; `pigeon-core` caches R by canonical prefix.
- Laws (property tests, ADR 0012): R depends only on the ancestor set
  (merge order, duplicate delivery, and split and rejoin cycles do not
  matter); R of a linear history is sequential application; every R has an
  admin; a removed device or `annex_uuid` never returns; a change applies at
  most once; a skipped change is listed. The `Roster` TLA+ model checks the
  concurrent self-retirement, rotation of a device key during a concurrent
  removal, and merge orders, and shows ADR 0009's rule deadlocking
  (13 §13.2).

## Alternatives considered

- *Union of the admin sets on conflict.* Symmetric in the example, but it
  needs a rule for every pair of fields, and it can bring back a demoted
  admin. Rejected for the single rule of the fold.
- *Last writer wins per field.* It silently overwrites a concurrent decision.
  Rejected: a conflicting `set` is skipped and shown.
- *A CRDT document* (ADR 0009, option 4). The fold is one, with commits as
  operations. A library would add a format beside git, which ADR 0009
  rejected.
