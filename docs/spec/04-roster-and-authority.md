# 4. Roster and authority

> Mission: define the roster file, the rule that decides which commits every
> device accepts (P4), and how proposals, decisions and Force work. The rule is a
> pure function of history (ADR 0009); it is the heart of pigeon's core.

## 4.1 Genesis

`pigeon create NAME` writes the **genesis commit**: no parent, a tree holding
`.gitattributes` ([03 §3.3](03-repository-and-roles.md)) and `.pigeon/roster.toml`
with one person (the creator, admin) and one device (this one), signed by this
device's key (ADR 0007). The **group ID** is the genesis commit ID. A device
accepts a history only if its root is the genesis named by its invitation
(ADR 0017); other founders join by invitation and are made admins by a roster
change.

## 4.2 The roster file

`.pigeon/roster.toml`, a plain git blob (ADR 0009), `format = 1` (ADR 0036). The
schema below is normative **[no ADR: field names]**; the content of each field
comes from the ADRs cited.

```toml
format = 1
rules = 1                           # the rule set the group runs; never decreases (ADR 0048)
group = "Famille Geiger"            # display name
k = 2                               # custody copies, in distinct failure domains (ADRs 0002, 0049)
min_sites = 1                       # optional: declared sites the copies must span (ADR 0049)
retention_days = 30                 # waiting period before a purge (ADR 0018)
relay = "https://relay.example.org" # optional group relay (ADR 0016)
telemetry_device = "…"              # optional device ID of the collector (ADR 0025)

[persons.paul]                      # person ID: lowercase ASCII, immutable
name = "Paul"
admin = true

[persons.martine]
name = "Martine"

[devices."3f9a0c1d…64 hex"]         # device ID = ed25519 public key (ADR 0007)
person = "paul"
name = "Paul's laptop"
role = "laptop"
annex_uuid = "…"                    # its git-annex repository UUID (03 §3.5)

[devices."b71e…"]
person = "paul"
name = "Paul's NAS"
role = "custodian"
annex_uuid = "…"
maxsize = "4 TB"                    # capacity for sizebalanced (ADR 0003)
domain = "paul-server"              # optional: hardware it shares with other custodians (ADR 0049)
site = "Paul's home"                # optional: where it stands (ADR 0049)

[devices."9c04…"]
person = "paul"
name = "Paul's recovery sheet"
role = "recovery"                   # a key on paper: no annex_uuid, never online (ADR 0047)

[[folders]]
path = "Paul"                       # a top-level folder, or a deeper one
rule = "owned"
owner = "paul"

[[folders]]
path = "Lab data"
rule = "add-only"
stewards = ["roy"]
```

Validity of a roster (checked on every commit that changes it): it parses,
`rules` is not lower than at the parents, at least one admin remains, every device names an existing
person, device names and person IDs are unique, `k ≥ 1`, every owner and steward
is a person, `min_sites ≥ 1`, and only custodians have `domain` or `site`,
each a non-empty label. Every laptop and custodian has an `annex_uuid`, a recovery device
has none, and an `annex_uuid` belongs to one device only: it is never given to
another device, even after its own was removed (ADR 0046). A device ID that
was removed is never added again (ADR 0051). A commit that leaves an invalid
roster is invalid.

## 4.3 Folder rules

The rule of a path is the entry of `[[folders]]` with the **longest `path`
prefix** of it, compared on portable names ([05 §5.4](05-catalogue-and-names.md))
**[no ADR: longest prefix]**. A path with no entry, including files at the root
of the tree, is *add-only* with the admins as stewards **[no ADR: default rule]**.

For a person *u* and a change of one path, under roster R:

| Change | owned by P | add-only, stewards S |
|---|---|---|
| add (path absent before) | *u* = P | any member |
| modify, delete | *u* = P | *u* ∈ S (the admins if S is empty) |
| rename | a delete and an add: both must be allowed | same |

`.pigeon/` follows its own table (ADRs 0009, 0017, 0018, 0038):

| Path under `.pigeon/` | Who may add or change it |
|---|---|
| `roster.toml`, device entries of person P | an admin, or a device of P (ADR 0017) |
| anything, from a device with `role = "recovery"` | only device entries of its own person (ADR 0047) |
| `roster.toml`, anything else | an admin |
| `decisions/<commit>.toml` with `verdict = "rejected"` | a decider of that proposal commit (§4.5, ADR 0038) |
| `decisions/<commit>.toml` with `verdict = "withdrawn"` | a device of the proposal's author (ADR 0038) |
| `decisions/purge-<id>.toml` | an admin (ADRs 0018, 0042, ux.md §7 `pigeon erase`) |
| `records/drop-<id>.toml` | any member, about their own device ([08 §8.7](08-custody-loops.md)) **[no ADR]** |
| anything else | an admin |

ADR 0009 says only admins change `.pigeon/`; ADR 0017 refined it for device
entries and ADR 0038 (Proposed) for decisions on suggestions, so that an owner or
steward who is not an admin can decide in their scope and nothing else. Each row
is checked under the roster at the commit's parents, like every rule of §4.4.
A device with `role = "recovery"` may add or remove devices of its own person
and nothing else, in `.pigeon/` or outside it, even when that person is an
admin; its commits are never a Force (§4.6, ADR 0047).

## 4.4 The authority rule

All functions below are pure and live in the core (ADR 0012).

- `R(p)` is the roster in force for the children of commit *p*: the fold of
  §4.8, which the tree of *p* holds as `.pigeon/roster.toml` (ADR 0051).
- `signer(c)` is the device whose key made the SSHSIG signature (namespace
  `git`) in the `gpgsig` header of *c*, verified by pigeon with the `ssh-key`
  crate (ADR 0007); `person(c)` is that device's person. Unsigned commits are
  invalid. git's own `allowedSignersFile` and commit dates play no part.
- `anc*(p)` is *p* and its accepted ancestors, and `catalogue(S)` is the tree
  of the logical catalogue of such a set ([05 §5.2](05-catalogue-and-names.md),
  ADR 0052): a function of the history, not of the parents' trees alone.

For a commit *c* other than the genesis, let **A(c)** be its parents that are
accepted and **X(c)** the others. *c* is **accepted** if and only if:

1. A(c) is not empty;
2. `signer(c)` is a device of `R(p)` for every *p* in A(c), or, when Δ = ∅
   (rule 3), for some *p* in A(c) (ADR 0051);
3. with base `B = catalogue(⋃ anc*(p) for p in A(c))` and `Δ = diff(B, tree(c))`,
   every change of Δ is allowed for `person(c)` under `R(p)` for every *p* in
   A(c) (§4.3), or *c* is a valid Force (§4.6). With one accepted parent *p*,
   B is `tree(p)`, because every accepted tree is the catalogue of its history;
4. if Δ changes the roster, the new roster is valid (§4.2).
5. every path that Δ adds is portable ([05 §5.4](05-catalogue-and-names.md)).
6. `tree(c)` is **well formed** (ADR 0044): only folders and files of mode
   100644 or 100755; outside `.pigeon/` and the root `.gitattributes`, each
   file is a git-annex pointer to a `SHA256E` key with its size; inside them,
   a plain blob ([07 §7.9](07-capture-and-install.md)).
7. every parent of *c* outside A(c) is a proposal (§4.5) that *c* adopts: the
   signer of *c* is one of its deciders under R(p) for every *p* in A(c), and
   `tree(c)` holds the proposal's change (ADR 0063).

A commit that breaks rule 6 is **malformed**: it is neither accepted nor kept as
a proposal, and its keys are not references (ADR 0042).

A commit is **unknown** to a release when the roster at one of its parents has a
`rules` it does not support, or when a `.pigeon/` document of its tree has a
`format`, or a field, that release does not know: every schema is closed
(ADR 0048). An unknown commit and its descendants are neither accepted nor
proposals; they stay as fetched and are judged again after an upgrade.
Compatibility law: the verdict of an older release is that of a newer one, or
*unknown*. Every tree capture
makes is well formed, so only a faulty or hostile device can write one.

Consequences, each a law with a property test (ADR 0012):

- A clean merge, whose tree is the catalogue of its history, has Δ = ∅. It is
  not a contribution, so it never counts in the catalogue
  ([05 §5.1](05-catalogue-and-names.md)), and a device of the roster of any
  of its parents may sign it, even one that another parent's roster removed:
  the merge changes nothing, and its roster, the fold, removes that device.
  Requiring the signer in every parent's roster leaves two branches that
  removed each other's devices with no one to merge them (`Roster_every`).
- **Adoption:** a decision merge has an accepted parent and a proposal parent in
  X(c); its Δ is the proposal's change, valid when the decider may make it
  (rule 7). A merge with a proposal parent that does not adopt it is a
  proposal itself, so it cannot mark a suggestion accepted. The
  proposal stays outside the catalogue; its content enters through the decision.
- A commit mixing allowed and disallowed changes is rejected as a whole (P4);
  capture never writes one, because it splits ([07 §7.3](07-capture-and-install.md)).
- **Revoked commits** (ADR 0062). In a set of commits, an accepted commit *x*
  is revoked when the set holds an accepted removal of *x*'s signer that does
  not have *x* as an ancestor, or when *x*'s signer is not a device of the
  roster folded from *x*'s ancestors in the set. The catalogue
  ([05 §5.2](05-catalogue-and-names.md)) and the fold (§4.8) take the
  accepted commits that are not revoked; of a revoked commit only the device
  removals apply. A revoked commit stays accepted, and its change is shown as
  a suggestion of its signer's person (§4.5). So a removed device's commits
  that its removal had not seen count nowhere, whichever device merges them,
  and neither do the keys they added ([09 §9.7](09-network-and-joining.md)).
- **Locality:** the verdict of *c* depends only on *c* and its ancestors. Hence
  `accepted(S ∪ T) = accepted(S) ∪ accepted(T)` for ancestor-closed sets S, T, and
  two devices with the same commits accept the same catalogue.
- A roster change takes effect for the *children* of the commit that makes it:
  a commit is judged by the roster at its parents, never by its own.

## 4.5 Proposals

A **proposal** is a signed commit that §4.4 does not accept. Proposals are made
by capture (a person saved a change their rules do not allow) or found in a
peer's history (a commit rejected here). Each is kept as
`refs/pigeon/proposals/<signer-device-id>/<commit>` and replicated (ADR 0009).

- **Shape.** Capture writes one proposal per capture batch and set of deciders,
  whose parent is the `main` it was captured on. A later save of a path already in
  a pending proposal of this device makes a new proposal whose parent is that
  proposal, so the chain keeps every captured save (P2) and people see one
  suggestion that was updated (ADR 0034) **[no ADR: shape]**.
- **Deciders** (ADRs 0038, 0063), for a decision on `main` *m*: the persons
  allowed (§4.3) to make every change of the adoption on *m* (the diff
  between *m* and the tree of *Accept* below) under R(*m*); if nobody may, the
  admins, who adopt with Force (§4.6). Adoption and rejection use this one
  set, recomputed when `main` moves, so the deciders shown are those of now.
  Capture splits a proposal per set of deciders, so only a proposal found in
  a peer's history, or one overtaken by `main`, falls to the admins this way.
- **Revisions.** A decision names one proposal commit and applies to that
  revision only. A later revision of the same chain is pending until decided
  itself; adopting a revision adopts its ancestors in the chain, and rejecting
  one leaves its later revisions pending.
- **State,** computed from the accepted catalogue, never stored:

| State | When |
|---|---|
| pending | neither adopted nor decided |
| accepted | an accepted commit has it as an ancestor (adoption) |
| rejected | `.pigeon/decisions/<commit>.toml` says `rejected`, with an optional note |
| withdrawn | the same file says `withdrawn`, written by its author ([07 §7.6](07-capture-and-install.md)) |
| superseded | a later proposal of the same chain exists |

  The state is the first row that holds, in the order accepted, rejected,
  withdrawn, superseded, pending (ADR 0063). The commits that a removal
  revokes (§4.4) are shown with the suggestions of their signer's person and
  decided in the same way.

- **Accept.** On a `main` that holds every accepted commit the proposal builds
  on (pigeond merges them first), the decider's device writes a merge with
  parents (`main`, the proposal) whose tree is `main`'s with the proposal's
  change (rule 7 of §4.4). If the path changed meanwhile on `main`, both
  versions are kept: the proposal's under its conflict name, with the
  proposal's author and time ([05 §5.2–5.3](05-catalogue-and-names.md)).
- **Reject.** The decider's device commits `.pigeon/decisions/<commit>.toml`:

```toml
format = 1
proposal = "<commit id>"
verdict = "rejected"        # or "withdrawn"
note = "On garde la recette de Mamie."
```

- Accept and reject are symmetric and both signed. Concurrent decisions on one
  proposal resolve deterministically: an accepted commit adopting it wins over a
  rejection, because content is never thrown away silently (ADR 0038).
- The proposal commit stays the author's signed **submission**; the adoption
  merge, signed by a decider, is the **authorised change**. Validation never
  accepts the submission itself, so its signer's rights never widen.
- Proposals and their content replicate like everything else, so the deciders
  have them before they decide, and custodians keep their content like any
  version. A suggestion's content has the state of any version: *local*, then
  *published*, and *protected* (shown *Safe*) only once K custodians hold
  verified copies (ADR 0034, [06 §6.2](06-status.md)).

## 4.6 Force

Force is the admin override (ux.md §2.2): a commit that §4.4 would reject only
because of folder rules, accepted because it carries the trailer
`Pigeon-Force: <reason>` and its signer is an admin under the rosters of its
accepted parents. Force never waives rule 2 (signature), rule 4 (roster
validity), rule 6 (well-formed tree) or the `.pigeon/` table. The CLI asks before forcing and the web
interface says exactly what changes (ux.md §3, principle 5). A forced commit is
shown in the file's history as "admin override by Roy". An admin's adoption
of a suggestion that nobody else may make is a Force (§4.5, ADR 0063).

The proposal (§2) let *any member* force; ux.md names Force "admin override".
This specification follows ux.md, the later decision **[no ADR]**.

## 4.7 Changing the roster

- Every roster change is a signed commit checked by §4.4 (ADR 0009). The CLI and
  the web interface are the only writers; a hand edit of `roster.toml` on a
  laptop is captured like any file and, from a non-admin, becomes a proposal.
- **Devices** (ADR 0017): adding a device of person P, or removing one, is
  allowed to an admin or to a device of P. Adding a person needs an admin.
- **Concurrent roster changes** may fork, and are never merged as a file:
  the roster of a merge is the fold of §4.8 (ADR 0051), which replaces ADR
  0009's rule that such a merge needs an admin valid under both parents.
- **Removing an admin** is allowed to another admin; the last admin cannot be
  removed (§4.2).
- A custodian's `maxsize`, `domain` and `site` may be changed by an admin or
  by a device of its person; `min_sites` needs an admin (ADR 0049).
- A roster change that changes K, a device's role or `maxsize` re-applies
  git-annex settings ([03 §3.5](03-repository-and-roles.md)). `domain`,
  `site` and `min_sites` are pigeon's alone: git-annex does not see them.

## 4.8 Roster forks: the fold (ADR 0051)

Roster changes need no online authority, so two branches can change the
roster concurrently. Their merge needs no admin: a device of either side may
sign it, and it recomputes the roster.

- **Change.** Each accepted commit *c* carries δ(c), the difference between
  the fold of its accepted parents and the `roster.toml` of its tree. A clean
  merge has none; a decision merge carries the change it adopts (§4.5). Its
  edits are `set` a field (from the value its author saw to a new one),
  `add` an entry, `remove device` D, and `remove person` P (with P's devices
  and P's steward roles). δ(c) is judged once, by §4.4, under its parents'
  rosters.
- **Canonical order.** The accepted commits that are *c* or its ancestors,
  topologically, and among those whose ancestors are all placed, the smallest
  (recorded author time, device ID, commit ID) first: the order of
  representatives of [05 §5.2](05-catalogue-and-names.md).
- **Fold.** R(c) applies the changes in that order, from the genesis roster.
  A change applies as a whole when every `set` finds the value its author saw
  or already the new one, every `add` finds its entry absent or equal, and
  the result is valid (§4.2). Otherwise it is **skipped**, except for its
  device removals, which always apply (a `remove person` removes the devices
  the person has at that point). A removal of what is gone is a no-op.
  The change of a revoked commit (§4.4, ADR 0062) is skipped the same way:
  only its device removals apply.
- **Skipped changes** are listed with their reason, a pure function of the
  history. The admins and the author see *Needs you*: "Alice and Bob gave up
  their admin role at the same time. One admin must remain: Bob stays
  admin." *Apply again* writes a new change, judged under today's roster.

For a linear history the fold is the same as applying each commit in turn,
and the change of *c* always applies in R(c), so the tree of every accepted
commit holds R(c). Laws: R(c) depends only on the set of *c*'s accepted
ancestors, not on the order of merges or deliveries; every R(c) has an admin;
a removed device never returns; a change applies at most once. An
implementation folds from genesis (or caches by canonical prefix): replaying
the other side's changes onto one's own roster depends on who merges
(`Roster_rebase`). Removing a device always takes effect; removing a person
is best effort across forks, since of two concurrent removals that would
leave no admin the later is skipped (`Roster_person`). The `Roster`
model ([13 §13.2](13-verification.md)) checks the concurrent
self-retirement, a key rotation during a concurrent removal, and the orders
of merges, and shows ADR 0009's rule deadlocking.
