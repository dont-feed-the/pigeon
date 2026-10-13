# 0024. The command line: `clap`, `--json` everywhere, stdout for results

- **Status:** Accepted, 13 October 2026. Chosen by the agent after a survey
  that Guillaume asked for.

## Context

Survey of crates.io on 13 October 2026 (downloads over the last 90 days):
`clap` 254 M, `argh` 2.9 M, `lexopt` 2.9 M, `bpaf` 1.9 M. `clap`'s derive API
gives help, suggestions, shell completions (`clap_complete`) and man pages
(`clap_mangen`). The Command Line Interface Guidelines (clig.dev) and the
Rust CLI book recommend: results on stdout, messages on stderr, honour
`NO_COLOR`, confirm dangerous actions, machine-readable output on request,
meaningful exit codes.

## Decision

- `clap` 4 with derive. Each subcommand maps to one request of `pigeond`'s
  API (ADR 0021): `status`, `invite`, `join`, `restore`, `evict`, `purge`,
  `accept`, `reject`, and so on.
- Every command accepts `--json`: its result on stdout, in the schema of the
  API response. Without it, human output goes to stdout and messages and
  progress to stderr. Colour goes through `anstream`, which honours
  `NO_COLOR` and terminal detection; progress bars (`indicatif`) appear only
  on a terminal.
- Dangerous operations (purge, Force, Drop, leaving a group) ask before
  acting; without a terminal they need `--yes`; `--no-input` never asks.
- Exit codes: 0 success, 1 failure (with a `Problem`, ADR 0023), 2 usage
  error (clap's default).
- Shell completions and man pages are generated from the same definitions.

## Consequences

- Scripts depend on `--json` and problem codes, not on human wording.
- The CLI holds no logic of its own: it parses, sends a request and renders.
