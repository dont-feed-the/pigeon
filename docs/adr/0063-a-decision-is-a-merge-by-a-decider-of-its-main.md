# 0063. A decision is a merge by a decider of the main it is made on

- **Status:** Proposed, 14 October 2026, by the agent, for issue #10
  (Guillaume's review: "Add models and trace vocabulary for capture/install,
  authorization, purge, and repair"), on the evidence of the `Authority`
  model. Refines ADR 0038.

## Context

ADR 0038 makes the deciders of a suggestion "the persons who may make every
change of its diff under the roster at its parents", or the admins if nobody
may. It says that a rejection is valid "when signed by a decider of that
commit, under the roster at the decision's parents". 04 §4.5 calls a
proposal *accepted* when "an accepted commit has it as an ancestor". The
`Authority` model ([13 §13.2](../spec/13-verification.md)) runs these rules
with an honest admin a, b who owns a folder, and a faulty member c who signs
anything on any parent. It finds five problems.

- **Deciders at the proposal's parents** (`Authority_proposal`, 3 states). a
  suggests a change in b's folder, then gives that folder to himself. b is
  still the decider, but rule 3 of 04 §4.4 rejects b's adoption, because b no
  longer owns the folder. a can adopt it but not reject it.
- **The proposal's own diff** (`Authority_mixed`, 4 states), even under
  today's roster. a adds `r` at the root. c suggests adding `r` and a file in
  b's folder. The diff makes b the decider. But `main` now has `r`, so
  adopting keeps both versions (04 §4.5). That modifies `r`, which only the
  admins may do, so b's adoption is rejected.
- **Admins without Force** (`Authority_noforce`). A suggestion that nobody may
  make falls to the admins. But rule 3 rejects an admin's adoption on folder
  rules unless it is a Force (04 §4.6), which ADR 0038 does not say.
- **A merge that adopts nothing** (`Authority_fakeadopt`). Any member signs
  a clean merge of `main` with someone's proposal, keeping `main`'s tree. The
  proposal becomes *accepted*, although its content never entered, and it
  leaves its deciders' list. With proposals also counted in the base of a
  merge, their content enters with no decider at all (`Authority_base`). That
  is why 04 §4.4 takes the base from A(c) only.
- **States that overlap.** A decided proposal can have a later revision, and
  a rejection can be concurrent with a withdrawal. 04 §4.5 gives no order
  between its states.

Also, an adoption made on a `main` that lacks some accepted commit the
proposal builds on brings that commit in. Its tree is then not the catalogue
of its history, so its children's base is wrong (`TreeIsCatalogue`).

## Decision

1. **Deciders are judged at the decision.** For a decision on `main` *m*,
   the deciders of a proposal are the persons who may make the adoption's
   change on *m*: the diff between *m* and *m* with the proposal's change
   (04 §4.5 *Accept*), under R(*m*). If nobody may, the deciders are the
   admins. Adoption and rejection use this one set, recomputed when `main`
   moves.
2. **Admins adopt with Force** what nobody else may. Their adoption carries
   `Pigeon-Force` (04 §4.6) and is shown as an admin override.
3. **Rule 7 of 04 §4.4.** Every parent of *c* outside A(c) must be a
   proposal. The signer of *c* must be one of that proposal's deciders, under
   the roster of each accepted parent. And the tree of *c* must hold the
   proposal's change. Otherwise *c* is not accepted, and is a proposal
   itself.
4. **The state of a proposal** is the first of these that holds: accepted,
   rejected, withdrawn, superseded, pending. So an adoption wins over a
   rejection (ADR 0038), a rejection over a withdrawal, and any decision over
   a later revision.
5. **pigeond decides only on a `main` that holds every accepted commit the
   proposal builds on**, merging them first.

## Consequences

- In the model, every decider of a pending proposal can make a valid
  adoption (`DecidersCanAdopt`), and every decision in a history was made by
  a decider or by the author (`DecisionsByDeciders`). A proposal is accepted
  only through a merge that holds its content (`AdoptionHoldsContent`). Each
  version in a catalogue entered through a commit allowed to make it
  (`AuthorisedContent`). `Authority_decide` and `Authority_faulty` pass all
  four.
- The deciders shown for a suggestion can change when `main` changes, for
  example after a folder changes owner. The suggestion's page shows them as
  of now.
- A faulty or old client's merge of a proposal that does not adopt it stays a
  proposal: it takes nothing away.
- If ADR 0038 is accepted, its status line should gain "deciders refined by
  ADR 0063".

## Alternatives considered

- **ADR 0038's words** (the roster at the proposal's parents), or the
  proposal's own diff under today's roster: each leaves a decider who cannot
  adopt (`Authority_proposal`, `Authority_mixed`).
- **Deciders frozen when the proposal is made**: the same failure, as soon as
  the folder changes owner.
- **A proposal is accepted when its content is in the catalogue**: a
  deletion, or a change that `main` made identically meanwhile, has no
  content to find. Requiring a decision merge is simpler to check.
