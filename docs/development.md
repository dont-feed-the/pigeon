# Developing pigeon

> Mission: how to build pigeon from source, check it, and release it.

## Build from source

With [rustup](https://rustup.rs), which fetches the pinned toolchain:

```sh
git clone https://github.com/mariogeiger/pigeon.git
cd pigeon
cargo build --release -p pigeond -p pigeon -p git-annex-p2p-iroh
packaging/install-git-annex --stock    # the official git-annex, where pigeon looks for it
```

Then copy the three programs to `bin/` of the directory above
(`~/.local/share/pigeon/bin`), put it on your `PATH`, and run `pigeond`.
A release is made by pushing a tag `v` + the workspace's version
(`.github/workflows/release.yml`). Each pull request bumps that version
and adds its entry to [CHANGELOG.md](../CHANGELOG.md), as
[AGENTS.md](../AGENTS.md) says.

Linux CI, lab and slice checks gate publication. macOS tests run periodically
through `macos-background`; both platforms' release archives still build.
Pull requests [select relevant checks from their changed files](ci-selection.md).
Main, release-tag and periodic runs retain full validation.
Track actual publication, candidate lead time and blocking jobs with
`python3 scripts/release-kpi`; its [KPI definition and targets](release-kpi.md)
explain the hourly reports. An immutable candidate can be retried using the
release workflow's `tag` input while retaining its original source commit.

## Try it on one computer

`scripts/try-pigeon` starts three pigeon daemons side by side in
`/tmp/pigeon-try`: Alice's laptop, Bob's laptop and a backup machine. It
puts them in one group, then prints what to try:

```sh
scripts/try-pigeon
echo hello > /tmp/pigeon-try/Alice/hello.txt
/tmp/pigeon-try/bin/alice status      # "Sending…", then "Everything is safe"
ls /tmp/pigeon-try/Bob                # hello.txt arrives in a few seconds
scripts/try-pigeon stop               # stops them and removes /tmp/pigeon-try
```

The three daemons talk over 127.0.0.1 only. The script works from the
build directory, and needs only `git-annex` on the `PATH` or installed with
`packaging/install-git-annex`.

## The repository

| Path | What it holds |
|---|---|
| [`crates/`](../crates/) | the daemon `pigeond`, the command line `pigeon`, the core algebras and the lab |
| [`spec/`](spec/README.md) | the specification, one subject per file, built from the ADRs |
| [`adr/`](adr/) | architecture decision records, one per choice, with an index |
| [`roadmap.md`](roadmap.md) | the milestones to v1.0 and when each counts as done |
| [`reports/`](reports/) | what each milestone delivered, and its known gaps |
| [`models/`](../models/) | TLA+ models, checked by TLC, and the catalogue merge's oracle |
| [`third_party/git-annex/`](../third_party/git-annex/) | the pinned git-annex and pigeon's patches |
| [`scripts/`](../scripts/) | the checks that CI runs |

```sh
cargo test --workspace                  # unit, property (hegel) and integration tests
scripts/check-file-length-and-header   # no file reaches 1000 lines; every file starts with a header
scripts/check-adr-index                # docs/adr/README.md lists every ADR
scripts/run-tlc                        # the fast set of TLA+ models (Java 11 or later)
```

The integration tests of `pigeond` need git-annex on the `PATH`. pigeon 0.9,
an earlier design, is kept under the tag `v0.9.0`.
