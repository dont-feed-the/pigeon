# Changelog

All notable user-visible changes to pigeon are documented here. Changes
not yet released are under *Unreleased*. While the version is 0.x, a
release that completes a milestone of the [roadmap](docs/roadmap.md)
increments the second number, and every other release the third.

## Unreleased

### Added

- Settings → Use of this computer and `pigeon settings resources.*` offer
  Gentle, Balanced and Fast presets, upload/download ceilings, power and
  metered-network choices, and local full-speed/integrity-check windows.
- Content transfers share adaptive upload/download limits across groups.
  Linux laptops wait for power or a non-metered network when their settings
  call for it; history and capture continue, and status explains the wait.

### Fixed

- Repeated in-place saves keep each captured version intact while it awaits
  backup, including when status checks or index refreshes inspect the file.
- Status checks and protection planning finish sooner for folders with many
  small files, and slow copy-location checks leave new saves available.
- A slow content transfer leaves later saves and history requests available;
  normal background downloads no longer keep the group's worktree locked.
- Heavy native commands can be suspended and resumed by resource policy
  without spending their active operation timeout. Cancelling a suspended
  command also stops its helpers.
- History and saved copies of absent files work when the shared folder's
  location uses a path alias, including `/tmp` on macOS.

## 0.6.1 — 2026-10-17

### Fixed

- `pigeon doctor` continues through backup integrity-check observations to
  report problems such as unavailable capacity or a refused signing key.
- A computer still in a group whose roster predates the supported format
  is told which group when it tries to join, create or recover another:
  *This computer is already in the group “48h”* instead of an empty name.

## 0.6.0 — 2026-10-17

### Added

- Review the exact versions covered by `pigeon erase PATH` or
  `pigeon erase --suggestion ID`, including content still needed elsewhere.
  The command and web pages show preparation, confirmation, waiting backup
  machines and cancellation. Preparation never submits a later erasure.
- Backup machines check retained files every 30 days during the nightly
  window, using idle I/O priority on Linux. Set `resources.scrub_window`
  to choose another local-time window.

### Fixed

- Erasure keeps bytes still named by a private staged file on this computer,
  including a staged copy whose working file is absent.
- While saved history is being verified after a change, status explains
  that copying is waiting and what to do if the wait continues.
- When a current version needs contents covered by an earlier erasure and
  no copy is known, its deciders see *Needs you* with recovery advice.
  The unavailable file still counts as missing, including while its new
  version waits to be installed. Separate private saves remain visible.
- Restoring an already retained version continues while erasure metadata is
  being checked. Browser erasure receipts link directly to waiting requests,
  and restoration explains how older contents become a new current version.
- Refounding keeps verified completed erasure in the signed archive. Pending
  requests stay with the old group; the new group asks for fresh preparation
  after its backup machines rejoin.
- New versions keep arriving while erasure is prepared or old groups are
  checked. Routine peer metadata fetches no longer interrupt those transfers.
- If computers agree to conflicting erasure preparations, the preview
  explains why it cannot proceed and how an admin can recover explicitly.
- History labels versions released by a completed erasure as erased. Restore
  receipts and pending requests explain that later saved or restored versions
  are kept without cancelling the earlier request.
- If restored contents are unavailable, the new saved version or suggestion
  remains requested across restarts. The command explains the wait, keeps
  existing local files, and can finish when a copy becomes available.
- A save, deletion or executable-bit change made while restore checks history
  stays in place, even if pigeon has already saved that change. Members can
  retry a waiting restore without losing their intervening saved suggestions.

## 0.5.16 — 2026-10-17

### Fixed

- `pigeon join` can finish an interrupted join with the same invitation and
  folder. Failed attempts clean up their files while retaining the admitted
  identity for a retry. Computers stuck with an older unfinished join can
  retry after updating.
- `pigeon leave` can forget groups whose roster predates the supported
  format, keeping their files so the computer can join or create a group.

## 0.5.15 — 2026-10-17

### Added

- Files whose copy on this computer differs from the group's version are
  listed on Home. Each one's page explains the difference and lets you take
  the group's version, keep yours as a copy next to it, or suggest yours
  again, saying where each version goes (as `pigeon conflicts` and
  `pigeon resolve` do). A file's page also marks its current version.
- Suggestions can be decided file by file. Each file of a suggestion says
  what changes in it (new, changed with its size before and after and, for
  small text files, the lines added and removed, deleted or renamed) and
  has its own Review button. Untick files, then choose *Accept selected* or
  *Decline selected*, with a note; with several suggestions, *Accept all*
  and *Decline all* decide them at once. From the command line:
  `pigeon accept ID FILE…` or `pigeon decline ID FILE… --note "why"`.
- Whoever made a suggestion sees, on Suggestions, on each file's page and
  in `pigeon suggestions`, which of their files were accepted or declined,
  by whom, and the note.
- *Free up space* first says which files will leave this computer, which
  backup machines keep them, and that *Keep on this computer* brings them
  back. If no backup machine has been in touch for an hour, it asks a
  second time and says when they were last heard from. `pigeon free` asks
  the same question, or takes `--yes`. Freed files read "Safe, not on this
  computer".
- While sharing is paused, every page says "Sharing is paused until 14:38"
  with *Share again*, even when something else needs you, and
  `pigeon status` adds the pause under any other headline.
- `pigeon create --person NAME` and a "Your name" field on the Create form
  set how the others in the group see you; without it pigeon uses your user
  name, else the computer's name, never "Me".
- A printed recovery sheet shows its own name, and People & machines lists
  it as "paper recovery sheet printed on <date>".
- Every page starts with a "Skip to content" link for keyboard users.
- `pigeon open` looks and works like a file manager. A sidebar holds the
  sections and, on every page, whether your files are safe. **Files** shows
  the folder you are in as a table, folders first, each with an icon by
  kind, its state ("All safe", "2 not safe yet", "Only on this computer"),
  its size and when it last changed; click a column to sort, type in the
  box to filter by name, and follow the trail at the top to go back up.
  Select a row (or its ⓘ) to see its details beside the table: its state,
  who can change it, and a file's latest versions, each with *Open a copy*
  and *Restore this version*, or a folder's *Keep on this computer* and
  *Free up space*. Actions now run without leaving the page and say their
  result in a message at the bottom. Dates show in your own time zone, and
  the interface follows your system's light or dark mode.
- Choose which folders are on this computer in `pigeon open`: every folder
  in **Files** says whether it is on this computer, and whose it is when it
  is someone else's folder ("Robin’s folder, not on this computer"), with
  *Keep on this computer* or *Free up space*; a folder you chose says
  "Your choice", with *Use the default*. **Folders on this computer**, under
  Files, shows all the group's folders as a tree with a box each: tick or
  untick them, folders inside following, and *Apply*. Folders not on this
  computer stay safe on the backup machines, and freeing one waits until
  its files are safe there.
- A folder's README shows under its files, as on code-hosting sites:
  `README.md` (or `README.markdown`) as formatted text, else `README` or
  `README.txt` as plain text. Its links to files and folders of the group
  open them in pigeon; pictures and links elsewhere are shown as links,
  never loaded. A README whose folder is not on this computer, or longer
  than 128 KB, says so, with a link to its page.

### Changed

- Files not shared yet say why: your copy differs from the group's version;
  its name does not work on every computer; it is a deletion waiting for
  your choice; or pigeon could not save it and will try again.
- When the backup machines a file waits for have not been heard from for a
  few minutes, `pigeon status` and Home say which one, instead of
  "Sending…".
- Removing a backup machine asks its own question and, if fewer backup
  machines than the group needs would be left, says plainly that your files
  will not be safe until you add one. Removing a computer says its files
  stay on it but are no longer shared, and that you can invite it again.
- Preparing a recovery sheet first checks that a backup machine answers; if
  none does, it says which to turn on, and nothing is changed.
- Times read as local times ("until 14:38", "expires tomorrow at 13:35")
  and sizes as "29.3 GB" ("29,3 Go" in French). Held deletions and the Space
  page name files and dates instead of internal ids.
- Settings shows only the pause action that applies. Copies opened with
  *Open a copy* are named in your language.
- When the computer name you chose was taken, joining tells you the name it
  got.
- Screen readers hear which version, computer, suggestion or folder a
  repeated button acts on. On phones, buttons, fields and menu links are
  easier to tap. A folder's page is titled with its name, shows names from
  that folder, and *Back* goes to the parent folder.
- Building pigeon from source now also needs Node.js 22.12 or later with
  npm, which build the web interface's script; `pigeon update --main` and
  `scripts/install-local` say so before building if it is missing. The
  released programs need nothing new.

### Fixed

- A Mac that runs `pigeon join` without `--name` is listed under its own
  name, such as "Robin’s MacBook Air-49d313", instead of "backup-49d313",
  which made a laptop look like a backup machine; `pigeon create` names it
  "Robin’s MacBook Air" instead of "computer". On a Mac pigeon takes the
  name of System Settings → General → Sharing, elsewhere the host name.
- "Sending…" no longer outlasts its ten minutes because other files were
  saved meanwhile: each file's wait counts from when it was saved.
- A computer removed from the group no longer says "Everything is safe": it
  says it was removed, that its files stay in its folder but are no longer
  shared or backed up, and to join again with the group's invitation line.
- The Space page no longer warns of "less than 25% spare" whenever the
  group has exactly as many backup machines as it needs. It warns only when
  a storage limit is below one copy, and otherwise says, as information,
  that there is no spare backup machine.
- A join that fails or cannot reach the inviter no longer leaves a
  half-joined group: trying again in the same folder works.
- A recovery sheet whose page was closed while waiting no longer stays in
  the group unprinted: pages finish their work, or undo it, even when the
  browser leaves.
- The file manager's script, stylesheet and icon load when it is opened
  through a bridge client's own pages.
- A slow or unavailable computer no longer delays accepting new versions
  from the group’s reachable computers, or installing their contents when
  those are already on this computer.

## 0.5.14 — 2026-10-16

### Added

- The operator dashboard (`telemetry/dashboards/pigeon-operations.json`)
  shows computers and groups by name, such as "rtx", instead of their
  64-digit IDs, one line per computer, with a Group selector: the
  collector adds the names from its signed roster when it forwards.

- In a per-person folder, a computer no longer keeps other people's
  folders unless its person chooses to: Laurent's laptop gets `src/laurent`
  but not `src/robin`, and the folder of someone who joins later arrives
  nowhere unasked. `pigeon keep ~/Group/src/robin` brings one to this
  computer, `pigeon free` sends it away again, and `pigeon sync` lists every
  folder with whether it is on this computer and why;
  `pigeon sync PATH --default` returns a folder to its default.
- pigeon updates itself. Every computer and backup machine installs the
  latest release once a day, and within minutes when another computer of
  the group runs a newer release. The update downloads the installer
  afresh from GitHub, so a release that broke pigeon is replaced by the
  next one. It shows nothing; `journalctl --user -u pigeon-update`
  (macOS: `~/Library/Logs/pigeon/update.log`) tells what it did.
  `pigeon update --auto off` turns it off and `--auto on` back on; to
  install with it off, use `curl … | sh -s -- --auto off`. A build of main
  is never replaced. Installations from before this release get it after
  one `pigeon update`.

### Changed

- `pigeon update` updates the installation it belongs to, wherever it is.
  On a backup machine, `sudo /opt/pigeon/bin/pigeon update` now works too.

### Removed

- The Homebrew tap. On a Mac installed with Homebrew, run
  `brew services stop pigeon` and `brew uninstall pigeon`, then the
  install line of the README; your folders, groups and device key stay.

### Changed

- Release validation runs the same 50 random crash schedules in parallel
  within each Linux shard, using different seeds. Longer filesystem trace
  checks first try a faster witness search, retaining the full search and
  all safety checks if it cannot explain the observations.

## 0.5.13 — 2026-10-16

### Added

- Per-person folders: `pigeon folder-rule ~/Group/src --per-person` (or
  *Each folder in it belongs to the person it is named after* in a folder's
  sharing rule in `pigeon open`) makes every `src/<name>` belong to the
  person of that name, including people who join later; others' changes
  there become suggestions. A folder's name is the person's name in
  lowercase letters and digits (Laurent's is `src/laurent`), and
  `pigeon folder-rule ~/Group/src` lists them. The first such rule moves the
  group to newer rules: pigeon lists the other computers with the pigeon
  they last reported and asks first, as a computer running an older pigeon
  waits for the group's changes until it is updated.
### Changed

- A computer is now in one group only. `pigeon create`, `pigeon join` and
  `pigeon recover` (and joining or creating in the web interface) refuse a
  second group, before writing anything, and say how to leave the first:
  `pigeon leave`, or *Leave the group* in Settings in `pigeon open`. Before,
  a second group was created, but `pigeon open` and the commands that take
  no folder only ever showed the first one.
- `pigeon leave` on a computer that is the only machine of its group leaves
  it at once, even as the group's last admin computer; its files stay in
  the folder. It used to refuse, so a group created by mistake could not be
  left.
- Pull requests run the checks relevant to their changed files. Documentation,
  installer and formal-model changes avoid unrelated expensive jobs, while
  shared dependencies, unknown paths and CI changes retain broader checks.
  Releases and periodic validation still run the full set.
- Client revocation addresses the saved signing key directly, so a change to
  a computer's name cannot redirect its roster removal.

### Fixed

- Files no longer stay "receiving" when a peer was initially unavailable
  and git-annex disabled its remote. Pigeon re-enables peers still in the
  group when checking their settings, so transfers can resume.
- On Linux with a umask of 002, the default on Ubuntu and Fedora desktops,
  pigeon thought every file might be in use: it delayed writing the group's
  versions into your folder, `pigeon free` and *Free up space* deleted
  nothing, and `pigeon restore` said the file was in use. Files are now
  only held back when another user's program could really be writing them.
- A computer that left its last group and then created or joined another
  one, without restarting pigeon, no longer changes identity at the next
  restart, which made the new group no longer recognise it.
- A client-management page, download or conflict action queued behind client
  revocation now checks the updated access state before contacting that peer.
- Reusing a shared content object after an interrupted publication completes
  its directory sync before reporting success.
- Pending suggestions keep their author's identity after the signing client
  is revoked, so the person's remaining devices can still manage them.
- Concurrent saves from separate clients no longer leave a hidden conflict
  that stops later synchronization. Existing affected peers recover when the
  saved version is safely retained under its conflict filename.

## 0.5.12 — 2026-10-16

### Fixed

- Existing shell startup files using `pigeon completions zsh`, `bash` or
  `fish` work again, without the "not in this build" warning. They load
  the current Tab completion, even when the daemon is stopped.
- Diagnostic bundles hide the group's permanent invitation secret in both
  this computer's information and parts shared by other computers. Sharing
  diagnostics keeps the roster readable without sharing permission to join.

## 0.5.11 — 2026-10-16

### Changed

- `pigeon create` now prints one line to share with everyone you want in the
  group. Run on another computer, it installs or updates pigeon, then joins:
  it asks which folder to use and your name in the group.
  `pigeon invite` prints the same line again, on any computer of the group,
  and People in the web interface shows it with a *Copy* button.
- The line works for everyone and never expires. Whoever has it can join
  under any name, with the rights of the person of that name, so share it
  only with the group. Giving the name of someone already in the group adds
  your computer to them, once you confirm (`--yes` confirms on the command
  line). Add `--backup-machine` at the end of the line on a backup machine.
- `pigeon join` asks for your name in the group (`--person NAME`) and, on a
  terminal, for the folder.
- A refounded group has its own line; the old one no longer works.

### Removed

- One-time invitations: `pigeon invite PERSON`, `--admin`,
  `--backup-machine`, `--list` and `--cancel`, and the `pgn1-` codes. Groups
  made by earlier releases cannot be read by this one: create the group
  again.

## 0.5.10 — 2026-10-16

### Fixed

- `pigeon update` on macOS no longer leaves pigeond stopped. It failed with
  "Bootstrap failed: 5: Input/output error" when macOS had not yet finished
  stopping the old pigeond; the update now waits for that, up to ten
  seconds, before starting the new one.

## 0.5.9 — 2026-10-16

### Changed

- Routine verification uses checked proofs for two more token hand-off
  cases and for suggestion adoption permissions. Their large finite checks
  remain available in the weekly audit and on request.

### Fixed

- Release validation waits for Free to finish before checking its final
  protection status.

## 0.5.8 — 2026-10-16

### Changed

- Release verification uses a parameterized proof for token hand-offs and
  balances the remaining full model checks across runners. Large finite
  checks already covered by proofs run weekly and on request.

## 0.5.7 — 2026-10-16

### Changed

- The web interface's Join and Create forms explain what is wrong in words
  and show the form again with what you typed. A folder must be a full
  path; `~` means your home folder. An invitation pasted wrapped over
  several lines, or as the whole `pigeon join …` line, is understood.
- A member who tries to invite a new person is told that only an admin
  can, instead of seeing "{ $inviter } may no longer invite".
- Each page of the web interface has its own browser tab title; unknown
  addresses show a page that says so; long file names wrap instead of
  widening the page; Home announces its headline to screen readers, says
  to reload when JavaScript is off, and lists what is not safe yet right
  under its headline. Pages follow the system's dark mode.

### Fixed

- Bridge writes wait briefly when Git is updating its index in the
  background, using the same bounded wait as other saved changes.
- Files whose names have accents or other non-ASCII letters, such as
  `été.txt`, are shared and protected even when pigeon runs without a
  UTF-8 locale, as under launchd or `brew services`. Before, one such
  file kept every new change on Home at "Checking N changed files"
  indefinitely, with no error shown.
- Home shows the number of suggestions awaiting a decision steadily,
  instead of "Not checked yet; pigeon may be busy" most of the time.
- Creating a group with an empty name, zero backup copies or a relative
  folder is refused before anything is written; before, it could leave
  a half-made repository behind, outside the folder you meant.
- A computer that has just joined says a backup machine "has not been
  heard from yet" instead of "has been offline since ." with no date.
- Leaving a group removes only placeholders of files whose content is
  elsewhere; a file you changed keeps your bytes, as the confirmation
  promises.
- The *Check installation* page has its own title, the diagnostics
  consent is no longer printed twice, and French plurals and wording are
  corrected on the Space page, Home's machine details and *Free up space*.

## 0.5.6 — 2026-10-16

### Changed

- Releases wait for Linux validation while macOS tests run periodically in
  the background. Linux and macOS binaries are still built and packaged.
- The release publication report measures time since an installable release,
  candidate lead time and the jobs delaying publication. Tags do not count.

## 0.5.5 — 2026-10-16

### Fixed

- macOS release validation uses separate folders for joined files and local
  settings, including on disks that ignore letter case. Restore validation
  waits for the completed telemetry export before checking its spans. Bridge
  checks use short temporary paths that fit macOS socket limits.

## 0.5.4 — 2026-10-16

### Changed

- Nextcloud clients open the regular Pigeon web interface through the bridge,
  with Files, version history, protection status, suggestions and settings
  for the selected client. Client management and conflict resolution remain
  available from every peer page.

## 0.5.3 — 2026-10-16

### Changed

- Nightly verification uses the parameterized purge safety proof; its large
  finite cross-check runs weekly, on releases and on request.

## 0.5.2 — 2026-10-16

### Changed

- Pull requests and main pushes use Ubuntu runners. Version tags also run
  macOS validation, including installation, rollback and uninstall checks.

### Added

- Optional catalogue traces let bridge operators check complete listings and
  revision-pinned refreshes against a TLA+ model.
- Nextcloud desktop and mobile clients can connect through `pigeon-bridge`.
  Each authorization creates its own Pigeon peer. The bridge's web portal
  lets users review suggestions, resolve conflicts and revoke a client.
- A bridge can reuse a laptop or custodian's immutable file storage. The
  first-custodian quick start explains server setup and client connections.

### Fixed

- The Linux system-install check captures the expected refusal from another
  account on the CLI's error channel before checking the service user.
- macOS release validation now accounts for Git refreshing a visible file
  after Keep is interrupted, while still checking journal recovery and
  rejecting unsafe transitions.
- Leaving waits for a peer that learned the device's membership; an older
  roster cannot acknowledge a removal it has never seen.
- Storage sharing leaves git-annex lock files and retention timestamps intact;
  only actual file content is deduplicated.
- Connecting a bridge to existing storage finishes sharing retained objects
  before registration succeeds, even when background transfers are stalled.
- Nextcloud desktop clients can finish chunked uploads using their relative
  destination paths.
- New clients can join after suggestions have been accepted or declined;
  joining fetches their signed proposal evidence before checking the history.
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
