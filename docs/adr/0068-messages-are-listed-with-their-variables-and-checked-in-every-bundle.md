# 0068. Messages are listed with their variables and checked in every bundle; the language follows gettext's order

- **Status:** Proposed, 14 October 2026, by the agent, for issue #17
  (Guillaume, in chat: "work on a first version of the implementation even if
  not all details are figured out yet"). Settles the display form of
  problem codes in 10 §10.6, marked **[no ADR: display form]**.

## Context

ADR 0028 puts all text for people in Fluent files and requires a test that
every message id exists in every language. 10 §10.5 picks the language from
the `language` setting, then the system locale, then English. The slice
ships English only, and ADR 0057 defers the completeness test until after
it. Writing `pigeon-i18n` left details open. Where do the bundles live? How
does code name a message, so that a test can check it? How is the system
locale read, and in what order? How are problem codes shown? And 14 §14.7
pins `fluent-langneg` 0.14, which does not fit `fluent-bundle` 0.16.

## Decision

1. **The bundles** are `crates/pigeon-i18n/i18n/<lang>/pigeon.ftl`, embedded
   with `include_str!`. The crate reads no file and no environment at run
   time. The slice has `en` alone; French is one more file and one more row
   of `BUNDLES`.
2. **A list of every message.** `pigeon_i18n::ids::MESSAGES` lists each id
   with the names of its variables. Code names a message as a `Message`:
   an id and its arguments, the `{"id", "args"}` of a Problem
   (10 §10.6). Arguments are counts, which select plural forms, or texts
   such as names, paths and times already rendered. `check(&msg)` tells
   whether the id is listed and the arguments are exactly its variables,
   so each renderer can check its messages in its own tests.
3. **The completeness test runs now**, for the bundles this build has. It
   parses each bundle with `fluent-syntax` and checks that:
   - every listed id is in the bundle, and the bundle lists no other;
   - a bundle uses no variable the list does not name, and English uses
     exactly those named;
   - messages refer to no other message and no term, so each message's
     variables are its own;
   - every message renders with sample arguments, without errors, and
     shows each text argument;
   - every problem code has `problem-<code>`, and every `hint-<code>`
     belongs to a code.

   The lint on English string literals in renderers comes with the
   renderers.
4. **The language.** A `language` other than `auto` names a language.
   Otherwise the system's languages are read in gettext's order: the first
   set of `LC_ALL`, `LC_MESSAGES` and `LANG` gives the locale, and
   `LANGUAGE`, a list, comes before it unless the locale is `C` or
   `POSIX`. `fr_FR.UTF-8@euro` reads as `fr-FR`. The request is matched
   against the available bundles by lookup, and English is the default.
   The caller passes the lookup of variables, so tests need no environment.
   On macOS, the language settings of applications started outside a
   terminal are not read yet.
5. **Rendering.** An argument is wrapped in Unicode isolation marks for the
   web interface, and not for a terminal. An unknown id renders as the id
   itself and never fails: the test keeps it from happening.
6. **Problem codes are shown** as `P-` and the code in capitals,
   `P-INVITE-EXPIRED`, from `ids::display_code`. The slice adds the code
   `file-system-not-in-this-build`, with the file system's name, for
   `create` and `join` on a file system ADR 0057 defers.
7. **`fluent-langneg` 0.13**, not 0.14. `fluent-bundle` 0.16 uses 0.13
   and `unic-langid` 0.9; 0.14 moved to `icu_locid`, whose language type
   the bundle does not take. One type, and no second copy of the crate.

## Consequences

- An id missing from a bundle, or a variable spelt two ways, fails CI
  rather than showing a raw id to a person.
- Adding a message is two edits, the bundle and `ids.rs`, and the test says
  when one was forgotten.
- Times, sizes and numbers with decimals are rendered by callers until
  `jiff` and a number format arrive. Relative times are messages already.
- When `fluent-bundle` moves to `icu_locid`, `fluent-langneg` follows it,
  and 14 §14.7 changes with them.
