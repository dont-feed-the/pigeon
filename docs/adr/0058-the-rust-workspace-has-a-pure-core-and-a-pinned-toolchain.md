# 0058. The Rust workspace has one crate per row of 14 §14.6, a pure core and a pinned toolchain

- **Status:** Proposed, 14 October 2026, by the agent, for issue #17
  (Guillaume, in chat: "work on a first version of the implementation even if
  not all details are figured out yet"). Records the crate layout of 14 §14.6
  and the build rules of 14 §14.8.

## Context

14 §14.6 lists ten crates and 14 §14.8 asks for edition 2024 and a pinned
stable toolchain, both marked **[no ADR]**. The safety slice (13 §13.12)
builds eight of them, and several agents write them in parallel, so the
layout, the lints and the CI job must be fixed before the first line of each
crate. ADR 0012 wants a pure core whose laws are checked by property tests;
ADR 0023 forbids `unwrap` and `expect` outside tests; ADR 0028 keeps English
text for people out of Rust code.

## Decision

1. **Layout.** One virtual workspace at the repository root, resolver 3, with
   each crate of 14 §14.6 in `crates/<name>/`. All crates share one version
   (`2.0.0-alpha.0` until the slice passes) and `publish = false`. A crate is
   created when its work starts: the slice creates `pigeon-core`,
   `pigeon-proto`, `pigeon-i18n`, `pigeon-git`, `pigeon-sys`, `pigeond`,
   `pigeon` and `git-annex-p2p-iroh`; `pigeon-web` and `pigeon-telemetry`
   come after it.
2. **Dependency direction.** Library crates depend on `pigeon-core`, never
   the reverse. `pigeon-proto` depends on `pigeon-core` only;
   `git-annex-p2p-iroh` on `pigeon-proto` and `pigeon-sys` only; `pigeon` on
   `pigeon-proto`, `pigeon-i18n` and `pigeon-core`; `pigeond` on any library.
   Versions of third-party crates are declared once, in
   `[workspace.dependencies]`, pinned to the minor versions of 14 §14.7.
3. **The pure core is checked.** `scripts/check-core-deps` runs `cargo tree`
   on `pigeon-core`'s normal dependencies, direct and transitive, and fails
   on any crate absent from `crates/pigeon-core/allowed-deps.txt`, where each
   entry states why it performs no I/O. Development dependencies, such as
   `hegeltest`, are free.
4. **Toolchain.** `rust-toolchain.toml` pins Rust 1.99.0 with clippy and
   rustfmt; `rust-version = "1.99"`; edition 2024 and rustfmt's 2024 style.
   An upgrade is one commit that changes the pin and passes CI.
5. **Lints, inherited by every crate.** Clippy's `unwrap_used`,
   `expect_used`, `panic`, `print_stdout` and `print_stderr` are denied
   outside tests (`clippy.toml` allows them in tests). An invariant that
   justifies an `expect` is written in an `#[allow(…, reason = "…")]` next to
   it. `unsafe_code` is denied except in `pigeon-sys`, which denies unsafe
   blocks without a `SAFETY:` comment. `missing_docs` warns, and CI turns
   warnings into errors. Output for people goes through `pigeon-i18n`
   (ADR 0028) and logs through `tracing` (ADR 0022), never `println!`.
6. **CI.** The `ci` workflow's `rust` job runs on `ubuntu-24.04` and
   `macos-latest` (13 §13.8): `cargo fmt --check`, `cargo clippy -D
   warnings`, `cargo test` (which runs the hegel-rust laws) and
   `scripts/check-core-deps`, all with `--locked`. `cargo deny` (ADR 0019)
   joins the job with the first third-party dependency.

## Consequences

- Parallel work conflicts only in `Cargo.lock` and in the members' own
  manifests; `Cargo.lock` is regenerated on rebase.
- A crate that does I/O cannot slip into the core unnoticed: adding one is
  an edit to the allow-list, seen in review.
- Every push compiles and tests on both platforms of the slice, at the cost
  of a macOS runner per push.
- The pin means a newer stable compiler is not used until someone upgrades
  it on purpose.
