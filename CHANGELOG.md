# Changelog

All notable user-visible changes to pigeon are documented here. Changes
not yet released are under *Unreleased*. While the version is 0.x, a
release that completes a milestone of the [roadmap](docs/roadmap.md)
increments the second number, and every other release the third.

## Unreleased

### Fixed

- A device opening incomplete peer requests leaves initial connection and
  header capacity available for other devices. Accepted transfers keep
  their existing limits.

## 0.6.39 — 2026-10-18

### Added

- Debug diagnostics show file-ingestion, byte-check and backup-planning
  progress. Reference reports retain these phase counters and a bounded
  preparation timeline without file names or credentials.

### Changed

- Development builds optimize bridge password hashing with the same
  algorithm and password settings.

## 0.6.38 — 2026-10-18

### Added

- With debug logging enabled, capture diagnostics show whether filesystem
  hints trigger a scan, without recording file names.

### Fixed

- An observed file deletion wakes background capture even if its filesystem
  notification was lost. Repeated views stay quiet, and held deletions
  still wait for a decision.


## 0.6.37 — 2026-10-18

### Fixed

- Saving many files starts fewer Git processes when checking captured bytes,
  while keeping the same file, content and executable-mode checks.

## 0.6.36 — 2026-10-18

### Added

- `scripts/check-upgrade` checks historical data with frozen old and new
  builds, including new saves and restores. Retired-format preservation has
  a separate result from a supported upgrade.

## 0.6.35 — 2026-10-18

### Fixed

- The persistent soak runner allows the specified ten-minute hot-file wait
  before requiring matching catalogues and retained file bytes.

## 0.6.34 — 2026-10-18

### Added

- Resource diagnostics can seed a test dataset in Fast mode, then sample
  with Gentle settings. These runs stay separate from budget qualification.

## 0.6.33 — 2026-10-18

### Fixed

- Incoming peer connections and streams have limits. Idle annex connections
  close without cutting off active file transfers.

## 0.6.32 — 2026-10-18

### Fixed

- Status keeps a new file pending while it is being staged for capture,
  instead of briefly reporting an empty safe folder.

- Capturing many saved files uses fewer Git processes while keeping batches
  bounded, with file names, bytes and executable permissions preserved.

## 0.6.31 — 2026-10-18

### Added

- Saved files wake background capture through native folder watching on Linux
  and macOS. Periodic checks continue if events are lost or watching is
  unavailable; status explains when pigeon is checking by scan only.

## 0.6.30 — 2026-10-18

### Fixed

- Linux capture checks open writers together, reducing repeated process
  scanning when many files are saved at once.

## 0.6.29 — 2026-10-18

### Fixed

- Saving a file can interrupt a background transfer that holds repository
  locks, allowing the save to finish instead of waiting on suspended work.
- Failed laptop joins preserve their unfinished folder and files added during
  setup. Retrying the same group keeps the original repository and identity.


## 0.6.28 — 2026-10-18

### Added

- Diagnostic logs include completed file-scan counts and durations, to help
  investigate resource use.



## 0.6.27 — 2026-10-18

### Fixed

- Printing a recovery sheet no longer reports an online backup as offline
  because a background fetch holds its cached history lock.

## 0.6.26 — 2026-10-18

### Fixed

- Failed backup joins retain their unfinished repository for retry, preserving
  files another application added during setup.

## 0.6.25 — 2026-10-18

### Fixed

- Invalid joining requests are refused promptly while a group is busy,
  without joining its queue of file operations.

## 0.6.24 — 2026-10-18

### Fixed

- Joining tries another admitter when one stops replying, and stops within
  its overall waiting limit.

## 0.6.23 — 2026-10-18

### Fixed

- Opening Files while a folder is being updated retries brief read conflicts
  automatically.
- Final checks of saved files follow the read delays of the drive containing
  the file, while still respecting system-wide disk pressure.

## 0.6.22 — 2026-10-18

### Fixed

- Incoming connections recover automatically if their network helper exits,
  without restarting pigeon. This carries forward the fix from the unpublished
  0.6.21 candidate.

- Protection work keeps its priority between files, so background scrubbing
  waits while the remaining files are being checked. Finishing or cancelling
  protection promptly lets waiting work resume.

## 0.6.21 — 2026-10-18

### Fixed

- Incoming connections recover if a background network helper stops
  unexpectedly, without restarting pigeon.

## 0.6.20 — 2026-10-18

### Added

- Large-file capture verification measures its own disk reads on Linux and macOS. When
  a device's recent median exceeds three times its learned quiet latency,
  heavy work backs off while capture keeps its progress floor.

## 0.6.19 — 2026-10-18

### Fixed

- Pasting an invitation with a long message no longer stalls while pigeon looks for the invitation.
- Background resource checks avoid repeated uncached disk reads when the preset’s native priority stays the same.

## 0.6.18 — 2026-10-18

### Fixed

- Choosing Fast now asks the installed user service for normal CPU and disk
  priority. Returning to Gentle or Balanced restores background priority.
  Switching between those native modes briefly restarts pigeon; switching
  between Gentle and Balanced keeps it running.

## 0.6.17 — 2026-10-18

### Fixed

- Large captures continue while their file bytes advance. Final byte verification shares capture’s disk backoff and cache limits, and stalled reads still time out.

## 0.6.16 — 2026-10-17

### Fixed

- Large file transfers keep running while their bytes advance; transfers that stop making progress still time out.
- On macOS, scheduled integrity checks start with throttled disk priority instead of failing before checking any files.

## 0.6.15 — 2026-10-17

### Fixed

- Between keys, kept-folder downloads yield to pending work that protects files, then resume without spending their timeout while waiting.
- Joining refuses unsupported protocol versions and unknown invitation or join fields instead of interpreting them as known authority. Invitation proofs use the constant-time comparison primitive.

## 0.6.14 — 2026-10-17

### Fixed

- Diagnostic output also hides pasted join invitations, partial socket messages, Git object contents and extra child-environment values.
- Diagnostic output hides recovery keys, stored invitation and authentication tokens, protocol data and private API messages.
- Removing a computer or backup machine now explains in English and French that its existing invitation still lets its holder join as a new computer.

## 0.6.13 — 2026-10-17

### Fixed

- Diagnostic formatting hides device signing keys, invitation secrets and repository authentication tokens.
- Repairs to damaged files can run alongside backup transfers and take priority over routine integrity checks.

## 0.6.12 — 2026-10-17

### Fixed

- Network diagnostics keep transfer counts and refusal reasons without copying file contents or connection passwords into the logs.

## 0.6.11 — 2026-10-17

### Changed

- Kept folders download in their configured high, normal or low priority, with newer versions first within each priority. Valid changes to `[groups.priority]` apply while pigeon runs.

## 0.6.10 — 2026-10-17

### Fixed

- pigeon backs up recent saves before older ones with the same copy needs,
  and refreshes pending transfers when new history arrives.

## 0.6.9 — 2026-10-17

### Fixed

- Saving new changes takes priority over transfers and integrity checks
  across groups. Transfers explain the wait and resume automatically.
- Repairing damaged contents also follows this computer’s resource settings.

## 0.6.8 — 2026-10-17

### Fixed

- `pigeon restore --at` refuses impossible calendar dates and invalid UTC
  offsets before choosing a version, leaving current files untouched.
- Joining as a backup machine on macOS explains that backups currently
  need Linux, instead of suggesting another APFS folder or an internal error.

## 0.6.7 — 2026-10-17

### Fixed

- A slow log destination no longer holds up the daemon. pigeon retains
  seven daily diagnostic files, and `PIGEON_LOG` accepts filters for
  individual components.

### Added

- Release-readiness notes link accepted decisions to their implementation
  and verification, and list the evidence still required.

## 0.6.6 — 2026-10-17

### Fixed

- `pigeon create` and `pigeon join` check the folder’s file system before
  making a repository. Unsupported or unreadable volumes explain how to
  choose another folder; `pigeon doctor` checks existing folders too.

## 0.6.5 — 2026-10-17

### Fixed

- Status checks and later saves remain available while a file or folder
  restore waits for its saved contents. A newer save is preserved if it
  changes the file before restoration can finish.

## 0.6.4 — 2026-10-17

### Fixed

- Historical versions can now count as safe when a backup independently
  merged the same captured history. `pigeon status` can recognize those
  equivalent merges despite their different IDs. Groups with erasure or
  refounding history keep their stricter acknowledgement check.

- Content checks ask the operating system to reclaim completed read pages
  while checking large files.
- Pending changes are described as not yet recorded by pigeon, to distinguish
  them from files already saved in an editor. The guide explains which rapid
  saves become history versions.

- Background capture waits for a file to settle before reading it and spaces
  captures of frequently saved files, while other stable files keep progressing.
  Asking for status preserves those waits.
- Status and install checks leave another file's unsettled save alone. Capture
  recognizes same-size edits even when an application preserves the file's
  modification time. Pending suggestions also detect later byte and executable
  mode changes, and rebuilding their cache keeps the accepted version intact.

## 0.6.3 — 2026-10-17

### Fixed

- Rejoining a backup can import its saved metadata while another fetch owns
  the peer's metadata-ref lock, without touching that lock.

## 0.6.2 — 2026-10-17

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

Earlier release notes: [v0.5.0 and earlier](docs/changelog/through-0.5.0.md).
