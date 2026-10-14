# 0051. The roster is a fold of its changes in a canonical order; roster forks merge without an admin

- **Status:** Proposed, 14 October 2026, by the agent, for issue #6
  (Guillaume's review: "Resolve concurrent roster updates with disjoint
  remaining admin sets"); revised 14 October 2026 for issue #10 on the
  evidence of the `Authority` model, absorbing the former ADR 0062 (a removal
  revokes what the removed device signed that the removal had not seen).
  Replaces the merge rule of ADR 0009 for `roster.toml`; the roster stays in
  the tree. Withdraws the merge policy of 09 §9.7.

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

Forks also let a removed device keep signing. 04 §4.4 judges a commit by the
rosters at its parents, and a clean merge may be signed by a device of any
parent's roster, so a stolen laptop can sign on an old parent after its
removal. 09 §9.7 tried to narrow that with a policy: `pigeond` never merges
into `main` a commit signed by a device absent from its `main`'s roster. The
`Authority` model ([13 §13.2](../spec/13-verification.md)), which gives a
removed laptop to an attacker, shows that the policy does not stop it (the
laptop merges its backdated change with the removal itself, and a device that
has not seen the removal fast-forwards: `Authority_tip`), that it leaves
accepted commits out of `main` so that devices with the same commits show
different trees (`Authority_tipquiet`, `Authority_tiplive`), that the rule
alone lets the change in (`Authority_none`), and that the laptop can add a
fresh key for its person (ADR 0017) that its removal does not name
(`Authority_contentonly`).

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

**Revocation.** In a set of commits, an accepted commit *x* is *revoked* if
the set holds an accepted removal of *x*'s signer that does not have *x* as an
ancestor, or if *x*'s signer is not a device of the roster folded from *x*'s
ancestors in the set (so a key added by a revoked commit signs nothing that
counts). Revocation does not change the verdict of 04 §4.4, which stays
local; it acts where a history is read. The fold (04 §4.8) and the catalogue
([05 §5.2](../spec/05-catalogue-and-names.md), ADR 0052) take the accepted
commits that are not revoked; of a revoked commit only its device removals
apply, as with a skipped change, so two concurrent removals of each other's
devices both apply. Nothing is deleted: a revoked commit's change is shown as
a suggestion of its signer's person (04 §4.5, ADR 0038), and the People page
says "3 changes from *Martine's old laptop* arrived after it was removed".
`pigeond` merges every accepted commit: 09 §9.7's policy is withdrawn. *I lost
a computer* lists the devices the lost one added, and offers to remove them
with it.

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
- Once a removal is in a device's history, nothing the removed device signed
  that the removal had not seen reaches that device's catalogue or roster,
  whatever the order of fetches and merges (`Authority_revoke`:
  `RemovedSilenced`, `RemovalRevokesAdded`), and devices that stop fetching
  converge (`ConvergedWhenQuiet`, `Converge` in `Authority_live`). An honest
  lost laptop's last unseen changes become suggestions that someone decides;
  removing one's own old laptop after it has synced loses nothing. Two
  branches that remove each other's devices also revoke each other's unseen
  work; recovery is ADR 0047.
- The catalogue stays a function of a set of commits, so its merge laws hold,
  but it is no longer monotone: a removal arriving can take a change out of
  it, and a file can leave a laptop. Install treats this as a deletion by the
  group; the content stays in history and with the custodians (INV-History).
  Each removal costs one ancestry test per commit of that device
  (`git merge-base --is-ancestor`, or generation numbers).
- Laws (property tests, ADR 0012): R depends only on the ancestor set
  (merge order, duplicate delivery, and split and rejoin cycles do not
  matter); R of a linear history is sequential application; every R has an
  admin; a removed device or `annex_uuid` never returns; a change applies at
  most once; a skipped change is listed. The `Roster` TLA+ model checks the
  concurrent self-retirement, rotation of a device key during a concurrent
  removal, and merge orders, and shows ADR 0009's rule deadlocking
  (13 §13.2); `Authority` checks revocation with a smaller roster.

## Alternatives considered

- *Union of the admin sets on conflict.* Symmetric in the example, but it
  needs a rule for every pair of fields, and it can bring back a demoted
  admin. Rejected for the single rule of the fold.
- *Last writer wins per field.* It silently overwrites a concurrent decision.
  Rejected: a conflicting `set` is skipped and shown.
- *A CRDT document* (ADR 0009, option 4). The fold is one, with commits as
  operations. A library would add a format beside git, which ADR 0009
  rejected.
- *Keep 09 §9.7's merge policy*, or *a time cut-off* on a removed device's
  commits: the model lets the change in through a device that has not seen
  the removal, and author times are the signer's own claim.
- *Revoke content but not keys* (`Authority_contentonly`): the key the laptop
  added keeps signing.
- *An online authority or a threshold of admins*: v1 has no server and no
  quorum (ADR 0001).
