# Changelog

All notable user-visible changes to pigeon are documented here. While the
version is 0.x, a milestone of the [roadmap](docs/roadmap.md) increments
the second number, and every other pull request the third.

## 0.4.6 — 2026-10-16

### Fixed

- Refounding keeps the folder rules valid when their former owners or stewards are absent from the new group.
- A backup repository can join a refounded group in place, keeping its content and archived history.
- Refounding and rejoining finish after a restart if interrupted after
  their verified history is recorded. Existing files, backup objects and
  annex identity are retained; a rejected welcome leaves the old group intact.
- Invitations stay bound to their original group. The same computer can
  retry after losing its welcome, while another computer cannot reuse the
  invitation. Unused invitations from older builds must be issued again.

## 0.4.5 — 2026-10-16

### Added

- Prepare and print a recovery sheet with a QR code from People & machines. The sheet’s instructions follow your language; the terminal prints the same code.
- People & machines recommends a sheet or a second computer for an admin with one signing computer, and a replacement sheet when backup machines change.

### Fixed

- Preparing a sheet before adding a backup explains the missing step. A sheet that no backup receives is revoked rather than counted as a usable paper key.
- Each prepared paper key has a unique date and identifier, so preparing a replacement on the same day works.

## 0.4.4 — 2026-10-16

### Added

- `pigeon debug bundle --from NAME` collects a chosen computer's diagnostics
  after its user allows one bundle with `pigeon debug allow NAME` or in
  Settings. Permission expires after ten minutes, works once, and covers
  only the shared group. Nothing is exported by default.
- Diagnostic collection asks before including file names and keeps the
  resulting bundle in a private local file.

### Fixed

- A busy computer can share diagnostics after a fresh permission even when
  its telemetry history exceeds the transfer limit. Older records are
  omitted explicitly so the newest complete records still arrive.

## 0.4.3 — 2026-10-16

### Fixed

- File history, restore and read-only views include verified versions from
  the archived group after refounding.
- Restore, recovery, sheet generation and removal show their ongoing action
  while they wait for group checks or a backup.
- Recovery sheets are shown as paper keys, with a revoke action, without
  an offline-computer warning.
- `RECOVERY.txt` quotes repository paths and sets a local git identity so
  its plain git-annex commands work on a fresh computer.

## 0.4.2 — 2026-10-16

### Fixed

- `pigeon recover` signs a new computer's roster entry with the recovery
  sheet's key. The surviving backup pulls and verifies that entry, so a
  backup belonging to another member can restore an admin without gaining
  that admin's authority.
- Recovery checks the custodian's signed history before writing files and
  admits recovery keys only to a bounded, read-only metadata exchange.

## 0.4.1 — 2026-10-16

### Added

- Admins can choose a private diagnostic collector with `pigeon settings
  telemetry-device NAME`, or stop collection with `off`. It receives only
  that group's diagnostics from its current members.
- A collector can forward to an OTLP HTTP backend explicitly configured on
  its own computer. An optional operator dashboard shows file states,
  annex latency and failures, queued work and transport throughput.
- Diagnostic traces connect CLI requests, daemon work, git children and
  peer streams. Debug bundles include buffered OpenTelemetry records.

## 0.4.0 — 2026-10-16

Milestone M3, reach and installation, with early M4 usability fixes
([installation evidence](docs/reports/reach-4.md),
[usability sessions](docs/reports/usability-5.md)).

### Added

- Backup machines can install with `pigeon-install --system`, which runs
  pigeon as its own account under systemd.
- `pigeon-install --uninstall` removes the programs and service while
  keeping shared folders, repositories, settings and the device key.
- Apple Silicon Macs can install through the Homebrew tap and run pigeon
  with `brew services start pigeon`.
- The web interface can create a group or join one by invitation, asking
  again before joining a refounded history.

### Fixed

- Joining checks every commit’s signature and permissions before adopting
  the invited history, including recovery and refounded joins.

- Service configurations handle installation paths containing spaces and
  special characters.
- Status sees newly saved changes before capture finishes and does not
  call them safe. With too few backup machines, it explains how to add one.
- A recent conversation with a laptop no longer makes an older backup
  safety check appear recent.
- Files at the top of the shared folder are visible in the web interface,
  with links to their versions.
- `pigeon open` starts the personal daemon when it is stopped. Settings
  confirmations use translated labels and explain when safety checks age.

- Releases and checks can recover the exact tested git-annex dependency
  archive when GitHub evicts its cache, after checking its source, producer,
  digest, architecture and version.

## 0.3.5 — 2026-10-16

### Fixed

- The macOS release is built again: 0.3.4's release failed to build
  pigeon's git-annex for macOS, so 0.3.4 was never published. 0.3.5 holds
  the same changes.

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
