# Changelog

All notable user-visible changes to pigeon are documented here. While the
version is 0.x, a milestone of the [roadmap](docs/roadmap.md) increments
the second number, and every other pull request the third.

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
