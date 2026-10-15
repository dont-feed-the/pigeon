# Instructions for agents

> Mission: what an agent does in this repository besides the change itself.

How to build, check and release is in [docs/development.md](docs/development.md).

## Every pull request bumps the version and records its change

1. **Bump the version once per pull request.** It is `version` under
   `[workspace.package]` in the root `Cargo.toml`. While the version is
   0.x, increment the third number; a pull request that completes a
   milestone of the [roadmap](docs/roadmap.md) increments the second and
   resets the third (M3 gives 0.4.0). Run `cargo check --workspace` so that
   `Cargo.lock` follows.
2. **Add an entry at the top of [CHANGELOG.md](CHANGELOG.md)**, headed
   `## X.Y.Z — YYYY-MM-DD`. Sort the changes under `### Added`,
   `### Changed`, `### Fixed` or `### Removed`. Write for the people who
   use pigeon: what they can do now or what behaves differently, in plain
   words (ADR 0027), with the command or page it concerns. Mention a CI,
   test or model change only if it changes what users see.
3. **More commits on the same branch** update that entry; they do not bump
   again.
4. **When main moved ahead** (a merge conflict on the version or the
   changelog), take main's version and entries, bump above main's version,
   and put this pull request's entry on top.

A release is the tag `v` + that version, and the release workflow refuses
any other tag.
