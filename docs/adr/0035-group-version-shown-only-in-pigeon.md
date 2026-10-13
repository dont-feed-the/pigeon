# 0035. During a conflict, the group's version is shown only in pigeon

- **Status:** Accepted, 13 October 2026, by Guillaume (UX question 4).
  Completes ADR 0034.

## Context

ADR 0034 leaves a person's saved file untouched when it diverges from the
group's version, and lets them choose how to resolve the conflict. To choose,
they need to see the group's version. Placing it next to their file
(`tarte (Paul's version).docx`) would put in the shared folder a file that is
not in the catalogue, which capture would have to ignore and people could
edit by mistake.

## Decision

- The shared folder holds only what the person saved and the accepted
  catalogue; no conflict copies appear in it.
- *Open Paul's version* (web) or `pigeon view PATH --group` writes a
  read-only temporary copy in pigeon's cache directory, outside the shared
  folder, named `tarte (Paul's version).docx`, and opens it with the default
  application. `pigeon view PATH --version N` does the same for any version
  of the file's history.
- Temporary copies are removed when the conflict is resolved, or after
  seven days.
- `pigeon conflicts` lists conflicts; `pigeon resolve PATH --take-group |
  --keep-copy DEST | --suggest-again` resolves one, like the three buttons.

## Consequences

- Capture never sees conflict copies, so it needs no exclusion rule for them.
- Comparing in the application is not side by side; the web page compares
  text and images itself (ux.md §6, Suggestions).
