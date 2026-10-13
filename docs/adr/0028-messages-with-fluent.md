# 0028. Messages come from one Fluent catalogue, in French and English

- **Status:** Accepted, 13 October 2026, by Guillaume (UX question 6).
  Proposed by the agent in the UX study.

## Context

The CLI, the web interface, the tray and notifications must say the same
things in the same words (ADR 0027), and the first groups speak French.
Messages carry counts ("1 file", "3 files"; French treats 0 as singular),
names and durations. Candidates on crates.io (13 October 2026): Project
Fluent (`fluent-bundle` 0.16, `fluent-langneg` 0.14, made by Mozilla for
Firefox), `rust-i18n` 4.2 (key-value YAML), `i18n-embed` 0.16 (Fluent or
gettext loaders).

## Decision

- All user-facing text lives in Fluent `.ftl` files, one per language,
  embedded in the binary at build time and loaded with `fluent-bundle`;
  `fluent-langneg` picks the language from the setting, then the system.
- v1 ships `en` and `fr`. A test checks that every message id exists in
  every language and that each message formats with sample arguments.
- Message ids are shared by the CLI, the web interface and the tray. A
  `Problem` (ADR 0023) carries a message id and its arguments, rendered at
  the edge in the reader's language; logs and `--json` keep the stable code.
- Times: `jiff` for time zones and spans; relative times ("4 minutes ago")
  are Fluent messages with a number argument.

## Consequences

- Adding a language is adding one file, which a translator can edit without
  Rust.
- English text cannot be written inline in code; reviews check for it.
