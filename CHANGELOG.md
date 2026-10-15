# Changelog

All notable user-visible changes to pigeon are documented here. While the
version is 0.x, a milestone of the [roadmap](docs/roadmap.md) increments
the second number, and every other pull request the third.

## Unreleased

### Added

- Nextcloud desktop and mobile clients can connect through `pigeon-bridge`.
  Each authorization creates its own Pigeon peer. The bridge's web portal
  lets users review suggestions, resolve conflicts and revoke a client.
- A bridge can reuse a laptop or custodian's immutable file storage. The
  first-custodian quick start explains server setup and client connections.

### Fixed

- A decision received before its suggestion no longer prevents that peer
  from receiving later edits once the suggestion arrives.

## 0.5.1 — 2026-10-16

### Fixed

- Rejoining a backup in place replaces its configured group correctly
  through directory aliases, including macOS's `/var` path. Other
  repositories and settings remain in their original order.

## 0.5.0 — 2026-10-16

Milestone M4, the interface and recovery, with the installation repairs
needed after the M3 candidate ([report](docs/reports/people-5.md)).

### Added

- Home explains current files and retained versions separately. The Space
  page and `pigeon status --capacity` show distinct storage needs, retained
  suggestions and dated reports from each backup machine.
- Read older and deleted files with History and View, restore an entire
  nested folder, and inspect a suggestion's actual contents before deciding.
- Admins can change folder sharing rules and print a private recovery sheet
  in English or French. Surviving backups can recover an admin; refounding
  retains old history, and export works without a running pigeon daemon.
- Free up space and Keep on this computer work in nested folder pages.
  A kept child overrides its freed parent without downloading its siblings.
- Doctor checks the transport helper and offers installation remedies in
  the terminal and Settings. Peer debug bundles require one-use consent.
- Optional group diagnostics connect requests, daemon work and network
  operations. The operator dashboard shows measured acceptance and
  protection delays, queues and transport observations.

### Fixed

- Joining, paper recovery and refounding retain the signed proposal proofs
  needed to verify earlier suggestion decisions.
- A temporary Git index lock no longer refuses a computer whose membership
  has already been committed; metadata repair resumes after a restart.
- People settings preserve unrelated private staged files and open saves.
  Files lists pending local work and excludes intentionally freed children
  from its promise to download files.
- Restore preserves executable modes and reports files still in use.
- Local diagnostics retain at most 256 KiB. Current operator measurements
  remain available even when verbose traces fill the export queue.
- Backup release validation uses the service account's private socket.
  Retrying publication builds the original tag and its matching sources.
- Temporary socket aliases and in-place join staging are created with
  owner-only permissions regardless of the account's usual umask.

## 0.4.3 — 2026-10-16

### Added

- The published purge protocol's safety guarantees are checked for arbitrary
  custodian counts and waiting periods, including restores, cancellations,
  restarts and delayed messages. The erase command is still not implemented.

### Changed

- Pull requests reuse checked model proofs when every formal input is
  unchanged, reducing repeated verification while changed models and the
  scheduled checks still receive fresh proofs.

## 0.4.2 — 2026-10-16

### Added

- Parameterized proofs check that capture preserves committed changes and
  safe drops keep the required copies, beyond the finite model sizes.
  Proof-covered drop checks still run independently each week and on releases.

### Fixed

- After a crash during an incoming install, recovery finishes the intended
  file index update, including deletions, instead of leaving the old version.
- Creating or deleting a file while an incoming version is being prepared
  keeps the person's change for capture instead of replacing it.
- Restoring a deleted file can finish recovery after a crash without getting
  stuck on its missing index entry.

- Leaving a group waits briefly for temporary Git index contention instead
  of failing before it can commit the computer's removal.

## 0.4.1 — 2026-10-16

### Changed

- Checks of a pull request take about ten minutes instead of 35: pigeond's
  daemon tests and the fast TLA+ models run in parallel shards, and the
  lab experiments join them. A push to main also runs the safety slice
  without its S9 crash schedules; release tags and a nightly run, skipped
  when main has not moved, run the full models and the S9 schedules.
- Only main saves build caches, so pull requests no longer evict the caches
  main and the releases rely on.
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

## 0.3.6 — 2026-10-16

### Fixed

- The release is published again: 0.3.5's release checks lost pigeon's
  git-annex build when GitHub's cache evicted it, so 0.3.5 was never
  published. They now take the build from the release itself. 0.3.6 holds
  the same changes as 0.3.5.

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
