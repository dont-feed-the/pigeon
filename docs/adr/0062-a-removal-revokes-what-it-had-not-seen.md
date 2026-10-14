# 0062. A removal revokes what the removed device signed that the removal had not seen

- **Status:** Proposed, 14 October 2026, by the agent, for issue #10
  (Guillaume's review: "Add models and trace vocabulary for capture/install,
  authorization, purge, and repair"), on the evidence of the `Authority`
  model. Refines ADR 0051 (the fold) and replaces the merge policy of 09 §9.7,
  which had no ADR.

## Context

04 §4.4 judges a commit by the rosters at its parents, so that the verdict
depends only on the commit and its ancestors. ADR 0051 lets a device of
*any* parent's roster sign a clean merge. 09 §9.7 says that a removed device
can still sign commits on an old parent. To narrow the damage, it adds a
policy: `pigeond` never merges into `main` a commit signed by a device absent
from its `main`'s roster, and keeps it as a proposal for the admins.

The `Authority` model ([13 §13.2](../spec/13-verification.md)) gives c's
laptop to an attacker once the admin has removed it. The attacker signs
anything on any parent it knows. The model shows four things.

- **The policy does not stop it** (`Authority_tip`, 6 states). The removed
  laptop signs a change on the genesis. Then it signs a clean merge of that
  change with the removal itself. The merge is valid, because the laptop's
  own branch still has it in its roster (ADR 0051). b's laptop has not yet
  seen the removal, so it fast-forwards to that merge. Its `main` now holds
  both the removal and the removed laptop's change.
- **The policy contradicts the rule** (`Authority_tipquiet`,
  `Authority_tiplive`). A device that has seen the removal never merges an
  accepted commit. Its `main` then never reaches the catalogue of the
  commits it has, and devices that have the same commits show different
  trees. "Kept as a proposal" also contradicts 04 §4.5, where a proposal is a
  commit that §4.4 does not accept.
- **The rule alone lets it in** (`Authority_none`). The admin's own device
  merges the backdated change after the removal.
- **The removed device can add a key that survives its removal**
  (`Authority_contentonly`). ADR 0017 lets a device of P add devices of P, so
  the laptop can add a fresh key for c on an old parent. The removal does not
  name that key, so it survives.

02 §2.2 leaves stolen keys outside the guarantees, and 09 §9.1 says that a
stolen laptop "can sign until it is removed". Under today's rule it can sign
after its removal too, and forever.

## Decision

- **Revoked commits.** In a set of commits, an accepted commit *x* is
  *revoked* in either case:
  - the set holds an accepted removal of *x*'s signer that does not have *x*
    as an ancestor;
  - *x*'s signer is not a device of the roster folded from *x*'s ancestors in
    the set. So a key added by a revoked commit signs nothing that counts.
- **The verdict stays local.** Revocation does not change the verdict of
  04 §4.4: a commit is accepted or not by itself and its ancestors.
  Revocation acts where a history is read:
  - the catalogue ([05 §5.2](../spec/05-catalogue-and-names.md), ADR 0052)
    and the roster fold (04 §4.8, ADR 0051) take the accepted commits that
    are not revoked;
  - of a revoked commit, only its device removals apply, as with a skipped
    change (ADR 0051). So two concurrent removals of each other's devices
    still both apply.
- **Nothing is deleted.** A revoked commit's change is shown as a suggestion
  of its signer's person ([04 §4.5](../spec/04-roster-and-authority.md)),
  decided by its deciders as any suggestion is. The People page says "3
  changes from *Martine's old laptop* arrived after it was removed".
- **pigeond merges every accepted commit.** 09 §9.7's merge policy is
  withdrawn.
- **The removal names what it can see.** *I lost a computer* lists the
  devices that the lost one added, and offers to remove them with it.

## Consequences

- Once a removal is in a device's history, nothing the removed device signed
  that the removal had not seen reaches that device's catalogue or roster.
  That holds on every device, whatever the order of fetches and merges.
  `Authority_revoke` checks it (`RemovedSilenced`, `RemovalRevokesAdded`),
  and devices that stop fetching and merging still converge
  (`ConvergedWhenQuiet`, and `Converge` in `Authority_live`).
- An honest lost laptop's last changes, if no one had seen them before the
  removal, become suggestions that someone decides. Removing one's own old
  laptop after it has synced loses nothing.
- Two branches that remove each other's devices (ADR 0051) also revoke each
  other's unseen work. Both removals still apply, and recovery is ADR 0047.
- The catalogue is still a function of a set of commits, so the merge laws
  (commutative, associative, idempotent) hold. It is no longer monotone:
  adding a removal to a set can take a change out of its catalogue. A file
  can then leave a laptop when a removal arrives. Install treats this as a
  deletion by the group, and the content stays in history and with the
  custodians (INV-History).
- Each removal of a device costs one ancestry test per commit of that device
  (`git merge-base --is-ancestor`, or generation numbers). Removals are rare.
- The `Roster` model folds without revocation; `Authority` checks revocation
  with a smaller roster (the owner of a folder, and devices).
- When ADR 0051 is accepted, its status line should gain "fold refined by
  ADR 0062".

## Alternatives considered

- **Keep 09 §9.7's policy**, for the head or for every commit: the model
  shows that it lets the change in through a device that has not yet seen the
  removal, and that it leaves accepted commits out of `main`.
- **A time cut-off** (refuse a removed device's commits whose author time is
  after the removal): author times are the signer's own claim, and the
  attacker backdates.
- **Revoke content but not keys** (`Authority_contentonly`): the key that the
  laptop added keeps signing.
- **Removals that list the commits they accept**: that is the set of the
  removal's ancestors, which needs no list.
- **An online authority or a threshold of admins**: v1 has no server and no
  quorum (ADR 0001).
