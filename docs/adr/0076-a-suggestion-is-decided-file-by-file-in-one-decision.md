# 0076. A suggestion is decided file by file, in one decision

- **Status:** Proposed, 16 October 2026, by the agent, for the usability
  study's finding "a suggestion of several files can only be accepted or
  declined whole, with no difference shown" ([usability-web-5](../reports/usability-web-5.md)),
  which the user put in scope with bulk accept and decline. Refines ADR
  0051 (decisions on suggestions).

## Context

Capture makes one proposal per batch and set of deciders (04 §4.5), so a
suggestion holds every file a person changed in a folder they may not
change. Its deciders could only adopt it whole, a merge whose tree holds
its whole change (rule 7 of 04 §4.4), or reject it whole, with
`.pigeon/decisions/<commit>.toml`. A participant could not take Bob's
change of `recipes.txt` and leave his change of `groceries.txt`, and Bob,
whose files stayed as he saved them, was never told which were taken.

Every device judges every commit offline with the rules of its release,
and an older release keeps a commit it does not understand as *unknown*
(ADR 0048): a new decision format would stall every older device's
`main` until it upgrades. A decision must also stay verifiable by a
computer that joins later, from signed objects alone (decision history,
[decision-history-5](../reports/decision-history-5.md)).

## Options

1. **Rule 7 allows a partial adoption**: a merge with the proposal whose
   tree holds the change of some paths, the others named by a new
   decision. Every older release refuses such a merge for good
   (`commit-not-allowed`), and every commit after it.
2. **A decision format 2** listing the accepted paths. Older releases
   keep the commit as unknown: their `main` stops there until they
   upgrade.
3. **One proposal per file** at capture. Decisions stay whole, but a batch
   of a hundred files is a hundred signed commits and merges, a rename
   spans two, and the bundles already made stay whole.
4. **One ordinary commit by the decider**: it holds the change of the
   accepted files as the adoption would and adds the decision `rejected`
   in format 1; trailers of its message name the accepted files.

## Decision

Option 4.

- A decision on some files is one commit on `main`, signed by a decider,
  with `main` as its only parent, not a merge. Its tree is the adoption's
  (`proposal::adoption`) for the accepted paths and `main`'s for the
  others (`proposal::partial`), and it adds
  `.pigeon/decisions/<commit>.toml`, `verdict = "rejected"`, with the
  decider's note. The authority rule (04 §4.4) judges it as any commit: a
  decider may make every change of the adoption, so every change it
  accepts, and adds the decision as a decider; an admin forces what
  nobody may change with `Pigeon-Force:` as for an adoption (04 §4.6).
- Its message names each accepted file in a `Pigeon-Accepted: "path"`
  trailer. The trailer is a claim: a reader counts a file accepted only
  when the commit's tree holds its change (`proposal::holds`); the other
  files of the suggestion were declined.
- Every decision closes the suggestion: the files not accepted are
  declined. With every file accepted it is the adoption, with none the
  decision `rejected`, as before. A rename is decided as one file.
- `pigeon accept ID FILE…` accepts the files named and declines the
  others; `pigeon decline ID FILE… [--note]` the reverse. They are a new
  request, `DecideFiles`, which an older `pigeond` refuses rather than
  reading as a whole decision. The web's Suggestions page ticks every
  file, with *Accept selected* and *Decline selected*, and *Accept all*
  and *Decline all* for several suggestions.
- Its author's computer shows, for 30 days, which files of each of their
  suggestions were accepted and which declined, by whom, with the note.

## Consequences

- No format, rule or wire message changes for older releases. An older
  release accepts the commit, reads the decision as a decline of the
  suggestion, and, file by file, its install ends the suggestion of each
  accepted file (`main` now holds the version its author saved) and makes
  each declined one a conflict (07 §7.6): what reaches disk is right. Only
  its wording is coarser: it says nothing of which files were taken.
- A computer that joins later verifies the commit from the signed
  proposal, as any decision (`tests/decide_files.rs`).
- An accepted file's change is written by the decider, not adopted: the
  history shows the decider as the writer of that version, where an
  adoption shows the suggestion's author, and no rename is recorded, as
  for an adoption.
- A decline that accepts files can surprise: the page and the command
  say before and after which files go which way.
- `Authority.tla` (`Authority_part`, `Authority_part4`) checks that such a
  decision by a decider is always accepted (`PartsAccepted`) and keeps the
  invariants of ADR 0051; `AuthorityTrace` checks the code's verdicts on
  random histories with such decisions.

## Alternatives considered

- **A later per-file decision on the rest**: keeping the declined files
  pending needs a state an older release cannot read.
- **Telling the author by a message outside the history**: nothing would
  prove it, and a computer that joins later would not know it.
