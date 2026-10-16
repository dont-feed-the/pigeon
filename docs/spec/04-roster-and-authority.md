# 4. Roster and authority

> Mission: the roster file, the rule that decides which commits every device
> accepts (P4), proposals, decisions and Force. The rule is a pure function of
> history (ADR 0009). Normative models: `Authority.tla` for acceptance and
> decisions, `Roster.tla` and `pigeon-core::roster` for the fold (§4.8).

## 4.1 Genesis

`pigeon create NAME` writes the **genesis commit**: no parent, a tree with
`.gitattributes` ([03 §3.3](03-repository-and-roles.md)) and a roster of one
admin and one device, signed by it (ADR 0007). The admin is named as the
creator asked (`--person`, the web's *Your name*), else by `$USER`, else by
the computer's name; its ID is made from that name as a new person's is
([09 §9.6](09-network-and-joining.md)) **[no ADR]**. The **group ID** is its
commit ID; a device accepts only a history rooted at the genesis its
invitation names (ADR 0017). Other founders join by invitation.

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
role = "laptop"                     # or "custodian" (with maxsize = "4 TB", ADR 0003),
                                    # or "recovery": a key on paper, no annex_uuid, never online (ADR 0047)
annex_uuid = "…"                    # its git-annex repository UUID (03 §3.5)

[[folders]]
path = "Lab data"                   # a top-level folder, or a deeper one
rule = "add-only"                   # or rule = "owned", owner = "paul"
stewards = ["roy"]
```

A roster is **valid** when it parses, `rules` is not lower than at the
parents, and `pigeon-core::roster::Roster::validate` passes: an admin, known
persons, unique names, `k ≥ 1`, an `annex_uuid` never reused (ADR 0046) on
each laptop and custodian and none on a recovery device, no removed device
back (ADR 0051), and a `relay`, if any, an `https://` URL (`http://` for
one on the local network). A commit that leaves an invalid roster is invalid.

## 4.3 Folder rules

The rule of a path is the `[[folders]]` entry with the longest `path` prefix
of it, on portable names ([05 §5.4](05-catalogue-and-names.md)); a path with
no entry, root files included, is *add-only* with the admins as stewards
**[no ADR: longest prefix, default rule]**.

| Change by person *u* | owned by P | add-only, stewards S |
|---|---|---|
| add (path absent before) | *u* = P | any member |
| modify, delete | *u* = P | *u* ∈ S (the admins if S is empty), or *u* alone wrote it (ADR 0070) |
| rename | a delete and an add, both allowed | same |

*u* **alone wrote** *p* when every contribution that wrote or deleted *p*'s
slot in the base is *u*'s: an adoption counts as its proposal's author's,
a conflict copy as its version's ([05 §5.2](05-catalogue-and-names.md);
`catalogue::View::writers`, `Authority.tla` `Writers`).

`.pigeon/` has its own table (ADRs 0009, 0017, 0018, 0047, 0051):

| Path under `.pigeon/` | Who may add or change it |
|---|---|
| device entries of person P in `roster.toml` | an admin, or a device of P (ADR 0017) |
| anything, from a device with `role = "recovery"` | only device entries of its own person, even if an admin; never a Force (ADR 0047) |
| `decisions/<commit>.toml`, `verdict = "rejected"` | a decider of that proposal (§4.5, ADR 0051) |
| `decisions/<commit>.toml`, `verdict = "withdrawn"` | a device of the proposal's author |
| `decisions/purge-<id>.toml` | an admin (ADR 0018, [08 §8.6](08-custody-loops.md)) |
| `records/drop-<device>-<id>.toml` | any member, about their own device (the device in the name), added only ([08 §8.7](08-custody-loops.md)) **[no ADR]** |
| `roster.toml`, anything else | an admin |

Each row is checked under the roster at the commit's parents.

## 4.4 The authority rule

Pure functions of the core (ADR 0012): `R(p)`, the roster in force for the
children of *p*, the fold of §4.8, held in `tree(p)`; `signer(c)`, the device
whose SSHSIG signature (namespace `git`) is in the `gpgsig` header of *c*,
verified with `ssh-key` (ADR 0007), and `person(c)` its person (unsigned
commits are invalid; `allowedSignersFile` and commit dates play no part);
`catalogue(S)` of an ancestor-closed set of accepted commits
([05 §5.2](05-catalogue-and-names.md)).

A commit *c* other than the genesis, with **A(c)** its accepted parents and
**X(c)** the others, is **accepted** iff (`Authority.tla`, INV-Valid of
[02 §2.5](02-guarantees.md)):

| # | Rule |
|---|---|
| 1 | A(c) is not empty |
| 2 | `signer(c)` is a device of `R(p)` for every *p* in A(c), or, when Δ = ∅, for some *p* (ADR 0051) |
| 3 | with `B = catalogue(⋃ anc*(p), p ∈ A(c))` (B = tree(p) for one accepted parent) and `Δ = diff(B, tree(c))`, every change of Δ is allowed for `person(c)` under `R(p)` for every *p* in A(c) (§4.3), or *c* is a valid Force (§4.6) |
| 4 | if Δ changes the roster, the new roster is valid (§4.2) |
| 5 | every path Δ adds is portable ([05 §5.4](05-catalogue-and-names.md)) |
| 6 | `tree(c)` is **well formed** ([07 §7.9](07-capture-and-install.md)): folders and files of mode 100644 or 100755 only; outside `.pigeon/` and the root `.gitattributes`, each file a git-annex pointer to a `SHA256E` key with its size; inside them, plain blobs |
| 7 | every parent in X(c) is a proposal (§4.5) that *c* adopts: `signer(c)` is one of its deciders under R(p) for every *p* in A(c), or an admin with Force (§4.6) **[no ADR]**, and `tree(c)` holds its change: each version it wrote, and no path it deleted unless B changed it since (ADR 0051) |

A commit breaking rule 6 is **malformed**: neither accepted nor a proposal,
and its keys are not references ([08 §8.6](08-custody-loops.md)). A commit is
**unknown** to a release when a parent's roster has a `rules` it does not
support, or a `.pigeon/` document has a `format` or field it does not know
(every schema is closed); it and its descendants stay as fetched, judged again
after an upgrade. An older release's verdict is a newer one's, or *unknown*
(ADR 0048).

Consequences, checked by `Authority.tla` (`AuthorisedContent`,
`DecisionsByDeciders`, `AdoptionHoldsContent`, `RemovedSilenced`,
`RemovalRevokesAdded`, `HonestMergesAccepted`, `Converge`, `ConvergedWhenQuiet`):

- A clean merge has Δ = ∅, counts in no catalogue ([05 §5.1](05-catalogue-and-names.md)),
  and any parent's roster may sign it, so two branches that removed each
  other's devices still merge (`Roster_every` deadlocks otherwise).
- **Adoption** is a merge of an accepted parent and a proposal, with Δ the
  proposal's change; a merge with a proposal it does not adopt is a proposal.
- A commit mixing allowed and disallowed changes is rejected whole; capture
  splits, so never writes one ([07 §7.3](07-capture-and-install.md)).
- **Revoked** (ADR 0051): an accepted *x* is revoked in a set holding an
  accepted removal of `signer(x)` not descending from *x*, or when
  `signer(x)` is not in the roster folded from *x*'s ancestors. The catalogue
  and the fold skip it except for its device removals; it is shown as a
  suggestion of its signer's person ([09 §9.7](09-network-and-joining.md)).
- **Locality:** a verdict depends only on *c* and its ancestors, so
  `accepted(S ∪ T) = accepted(S) ∪ accepted(T)` for ancestor-closed S, T; a
  roster change applies to the *children* of its commit.
- **In code:** `pigeon-core::sig` signs and verifies (`tests/sig.rs`) and
  `authority::judge` gives the verdict for one roster R(p)
  (`tests/authority.rs`, against an oracle of §4.3's tables).
  `commit::judge` gives it for any parents: under each accepted parent's
  roster (rules 2 and 3), with rule 5, and a parent outside A(c) must be a
  proposal it adopts (rule 7, `proposal::adopts`). Its verdicts on random
  histories of commits, decisions and adoptions are `Authority.tla`'s
  (`tests/authority_trace.rs`, `AuthorityTrace`). `pigeond` (`merge.rs`)
  judges every commit once, parents first, with it, against
  B = `catalogue::Engine`'s catalogue of its accepted parents; a merge of
  parents whose rosters differ (§4.8) must hold their fold, after which its
  roster leaves Δ. Capture splits a batch with `authority::split`
  (§4.5). `pigeond` signs every commit it writes with the device key, and
  accept takes the last accepted of a peer's commits in topological order:
  by locality, an accepted commit may hold refused ones, the proposals it
  adopts. Unless the peer's `main` is accepted, the first commit not
  accepted is named by `doctor` with its problem code: `commit-unsigned`, `commit-forged`,
  `commit-unknown-signer`, `commit-not-allowed`, `commit-malformed` or
  `commit-waits` ([10 §10.6](10-interfaces.md)).

## 4.5 Proposals

A **proposal** is a signed commit §4.4 does not accept, made by capture or
found in a peer's history, kept as
`refs/pigeon/proposals/<signer-device-id>/<commit>` (ADR 0009). Deciders,
adoption and concurrent decisions are ADR 0051, checked by `Authority.tla`
(`DecidersCanAdopt`, `DecisionsByDeciders`, `AdoptionHoldsContent`).

| Point | Rule |
|---|---|
| shape **[no ADR]** | one proposal per capture batch and set of deciders, on the `main` it captured on; a later save of a path in a pending proposal of this device stacks a new one on it, shown as one suggestion (ADR 0034) |
| deciders | the persons allowed (§4.3) to make every change of its adoption on `main`, where a conflict copy is a new file; if nobody, the admins, with Force; recomputed when `main` moves |
| revisions | a decision names one commit: adopting it adopts its ancestors in the chain; rejecting it leaves later ones pending |
| state (computed) | the first that holds: *accepted* (an accepted commit descends), *rejected* or *withdrawn* (a decision, [07 §7.6](07-capture-and-install.md)), *superseded*, *pending*; revoked commits alike |
| accept | on a `main` holding all it builds on, merge (`main`, proposal) with its change; a path changed meanwhile keeps both, the proposal's under its conflict name ([05 §5.2–5.3](05-catalogue-and-names.md)) |
| some files (ADR 0075) | one commit on `main`, not a merge, signed by a decider: its tree holds the adoption's change of the accepted paths only (`proposal::partial`), with Force as for the adoption, and adds the decision `rejected` for the rest; a `Pigeon-Accepted: "path"` trailer names each accepted path, believed only where the tree holds its change (`proposal::holds`). It closes the proposal; a rename is decided as one file |
| concurrency | an adoption wins over a rejection; the proposal stays the author's **submission**, the adoption the decider's **authorised change**, so rights never widen |
| content | replicates like any version: *protected* only once K custodians hold it (ADR 0034, [06 §6.2](06-status.md)) |

**In code** (M2, first step): `authority::deciders` and `authority::split`
(`tests/authority.rs`: `a_proposals_deciders_are_those_who_may_adopt_it`,
each decider's device may sign the adoption and nobody else's;
`capture_splits_a_batch_into_an_accepted_commit_and_proposals`, nothing
lost, the commit on `main` accepted, each proposal refused for folder rules
alone). `pigeond::proposal` writes and keeps them; accept keeps a peer's
only if its ref names its commit, the signature is the named device's, and,
judged as one commit on the first accepted commit it builds on by first
parents, it is refused for folder rules alone (`commit-not-allowed`). Place
moves their content like `main`'s, and status reports list them.
`pigeon suggestions` lists the pending ones (not in `main`, and no other
kept proposal stacked on them) with their deciders on the current `main`;
`pigeon accept` writes the adoption, `proposal::adoption` (laws in
`tests/proposal.rs`: it holds its proposal's change, keeps what `main`
changed meanwhile and every path the proposal did not change, adds only
conflict copies, and is the proposal itself on an unchanged `main`).
`pigeon decline` commits the decision `rejected`; `authority::judge`
accepts a new `.pigeon/decisions/<commit>.toml` from a decider of that
proposal (`rejected`) or its author, the person of its signer under the
roster of its base (`withdrawn`), never a changed one, and lets the commit
wait when this device does not keep the proposal
(`a_decision_is_accepted_iff_its_signer_may_make_it`; `commit::about`).
`pigeon resolve` writes a withdrawal (07 §7.6, `pigeond::resolve`).
`pigeon accept ID FILE…` and `pigeon decline ID FILE… [--note TEXT]`
(`DecideFiles`, ADR 0075) decide some files: the files named are accepted
and the others declined, or the reverse, in one decision
(`pigeond::decide_files`; laws in `tests/proposal.rs`: a partial decision
is the adoption on the paths it accepts and `main` elsewhere; models
`Authority_part`, `Authority_part4`: a decider's such decision is always
accepted; `AuthorityTrace` checks the code's verdicts on them;
`pigeond/tests/decide_files.rs` and `suggestion_files.rs`: a later joiner
verifies them). An older release accepts the commit and reads a decline:
its install ends the suggestion of each file `main` now holds as saved
and makes the others conflicts (07 §7.6). `pigeon suggestions` lists what
each file's change is (new, changed, deleted, renamed, sizes, and the lines
added and removed of a small text file whose two versions are here), and,
to its author, how each of their suggestions was decided in the last 30
days: by whom, which files were accepted and which declined, and the note
(`pigeond::decided`).

A rejection or withdrawal is a commit of `.pigeon/decisions/<commit>.toml`:

```toml
format = 1
proposal = "<commit id>"
verdict = "rejected"        # or "withdrawn"
note = "On garde la recette de Mamie."
```

## 4.6 Force

A commit §4.4 rejects only for folder rules is accepted with the trailer
`Pigeon-Force: <reason>` when its signer is an admin under the rosters of
its accepted parents. It never waives rules 2, 4 and 6 or the `.pigeon/`
table. The CLI asks first, the web interface says what changes, and history
shows "admin override by Roy". Only
admins force, as ux.md says, not any member as the proposal said **[no ADR]**.

## 4.7 Changing the roster

- Every roster change is a signed commit checked by §4.4 (ADR 0009); a hand
  edit of `roster.toml` is captured like any file, a proposal from a
  non-admin.
- A device of P: added or removed by an admin or a device of P (ADR 0017;
  `a_member_who_is_no_admin_signs_only_their_devices`). A person, or an
  admin's removal: an admin; the last admin stays. A custodian's `maxsize`:
  an admin or a device of its person.
- Forks merge by the fold of §4.8, never a file merge (ADR 0051). A change
  of K, a role or `maxsize` re-applies the settings ([03 §3.5](03-repository-and-roles.md)).

## 4.8 Roster forks: the fold (ADR 0051)

Normative: `pigeon-core::roster` with its laws and `Roster.tla`. Each
accepted commit carries a **change** δ(c) from the fold of its accepted
parents (`set` from the value its author saw, `add`, `remove device`,
`remove person` with their devices and steward roles), judged once under
its parents; a clean merge has none, a decision merge the change it adopts.

| Rule | Law (`tests/roster.rs`) | `Roster.tla` |
|---|---|---|
| canonical order: topological, then smallest (author time, device ID, commit ID), as in [05 §5.2](05-catalogue-and-names.md) | `the_canonical_order_is_topological` | `TreeIsFold` |
| a change applies whole when every `set` finds its old or new value, every `add` its entry absent or equal, and the result is valid; else it is **skipped** | `every_tree_is_the_fold_of_its_history` | `TreeIsFold`, `Agreement` |
| the fold depends only on the set of ancestors | `the_fold_depends_only_on_the_set_of_changes` | `Agreement` |
| every result is valid with an admin; `remove device` edits always apply (`Removals = "always"`), a revoked commit's change is skipped likewise | `every_fold_is_valid_and_removals_win` | `AdminRemains`, `RemovalWins`, `PersonRemovalWins` |
| a removed device or `annex_uuid` never returns, even one never added | `removing_an_absent_device_keeps_it_out` | `RemovalWins` |
| skips need concurrency; a linear history skips nothing | `skips_need_concurrency`, `a_linear_history_skips_nothing` | `SkipsNeedConcurrency` |
| a merge is always possible | — | `MergeAlwaysPossible` |

Replaying one side onto the other depends on who merges (`Roster_rebase`);
of two concurrent removals that would leave no admin the later is skipped
(`Roster_person`). **Skipped changes** are listed with their reason; the
admins and the author see *Needs you* ("Alice and Bob gave up their admin
role at the same time. One admin must remain: Bob stays admin."). *Apply
again* writes a new change.

**In code** (M2): `pigeond::fold` records each accepted commit's δ — the
edits from its one parent roster to its tree's, none for a merge — in a
`roster::History` beside the catalogue engine. A merge of parents whose
rosters differ must hold exactly `History::fold` of its history (else
`commit-malformed`); its roster then leaves Δ, so it is judged as a clean
merge that any parent's roster may sign, and accept writes such merges
with the fold (`pigeond/tests/merge.rs`, `roster_forks_merge_by_the_fold`).
Skipped changes are logged (`roster-skipped`), not yet shown; revoked
commits are not yet skipped.
