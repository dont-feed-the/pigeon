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
group = "Famille Geiger"            # display name
k = 2                               # custody copies (ADR 0002)
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

[[folders]]
path = "Paul"                       # a top-level folder, or a deeper one
rule = "owned"
owner = "paul"

[[folders]]
path = "Lab data"
rule = "add-only"
stewards = ["roy"]
```

Validity of a roster (checked on every commit that changes it): it parses, its
`format` is known, at least one admin remains, every device names an existing
person, device names and person IDs are unique, `k ≥ 1`, every owner and steward
is a person. A commit that leaves an invalid roster is invalid.

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
| `roster.toml`, anything else | an admin |
| `decisions/<commit>.toml` with `verdict = "rejected"` | a decider of that proposal commit (§4.5, ADR 0038) |
| `decisions/<commit>.toml` with `verdict = "withdrawn"` | a device of the proposal's author (ADR 0038) |
| `decisions/purge-<id>.toml` | an admin (ADR 0018, ux.md §7 `pigeon erase`) |
| `records/drop-<id>.toml` | any member, about their own device ([08 §8.7](08-custody-loops.md)) **[no ADR]** |
| anything else | an admin |

ADR 0009 says only admins change `.pigeon/`; ADR 0017 refined it for device
entries and ADR 0038 (Proposed) for decisions on suggestions, so that an owner or
steward who is not an admin can decide in their scope and nothing else. Each row
is checked under the roster at the commit's parents, like every rule of §4.4.

## 4.4 The authority rule

All functions below are pure and live in the core (ADR 0012).

- `R(p)` is the roster in the tree of commit *p*.
- `signer(c)` is the device whose key made the SSHSIG signature (namespace
  `git`) in the `gpgsig` header of *c*, verified by pigeon with the `ssh-key`
  crate (ADR 0007); `person(c)` is that device's person. Unsigned commits are
  invalid. git's own `allowedSignersFile` and commit dates play no part.
- `merge(t₁, …, tₙ)` is the deterministic tree merge of [05 §5.2](05-catalogue-and-names.md).

For a commit *c* other than the genesis, let **A(c)** be its parents that are
accepted and **X(c)** the others. *c* is **accepted** if and only if:

1. A(c) is not empty;
2. `signer(c)` is a device of `R(p)` for every *p* in A(c);
3. with base `B = merge(tree(p) for p in A(c))` and `Δ = diff(B, tree(c))`, every
   change of Δ is allowed for `person(c)` under `R(p)` for every *p* in A(c)
   (§4.3), or *c* is a valid Force (§4.6);
4. if Δ changes the roster, the new roster is valid (§4.2).
5. every path that Δ adds is portable ([05 §5.4](05-catalogue-and-names.md)).

Consequences, each a law with a property test (ADR 0012):

- A clean merge of accepted heads has Δ = ∅, so any member's device may sign it.
- **Adoption:** a decision merge has an accepted parent and a proposal parent in
  X(c); its Δ is the proposal's change, valid when the decider may make it. The
  proposal stays outside the catalogue; its content enters through the decision.
- A commit mixing allowed and disallowed changes is rejected as a whole (P4);
  capture never writes one, because it splits ([07 §7.3](07-capture-and-install.md)).
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
- **Deciders** (ADR 0038). The persons allowed to make every change of the
  proposal's diff under the roster at its parents (§4.3); if nobody may make
  them all, the admins. Capture splits a proposal per set of deciders, so only a
  proposal found in a peer's history can fall to the admins this way.
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

- **Accept.** The decider's device writes a merge with parents (`main`, the
  proposal) and tree `merge(main, proposal)`; if the path changed meanwhile on
  `main`, both versions are kept under conflict names ([05 §5.3](05-catalogue-and-names.md)).
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
validity) or the `.pigeon/` table. The CLI asks before forcing and the web
interface says exactly what changes (ux.md §3, principle 5). A forced commit is
shown in the file's history as "admin override by Roy".

The proposal (§2) let *any member* force; ux.md names Force "admin override".
This specification follows ux.md, the later decision **[no ADR]**.

## 4.7 Changing the roster

- Every roster change is a signed commit checked by §4.4 (ADR 0009). The CLI and
  the web interface are the only writers; a hand edit of `roster.toml` on a
  laptop is captured like any file and, from a non-admin, becomes a proposal.
- **Devices** (ADR 0017): adding a device of person P, or removing one, is
  allowed to an admin or to a device of P. Adding a person needs an admin.
- **Concurrent roster changes** merge like any file. Two edits of the same TOML
  table are a conflict that the merge cannot keep as two files; the merge then
  keeps the side of the commit that sorts first ([05 §5.3](05-catalogue-and-names.md))
  and the device that merges proposes the other side to the admins **[no ADR]**.
  Per ADR 0009, a merge commit that changes `roster.toml` relative to its base
  needs an admin signature valid under both parents' rosters.
- **Removing an admin** is allowed to another admin; the last admin cannot be
  removed (§4.2).
- A roster change that changes K, a device's role or `maxsize` re-applies
  git-annex settings ([03 §3.5](03-repository-and-roles.md)).
