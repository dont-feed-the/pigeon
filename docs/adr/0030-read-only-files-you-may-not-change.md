# 0030. Files you may not change are read-only on your disk

- **Status:** Rejected, 13 October 2026, by Guillaume (UX question 2), in
  favour of ADR 0033. Proposed the same day by the agent in the UX study.

## Context

ADR 0009 turns a change that breaks the folder rules into a *proposal*
(shown to people as a *suggestion*), and the capture loop puts the accepted
version back. For someone who is not technical, seeing a saved edit undone
is alarming, even if the edit is kept elsewhere. Every desktop already has a
familiar signal for "you cannot change this": a read-only file, which
applications report when saving.

## Decision

- When pigeon installs a file that this person may not change (someone
  else's folder; an existing file in an add-only folder they do not steward),
  it sets it read-only on disk. Files they may change stay writable.
- *Suggest a change* (web interface, `pigeon suggest PATH`) makes an editable
  copy in the person's own space and, when they are done, sends it as a
  suggestion.
- Proposals stay the safety net: an edit that still slips through (the rule
  changed while the computer was offline; the person removed the read-only
  flag) becomes their suggestion, the accepted version comes back, and a
  notification explains it in one sentence.
- The read-only flag is a hint for people, not a security boundary: the
  authority rule (P4) is still enforced on every commit.

## Consequences

- Install sets and clears permissions as rules change (a roster change
  that grants a folder makes its files writable).
- Windows (v1.1) uses the read-only attribute, with the same caveats.
- Experiment to add: applications' behaviour when saving a read-only file
  (Office, LibreOffice, Preview, image editors).
