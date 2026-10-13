# 0034. pigeon never overwrites what a person saved; a diverging file is a conflict they resolve

- **Status:** Accepted, 13 October 2026, by Guillaume (chat: "we don't
  overwrite user files but signal that there is a conflict"). Supersedes
  ADR 0033.

## Context

ADR 0033 kept files of others' folders editable, turned a saved edit into a
suggestion, and then put the accepted version back on the person's disk.
Guillaume rejected the put-back: the owner accepts or declines the
suggestion, but in neither case does pigeon overwrite the person's file. The
proposal (§4, capture loop) also said "the file is restored"; that line no
longer holds.

## Decision

- **Rule:** pigeon never replaces, moves aside or removes content that a
  person saved, unless that person asks. Installing a new accepted version
  skips a path whose file differs from what pigeon last installed there and
  is not part of the accepted catalogue.
- Files of folders a person may not change stay editable. Saving one
  captures a signed suggestion (ADR 0009); the file stays as saved. Its
  state reads "Your suggestion, waiting for Paul". Later saves update the
  same suggestion.
- **Accepted:** the person's version becomes the group's; the divergence
  ends.
- **Declined, or the group's version changed meanwhile:** a *conflict*,
  signalled once by notification and kept on Home ("Needs you") and in
  Suggestions → Yours: "Paul declined your change to *tarte.docx*; your
  version is still on your computer." The person chooses: *Take the group's
  version*, *Keep mine as a copy* (in a folder they may change), or *Suggest
  again*. Until then their file stays untouched, and newer accepted versions
  of that path are not installed on their disk.
- A conflict changes no safety state: the headline level (ADR 0027) is still
  about safety. The person's version is safe as the suggestion's content,
  which backup machines keep like any version.
- Where the group's version shows during a conflict is decided separately
  (UX question 4).

## Consequences

- `pigeond` keeps a local table of divergences: path, the person's
  suggestion, the accepted version they diverged from. Install consults it;
  status reports "1 file differs from the group's".
- P5 (convergence) is about the logical catalogue; physical trees already
  differ by selective sync, and now also by unresolved divergences.
- P2 becomes stronger: install never destroys a person's saved file, even
  after capturing it.
- The capture and install models (TLA+) must include divergence: no
  behaviour installs over a diverging path.
