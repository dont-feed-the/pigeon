# 0033. Files you may not change stay editable; a saved edit becomes your suggestion

- **Status:** Superseded by 0034 (the accepted version is never put back
  over the person's file). Accepted on 13 October 2026 by Guillaume (UX
  question 2), who preferred it to read-only files (ADR 0030, rejected).

## Context

ADR 0009 turns a change that breaks the folder rules into a proposal, shown
to people as a *suggestion*. ADR 0030 proposed to prevent such edits with
read-only files. Guillaume chose to keep files editable: nobody is blocked
when saving, the behaviour is the same on every platform, and no file
permissions follow the roster. The cost is a surprise: a saved edit is put
back. This ADR makes that surprise as small and as clear as possible.

## Decision

- Files of folders a person may not change stay writable on their disk.
- Saving such a file captures the edit as that person's signed suggestion
  (ADR 0009), sent to whoever decides (the owner, or the folder's steward).
- The accepted version is put back only once the file has been quiet for
  one minute, so an application still saving is not fought. Saves before
  that update the same suggestion: one suggestion per file and editing
  session, not one per save.
- One notification per suggestion, at put-back time: "Your change to
  *tarte.docx* was sent to Paul as a suggestion, because this folder is
  Paul's. Your version is in Suggestions → Yours."
- The rule is shown before anyone edits: the Files page and
  `pigeon status PATH` say "Paul's folder: your changes become suggestions
  to Paul".

## Consequences

- No `pigeon suggest` command: editing is suggesting.
- The save-pattern suite must cover put-back while an application holds the
  file open (Office, LibreOffice, Preview, image editors): some will ask to
  reload, some will save again, which only updates the suggestion.
- The suggestion holds the person's version safely, so put-back never loses
  their work (P2).
