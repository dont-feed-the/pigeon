# pigeon v2

pigeon keeps the files of a small group (a family, a lab, a team) on
everyone's machines, without a central server. v2 is a rewrite: a policy and
user-experience layer over [git-annex](https://git-annex.branchable.com/),
which already knows how to store versions, count copies and refuse unsafe
deletions. pigeon adds the parts people see: who belongs to the group, which
machines keep the safe copies, and a status everybody can read.

> **Status:** design phase. This branch holds the decisions, the models and
> the experiments; there is no code yet. pigeon 0.9 lives on `main` and
> under the tag `v0.9.0`; v2 does not reuse it (ADR 0015).

## The promise in one picture

A version of a file goes through three states, and pigeon always says which:

| State | Meaning |
|---|---|
| local | only on the machine where it was saved |
| published | the group knows about it, but too few safe copies exist yet |
| protected | at least K custodian machines hold a verified copy |

Deleting is reversible until an explicit, signed purge. The full list of
guarantees (P1–P6) is in the [proposal](docs/design/proposal/proposal.pdf).

## Where things are

| Path | What it holds |
|---|---|
| [`docs/adr/`](docs/adr/) | architecture decision records, one per choice, with an index |
| [`docs/design/`](docs/design/) | proposal, review of the drafts, git-annex study, related work, spec drafts |
| [`models/tla/`](models/tla/) | TLA+ models of custody and their expected TLC verdicts |
| [`lab/git-annex/`](lab/git-annex/) | shell experiments E1–E10 on git-annex behaviour |
| [`scripts/`](scripts/) | the checks that CI runs |

## Checking the repository

The same commands run in CI ([ADR 0026](docs/adr/0026-continuous-integration.md)):

```sh
scripts/check-file-length-and-header   # no file reaches 1000 lines; source files start with a header
scripts/check-adr-index                # docs/adr/README.md lists every ADR, with its status
scripts/run-tlc                        # the fast set of TLA+ models (Java 11 or later)
scripts/run-tlc --full                 # every model, including the slow ones
```

`run-tlc` downloads TLC v1.7.4 once and checks its SHA-256. A model must give
the verdict written in [`models/tla/expected.tsv`](models/tla/expected.tsv):
some configurations must fail, because they show why the naive designs lose
data or deadlock.

## Licence

MIT OR Apache-2.0, at your option ([ADR 0019](docs/adr/0019-licence.md)).
