# Changelog

All notable user-visible changes to pigeon are documented here. While the
version is 0.x, a milestone of the [roadmap](docs/roadmap.md) increments
the second number, and every other pull request the third.

## 0.3.5 — 2026-10-16

### Fixed

- After a crash during an incoming install, recovery finishes the intended
  file index update, including deletions, instead of leaving the old version.
- Creating or deleting a file while an incoming version is being prepared
  keeps the person's change for capture instead of replacing it.
- Restoring a deleted file can finish recovery after a crash without getting
  stuck on its missing index entry.

## 0.3.4 — 2026-10-16

### Changed

- `pigeon update --main` takes pigeon's git-annex from the latest release
  when main pins the same one, instead of building it for an hour on a
  computer that has none yet.

## 0.3.3 — 2026-10-16

### Added

- Tab completion in zsh, bash and fish: pigeon completes its commands,
  options and paths, and what the daemon lists (suggestion IDs, held
  changes, invitations, the group's machines, settings and their values).
  Add `source <(COMPLETE=zsh pigeon)` (or `bash`; for fish,
  `COMPLETE=fish pigeon | source`) to your shell's startup file.

### Changed

- The command line checks each command's forms before asking the daemon:
  `pigeon held ID` needs one of `--share`, `--bring-back` and `--rename`,
  `pigeon resolve` exactly one choice, and `pigeon invite --list` no
  person; a wrong form prints that command's usage.

## 0.3.2 — 2026-10-16

### Added

- `pigeon update --main` installs main's latest commit, built on this
  computer with git and Rust, and builds pigeon's git-annex too when main
  changed it (taking the official one if that build fails). `pigeon
  --version` then names the commit, as in `pigeon 0.3.2+g1a2b3c4d5e6f`, and
  a plain `pigeon update` goes back to the latest release.

## 0.3.1 — 2026-10-16

The version follows the roadmap: `pigeon --version` says 0.3.1, where the
release of 15 October said 2.0.0-alpha.0. This release holds everything
merged since v0.3.0, the first steps of milestones M3 and M4.

### Added

- Releases are built from a tag, `pigeon-install` installs in one command,
  and `pigeon update [--version TAG | --rollback]` changes release.
- A group may name its own relay in the roster, in place of n0's.
- The web interface, served by `pigeond` and opened by `pigeon open`:
  Home, Suggestions (accept and decline), Files, a folder's files, a
  file's versions with *Restore this version* and *Open a copy*, Deleted
  files, People & machines (invite, cancel an invitation, remove a
  machine), and Settings.
- The French catalogue; the command line and the web speak the `language`
  setting.
- Notifications, and the launcher.
- `pigeon settings [KEY [VALUE]]`, `pigeon pause [DURATION]` and
  `pigeon resume`.
- `pigeon people` and `pigeon deleted [PATH]`.
- `pigeon invite --list` and `pigeon invite --cancel ID`.
- `pigeon leave`, which asks first.
- Held mass deletions and held names: `pigeon held` and `pigeon held ID
  --share | --bring-back | --rename`.
- `pigeon export --to DIR`: the plain files, without `pigeond`, each
  object checked.
- `RECOVERY.txt` on each backup machine.
- `pigeon free --without-checking PATH`.
- `pigeon view PATH [--version N | --group]`.
- `pigeon refound`, and joining a refounded group in place.
- `pigeon restore PATH --at TIME`.
- Rounds as OTLP-JSON traces, through `pigeon-telemetry`.

### Changed

- Saves not yet captured are preserved, and retained history is
  protected.
- An aside that no process writes and that still holds the old content is
  released at once, not after the keep period, so a laptop no longer holds
  two copies of each new file for ten minutes.

## 0.3.0 — 2026-10-15

Milestone M2, several laptops and several people
([report](docs/reports/people-3.md)): laptops install incoming versions,
the catalogue merge, invitations of people, suggestions and their
decisions, roster forks that merge, concurrent edits kept as conflicts,
add-only folders, and renames.

## 0.2.0 — 2026-10-15

Milestone M1, trust and durability ([report](docs/reports/trust-2.md)):
signed commits, `clock.json`, placement in queue order, the problem
catalogue, status reports, versioned formats, and pigeon's own git-annex.

## 0.1.0 — 2026-10-15

Milestone M0, the safety slice ([report](docs/reports/slice-1.md)): one
person, one laptop and two backup machines keep files safe with K = 2.
