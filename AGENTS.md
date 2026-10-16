# Instructions for agents

> Mission: what an agent does in this repository besides the change itself.

How to build, check and release is in [docs/development.md](docs/development.md).

## Every pull request records its change, and leaves the version alone

1. **Do not change the version in a pull request.** `version` under
   `[workspace.package]` in the root `Cargo.toml` changes only in a
   release commit (below), so that pull requests merged one after another
   do not conflict on it.
2. **Add an entry under `## Unreleased`** at the top of
   [CHANGELOG.md](CHANGELOG.md), adding that heading if it is missing.
   Sort the changes under `### Added`, `### Changed`, `### Fixed` or
   `### Removed`. Write for the people who use pigeon: what they can do now
   or what behaves differently, in plain words (ADR 0027), with the command
   or page it concerns. Mention a CI, test or model change only if it
   changes what users see.
3. **More commits on the same branch** update that entry.
4. **When main moved ahead** with entries of its own under
   `## Unreleased`, keep both: merge this pull request's items into main's
   subsections.

## A release sets the version

A release commit on main sets the version, runs `cargo check --workspace`
so that `Cargo.lock` follows, and renames `## Unreleased` to
`## X.Y.Z — YYYY-MM-DD`. While the version is 0.x, a release increments the
third number; a release that completes a milestone of the
[roadmap](docs/roadmap.md) increments the second and resets the third (M3
gives 0.4.0). The release is then the tag `v` + that version, and the
release workflow refuses any other tag.
