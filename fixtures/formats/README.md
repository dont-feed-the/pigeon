# Format fixtures

Golden files of the formats pigeon replicates or keeps (ADR 0048, ADR 0036,
spec 13 §13.6), read by `crates/pigeond/tests/formats.rs` on every CI run.

| Directory | What it holds | What this release must say |
|---|---|---|
| `roster/known/` | rosters of format 1, as this release writes them | read, and read back the same once rendered |
| `roster/unknown/` | rosters a newer release could write: `format = 2`, a key format 1 does not define, a role or folder rule this release does not know | *unknown*: never read, never refused |
| `roster/malformed/` | files that are not rosters of format 1 | *malformed* |
| `status/known/`, `status/unknown/`, `status/malformed/` | status reports (06 §6.3), the same way | as their directory says; an unknown report does not count |
| `clock/known/`, `clock/unknown/`, `clock/missing/` | `.git/pigeon/clock.json` (11 §11.3) | as their directory says; an unknown file is kept as it is |
| `commits/*.toml` | one commit each: its parent's roster (a file of `roster/`), the trees before and after, who signed it | the `verdict`: `accepted`, `refused:<problem code>` or `unknown` |
| `commits/*.commit` | the commit objects, signed by the fixture keys | |

The fixture key of device *i* is 32 bytes of value *i*: Ana's laptop is 1,
Ben's laptop 2, Cleo's custodian 3, Ben's phone 4, Ana's recovery sheet 5;
device 9 was removed. A tree entry of a case is a blob's text, a table with
`mode` and `content`, `{ roster = "<file of roster/>" }`, or
`{ deleted = true }`.

**Rules.**

- A file of `known/` is never edited, nor removed while a release supports its
  format: an older release's files must read the same in every newer one.
- A newer format moves its examples from `unknown/` to `known/` only in the
  release that reads it; the files of `unknown/` stay as examples of what the
  release after it may write.
- `PIGEON_WRITE_FIXTURES=1 cargo test -p pigeond --test formats` writes the
  generated files that are missing (the known rosters, reports and clock
  file, and each case's `.commit`) and never rewrites one.
- The verdicts follow the compatibility law: an older release's verdict is a
  newer release's, or *unknown*.
