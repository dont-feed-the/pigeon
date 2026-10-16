# 0026. Continuous integration: GitHub Actions checks rules and model verdicts

- **Status:** Accepted, 13 October 2026. Guillaume asked for a v2 branch with
  CI; the agent chose the details below.

## Context

v2 starts as documents and models, with no code yet. What can break today:
a file over the 1000-line rule, an ADR missing from its index, and a TLA+
model edit that silently changes a verdict. The last one is the dangerous
one. Three of our configurations are *negative tests*: they must fail
(`NoLoss` violated, `TokensNeverLost` violated, a deadlock), because they
show why the naive designs are wrong. An edit that makes them pass hides
the argument for the design.

TLC timings in the agent's sandbox (two workers): every configuration but `T_n42o4`
finishes in under two minutes; `T_n42o4` takes several minutes or more.

TLC releases: v1.7.4 is the last stable release and is immutable on GitHub.
v1.8.0 is a rolling prerelease, and the nightly jar changes every night.
v1.7.4 lacks the `Json` module (`ndJsonDeserialize`) that trace validation
needs (ADR 0008); the nightly of 6 October 2026 has it.

## Decision

- GitHub Actions, two workflows. `ci` runs on every push and pull request:
  the repository rules (`scripts/check-file-length-and-header`,
  `scripts/check-adr-index`) and the fast set of models. `models-full` runs
  every configuration when a model or the runner changes, weekly, and by hand.
- Three tiers since 0.4.1. Pull requests run `ci` and `lab`, about ten
  minutes, with the fast models and pigeond's daemon tests in shards. A push
  to main adds `slice` without its S9 crash schedules. Release tags and the
  nightly run (skipped when main has not moved) add `models-full` and S9.
  Rust caches are saved from main only, so pull requests cannot evict them.
- `models/tla/expected.tsv` states the expected verdict of each
  configuration: `pass`, `deadlock` or `violation:<Invariant>`.
  `scripts/run-tlc` runs TLC and fails on any difference, in either
  direction.
- TLC is pinned: v1.7.4, downloaded from the GitHub release and refused
  unless its SHA-256 matches the one written in `scripts/run-tlc`. Java 21
  (Temurin).
- Third-party actions are pinned by commit SHA, with the version in a comment.
- Workflows get read-only repository permissions.
- The git-annex lab joins CI only once its scripts assert their results;
  today they print them. The Rust jobs (format, clippy, tests, licence
  check) arrive with the first crate.

## Consequences

- A verdict change is visible on the pull request, with the tail of the
  TLC output.
- When trace validation lands, v1.7.4 is not enough. We will then pin a
  build that has the `Json` module, by SHA-256 as now, in a new ADR.
- GitHub runs `schedule` and `workflow_dispatch` only from the default
  branch, which is now `main`: the nightly full set runs there.
- Pinned actions need manual updates; Dependabot can propose them later.
