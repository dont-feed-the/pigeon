# 4. Roster and authority

> Mission: the roster file, the rule that decides which commits every device
> accepts (P4), proposals, decisions and Force. The rule is a pure function of
> history (ADR 0009). Normative models: `Authority.tla` for acceptance and
> decisions, `Roster.tla` and `pigeon-core::roster` for the fold (§4.8).

## 4.1 Genesis

`pigeon create NAME` writes the **genesis commit**: no parent, a tree with
`.gitattributes` ([03 §3.3](03-repository-and-roles.md)) and a roster of one
admin person and one device, signed by this device (ADR 0007). The **group
ID** is its commit ID. A device accepts a history only if its root is the
genesis its invitation names (ADR 0017); other founders join by invitation
and are made admins by a roster change.

## 4.2 The roster file

`.pigeon/roster.toml`, a plain git blob (ADRs 0009, 0036). Field names are
normative **[no ADR: field names]**:

```toml
format = 1
rules = 1                           # the rule set the group runs; never decreases (ADR 0048)
group = "Famille Geiger"            # display name
k = 2                               # custody copies, on distinct custodians (ADRs 0002 and 0046)
retention_days = 30                 # waiting period before a purge (ADR 0018)
relay = "https://relay.example.org" # optional group relay (ADR 0016)
telemetry_device = "…"              # optional device ID of the collector (ADR 0025)

[persons.paul]                      # person ID: lowercase ASCII, immutable
name = "Paul"
admin = true

[devices."3f9a0c1d…64 hex"]         # device ID = ed25519 public key (ADR 0007)
person = "paul"
name = "Paul's laptop"
role = "laptop"                     # or "custodian" (with maxsize = "4 TB", ADR 0003)
annex_uuid = "…"                    # its git-annex repository UUID (03 §3.5)

[devices."9c04…"]
person = "paul"
name = "Paul's recovery sheet"
role = "recovery"                   # a key on paper: no annex_uuid, never online (ADR 0047)

[[folders]]
path = "Lab data"                   # a top-level folder, or a deeper one
rule = "add-only"                   # or rule = "owned", owner = "paul"
stewards = ["roy"]
```

A roster is **valid** when it parses, `rules` is not lower than at the
parents, an admin remains, every device names an existing person, device
names and person IDs are unique, `k ≥ 1`, owners and stewards are persons,
every laptop and custodian has an `annex_uuid` that no other device ever had
(ADR 0046), a recovery device has none, and no removed device ID returns
(ADR 0051). A commit that leaves an invalid roster is invalid.

## 4.3 Folder rules

The rule of a path is the `[[folders]]` entry with the longest `path` prefix
of it, on portable names ([05 §5.4](05-catalogue-and-names.md)); a path with
no entry, root files included, is *add-only* with the admins as stewards
**[no ADR: longest prefix, default rule]**.

| Change by person *u* | owned by P | add-only, stewards S |
|---|---|---|
| add (path absent before) | *u* = P | any member |
| modify, delete | *u* = P | *u* ∈ S (the admins if S is empty) |
| rename | a delete and an add, both allowed | same |

`.pigeon/` has its own table (ADRs 0009, 0017, 0018, 0047, 0051):

| Path under `.pigeon/` | Who may add or change it |
|---|---|
| device entries of person P in `roster.toml` | an admin, or a device of P (ADR 0017) |
| anything, from a device with `role = "recovery"` | only device entries of its own person, even if an admin; never a Force (ADR 0047) |
| `decisions/<commit>.toml`, `verdict = "rejected"` | a decider of that proposal (§4.5, ADR 0051) |
| `decisions/<commit>.toml`, `verdict = "withdrawn"` | a device of the proposal's author |
| `decisions/purge-<id>.toml` | an admin (ADR 0018, [08 §8.6](08-custody-loops.md)) |
| `records/drop-<id>.toml` | any member, about their own device ([08 §8.7](08-custody-loops.md)) **[no ADR]** |
| `roster.toml`, anything else | an admin |

Each row is checked under the roster at the commit's parents.

## 4.4 The authority rule

Pure functions of the core (ADR 0012):

- `R(p)`: the roster in force for the children of *p*, the fold of §4.8, which
  `tree(p)` holds as `.pigeon/roster.toml`.
- `signer(c)`: the device whose SSHSIG signature (namespace `git`) is in the
  `gpgsig` header of *c*, verified with the `ssh-key` crate (ADR 0007);
  `person(c)` is its person. Unsigned commits are invalid; git's
  `allowedSignersFile` and commit dates play no part.
- `catalogue(S)`: the logical catalogue of an ancestor-closed set of accepted
  commits ([05 §5.2](05-catalogue-and-names.md)).

For a commit *c* other than the genesis, with **A(c)** its accepted parents
and **X(c)** the others, *c* is **accepted** iff:

1. A(c) is not empty;
2. `signer(c)` is a device of `R(p)` for every *p* in A(c), or, when Δ = ∅,
   for some *p* in A(c) (ADR 0051);
3. with `B = catalogue(⋃ anc*(p), p ∈ A(c))` and `Δ = diff(B, tree(c))`, every
   change of Δ is allowed for `person(c)` under `R(p)` for every *p* in A(c)
   (§4.3), or *c* is a valid Force (§4.6); with one accepted parent, B = tree(p);
4. if Δ changes the roster, the new roster is valid (§4.2);
5. every path Δ adds is portable ([05 §5.4](05-catalogue-and-names.md));
6. `tree(c)` is **well formed** ([07 §7.9](07-capture-and-install.md)): only
   folders and files of mode 100644 or 100755; outside `.pigeon/` and the root
   `.gitattributes`, each file a git-annex pointer to a `SHA256E` key with its
   size; inside them, plain blobs;
7. every parent in X(c) is a proposal (§4.5) that *c* adopts: `signer(c)` is
   one of its deciders under R(p) for every *p* in A(c), and `tree(c)` holds
   the proposal's change (ADR 0051).

A commit breaking rule 6 is **malformed**: neither accepted nor a proposal,
and its keys are not references ([08 §8.6](08-custody-loops.md)). A commit is
**unknown** to a release when a parent's roster has a `rules` it does not
support, or a `.pigeon/` document has a `format` or field it does not know
(every schema is closed); it and its descendants stay as fetched, judged again
after an upgrade. An older release's verdict is a newer one's, or *unknown*
(ADR 0048).

Consequences, checked by `Authority.tla` (`AuthorisedContent`,
`DecisionsByDeciders`, `AdoptionHoldsContent`, `RemovedSilenced`,
`RemovalRevokesAdded`, `Converge`, `ConvergedWhenQuiet`):

- A clean merge has Δ = ∅, counts in no catalogue ([05 §5.1](05-catalogue-and-names.md)),
  and a device of any parent's roster may sign it, so two branches that
  removed each other's devices can still merge (`Roster_every` shows the
  deadlock otherwise).
- **Adoption** is a merge with an accepted parent and a proposal parent; its
  Δ is the proposal's change. A merge with a proposal parent it does not
  adopt is itself a proposal.
- A commit mixing allowed and disallowed changes is rejected whole; capture
  splits, so never writes one ([07 §7.3](07-capture-and-install.md)).
- **Revoked commits** (ADR 0051): an accepted *x* is revoked in a set that
  holds an accepted removal of `signer(x)` not descending from *x*, or when
  `signer(x)` is not in the roster folded from *x*'s ancestors in the set. The
  catalogue and the fold skip it except for its device removals; it is shown
  as a suggestion of its signer's person. So nothing a removed device signed
  unseen by its removal counts, nor the keys it added ([09 §9.7](09-network-and-joining.md)).
- **Locality:** the verdict of *c* depends only on *c* and its ancestors, so
  `accepted(S ∪ T) = accepted(S) ∪ accepted(T)` for ancestor-closed S, T, and
  a roster change applies to the *children* of its commit.

## 4.5 Proposals

A **proposal** is a signed commit §4.4 does not accept: made by capture, or
found in a peer's history. Each is kept as
`refs/pigeon/proposals/<signer-device-id>/<commit>` (ADR 0009).

- **Shape [no ADR].** Capture writes one proposal per batch and set of
  deciders, on the `main` it captured on. A later save of a path in a pending
  proposal of this device makes a new proposal on top of it; people see one
  updated suggestion (ADR 0034).
- **Deciders** of a proposal on `main` *m* (ADR 0051): the persons allowed
  (§4.3) to make every change of its adoption on *m*; if nobody may, the
  admins, who adopt with Force. Recomputed when `main` moves.
- **Revisions.** A decision names one proposal commit. Adopting a revision
  adopts its ancestors in the chain; rejecting one leaves later ones pending.
- **State**, computed, never stored; the first that holds, in this order:
  *accepted* (an accepted commit descends from it), *rejected* or
  *withdrawn* (`.pigeon/decisions/<commit>.toml`, [07 §7.6](07-capture-and-install.md)),
  *superseded* (a later revision exists), *pending*. Revoked commits (§4.4) are
  shown and decided the same way.
- **Accept.** On a `main` holding every accepted commit the proposal builds
  on, the decider's device merges (`main`, proposal) with the proposal's
  change. If the path changed meanwhile, both versions are kept, the
  proposal's under its conflict name ([05 §5.2–5.3](05-catalogue-and-names.md)).
- **Reject.** The decider's device commits:

```toml
format = 1
proposal = "<commit id>"
verdict = "rejected"        # or "withdrawn"
note = "On garde la recette de Mamie."
```

- Concurrent decisions resolve deterministically: an adoption wins over a
  rejection (ADR 0051). The proposal stays the author's signed
  **submission**; the adoption, signed by a decider, is the **authorised
  change**, so a signer's rights never widen.
- Proposals replicate like everything else, and their content has the states
  of any version: *protected* only once K custodians hold it (ADR 0034,
  [06 §6.2](06-status.md)).

## 4.6 Force

Force is the admin override: a commit §4.4 rejects only because of folder
rules, accepted because it carries the trailer `Pigeon-Force: <reason>` and
its signer is an admin under the rosters of its accepted parents. It never
waives rules 2, 4 and 6 or the `.pigeon/` table. The CLI asks first; the web
interface says what changes; history shows "admin override by Roy". Only
admins force, as ux.md says, not any member as the proposal said **[no ADR]**.

## 4.7 Changing the roster

- Every roster change is a signed commit checked by §4.4 (ADR 0009); the CLI
  and the web interface write them. A hand edit of `roster.toml` is captured
  like any file, a proposal from a non-admin.
- Adding or removing a device of P: an admin or a device of P (ADR 0017).
  Adding a person or removing an admin: an admin; the last admin stays.
  A custodian's `maxsize`: an admin or a device of its person.
- Concurrent changes may fork; the roster of a merge is the fold of §4.8,
  never a file merge (ADR 0051).
- A change of K, a role or `maxsize` re-applies the git-annex settings
  ([03 §3.5](03-repository-and-roles.md)).

## 4.8 Roster forks: the fold (ADR 0051)

Normative: `pigeon-core::roster` with its laws (`tests/roster.rs`) and
`Roster.tla` (`TreeIsFold`, `Agreement`, `AdminRemains`, `RemovalWins`,
`PersonRemovalWins`, `SkipsNeedConcurrency`, `MergeAlwaysPossible`). In short:

- Each accepted commit carries a **change** δ(c) from the fold of its
  accepted parents to its `roster.toml`: edits `set` a field (from the value
  its author saw), `add` an entry, `remove device`, `remove person` (with
  their devices and steward roles). A clean merge has none; a decision merge
  carries the change it adopts. δ(c) is judged once, under its parents.
- **Canonical order:** topological, and among commits whose ancestors are
  placed, smallest (author time, device ID, commit ID) first, as in
  [05 §5.2](05-catalogue-and-names.md).
- **Fold:** from the genesis roster, a change applies whole when every `set`
  finds its old or new value, every `add` finds its entry absent or equal,
  and the result is valid; otherwise it is **skipped**, except its
  `remove device` edits, which always apply (`Removals = "always"`). A revoked
  commit's change is skipped the same way. The fold remembers every removed
  device and `annex_uuid`, even of one never added, so neither returns.
- **Laws:** the fold depends only on the set of ancestors; every result is
  valid with an admin; removals win; skips need concurrency; a linear history
  skips nothing. Replaying one side onto the other depends on who merges
  (`Roster_rebase`); of two concurrent removals that would leave no admin the
  later is skipped (`Roster_person`).
- **Skipped changes** are listed with their reason; the admins and the author
  see *Needs you* ("Alice and Bob gave up their admin role at the same time.
  One admin must remain: Bob stays admin."). *Apply again* writes a new change.
