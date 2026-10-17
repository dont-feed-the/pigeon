# pigeon's English messages (ADR 0028). The words follow the vocabulary of
# docs/design/ux.md §2.2. Each id is listed, with its variables, in
# crates/pigeon-i18n/src/ids.rs; a test checks that both lists agree.

## The states of a version (ux.md §2.1, ADR 0027)

state-local = Only on this computer
state-published = Shared, not safe yet
state-protected = Safe
state-uncertain = Not checked since { $since }
state-missing = No copy can be found
state-erased = Erased
state-not-here = { $state }, not on this computer

## Headline levels (ux.md §4)

level-needs-you = Needs you
level-not-safe = Not safe yet
level-working = Working
level-safe = Safe
level-not-checked = Not checked recently

## Headlines: what, then why and what next (ux.md §4)

headline-safe = Everything is safe. Checked { $ago }.
headline-capturing =
    { $files ->
        [one] Checking 1 changed file; it is not safe yet.
       *[other] Checking { $files } changed files; they are not safe yet.
    }
headline-checking = The local folder could not be checked. Run: pigeon doctor
headline-inventory-unreadable = Pigeon cannot check every saved version right now.
headline-too-few-backups = Not safe yet: add a backup machine.
why-capturing = This change is waiting to be saved and copied to the backup machines.
why-differs = Your copy differs from the group's version. Choose which to keep.
why-name = Its name does not work on every computer. Rename it.
why-held-deletion = It was deleted along with many other files. pigeon waits for you to choose: delete them for everyone, or bring them back.
why-capture-failed =
    pigeon could not save this change: { $why ->
        [disk-full] the disk is full
        [no-access] it is not allowed to read the file
        [busy] another program was using the folder
       *[other] something went wrong
    }. It will try again.
why-removed = This computer is no longer in the group, so this file is not backed up.
why-too-few-backups = { $have } of { $need } backup machines are set up. To add one, run on it the line pigeon invite prints, with --backup-machine at its end.
headline-not-safe =
    { $files ->
        [one] 1 file version is not safe yet.
       *[other] { $files } file versions are not safe yet.
    }
headline-working =
    { $files ->
        [one] Sending 1 file version to the backup machines…
       *[other] Sending { $files } file versions to the backup machines…
    }
headline-not-checked = Last confirmed safe { $when }. pigeon can't reach the group right now; changes since then are only on this computer.
headline-disk-full = Your disk is full. pigeon can't save new changes. Free up space or empty the trash.
headline-no-capacity = This backup machine has no room for { $files } files. It needs { $missing_mb } MB more free space.
headline-machine-missing =
    { $days ->
        [one] { $machine } has been missing for 1 day.
       *[other] { $machine } has been missing for { $days } days.
    } Replace it or add another backup machine.
headline-machine-full = { $machine } is full; it waits until space is freed on it.
headline-machine-no-capacity = { $machine } has no room for { $files } files: it needs { $missing_mb } MB more free space. Free up space on it, or add another backup machine.
headline-machine-lost = { $machine } no longer finds its backup folder; it counts for nothing until it is set up again.
headline-removed = This computer was removed from the group. Its files stay in its folder, but they are no longer shared or backed up. To share them again, ask someone in the group for its invitation line and join again.
why-machine-offline = { $machine } was last heard from { $since }.
why-machine-not-heard = { $machine } has not been heard from yet on this computer.
why-safe-copies = { $have } of { $need } safe copies
next-when-back = Nothing to do: they will be safe when { $machine } is back.
next-repairing = Repairing: { $files } files are being copied to another backup machine.

## Relative times (ux.md §4); absolute times come with jiff

time-just-now = just now
time-minutes-ago =
    { $minutes ->
        [one] 1 minute ago
       *[other] { $minutes } minutes ago
    }
time-hours-ago =
    { $hours ->
        [one] 1 hour ago
       *[other] { $hours } hours ago
    }
time-days-ago =
    { $days ->
        [one] 1 day ago
       *[other] { $days } days ago
    }
time-never = never

## Times to come, in this computer's local time, and sizes (units.rs)

time-clock = { $hour }:{ $minute }
time-month =
    { $index ->
        [1] January
        [2] February
        [3] March
        [4] April
        [5] May
        [6] June
        [7] July
        [8] August
        [9] September
        [10] October
        [11] November
       *[12] December
    }
time-date = { $day } { $month }
time-date-year = { $day } { $month } { $year }
time-until =
    { $moment ->
        [today] until { $clock }
        [tomorrow] until tomorrow at { $clock }
       *[other] until { $date } at { $clock }
    }
time-expires =
    { $moment ->
        [today] expires at { $clock }
        [tomorrow] expires tomorrow at { $clock }
       *[other] expires on { $date } at { $clock }
    }
number-decimal = { $whole }.{ $tenth }
number-thousands = { $high },{ $low }
size-bytes =
    { $plural ->
        [one] { $size } byte
       *[other] { $size } bytes
    }
size-kb = { $size } kB
size-mb = { $size } MB
size-gb = { $size } GB
size-tb = { $size } TB

## Commands of the safety slice (13 §13.12)

cmd-create = Create a group with this computer as its first member
cmd-invite = Print the line that adds a person, a computer or a backup machine to the group
cmd-join = Join a group with its invitation
cmd-status = Say whether your files are safe, and why not
cmd-machines = List the group's computers and backup machines
cmd-open = Open pigeon in your browser
cmd-history = List the versions of a file
cmd-restore = Restore a version, or a deleted file
cmd-free = Free up a folder's space on this computer
cmd-keep = Keep a folder on this computer
cmd-sync = See which folders are on this computer; --default makes a folder follow its default again
cmd-suggestions = List the suggested changes waiting for a decision
cmd-accept = Accept a suggested change into the group's files; name files to accept only those and decline the others
cmd-decline = Decline a suggested change, saying why if you like; name files to decline only those and accept the others
cmd-conflicts = List your files that differ from the group's version
cmd-resolve = Choose for a file that differs: the group's version, yours as a copy, or suggest it again
cmd-remove-computer = Remove a lost or retired computer from the group
cmd-recovery-sheet = Print the recovery sheet for this group
cmd-recover = Recover the group on a new computer from its backup machines
cmd-doctor = Check the installation and explain what is wrong
cmd-debug-bundle = Gather diagnostics for someone helping you
cmd-update = Install the latest release of pigeon, a given one, main built from source, or the one before; or turn the automatic update on or off
cmd-completions = Print the shell script that enables Tab completion
opt-json = Print the result as JSON, for scripts
opt-backup-machine = For a backup machine rather than a person's computer
opt-not-shared = List what is not shared, and why
help-usage = Usage: pigeon COMMAND [ARGUMENTS]
help-commands = Commands:

## Results

free-result = Files safe and eligible to free: { $safe } of { $total }. Pigeon will free their space in the background; other files wait for safe copies.
free-waiting =
    { $files ->
        [one] 1 file is not safe yet; pigeon will free it once it is.
       *[other] { $files } files are not safe yet; pigeon will free them once they are.
    }
free-preview-leave =
    { $files ->
        [one] 1 file will be removed from this computer.
       *[other] These { $files } files will be removed from this computer.
    }
free-preview-stay =
    { $files ->
        [one] It stays safe on the backup machines: { $machines }.
       *[other] They stay safe on the backup machines: { $machines }.
    }
free-preview-none = No file here is safe on the backup machines yet, so nothing leaves this computer for now.
free-preview-undo = “Keep on this computer” brings them back whenever you want.
problem-backups-quiet =
    { $backups ->
        [one] The backup machine { $machines } was last in touch { $heard }, so pigeon can't confirm right now that it still has these files.
       *[other] The backup machines ({ $machines }) were last in touch { $heard }, so pigeon can't confirm right now that they still have these files.
    }
free-backups-never =
    { $backups ->
        [one] pigeon has not heard from the backup machine { $machines } yet, so it can't confirm that it has these files.
       *[other] pigeon has not heard from the backup machines ({ $machines }) yet, so it can't confirm that they have these files.
    }
hint-backups-quiet = If you go ahead, the files leave this computer as soon as pigeon hears from the backup machines again and confirms their copies, even if you are not there.
confirm-free-quiet = Free up space anyway? [y/N]
web-free-title = Free up space in { $folder }?
web-free-quiet-title = The backup machines have not been in touch
web-free-anyway = Free up space anyway
web-free-cancel = Cancel
keep-result = { $folder } will be kept on this computer. Files that are absent will download when a copy is reachable; check Home for progress.
suggestions-none = No suggestion is waiting.
suggestion-decide = { $id }  { $author } suggests a change of { $paths }, saved { $saved }. Accept it with: pigeon accept { $id }
suggestion-other = { $id }  { $author } suggests a change of { $paths }, saved { $saved }; someone else decides.
suggestion-yours = { $id }  Your change of { $paths }, saved { $saved }, waits for a decision.
accepted-suggestion = { $author }'s suggestion is now in the group's files.
declined-suggestion = { $author }'s suggestion is declined; they will be told.
decided-all-accepted =
    { $count ->
        [one] You accepted { $author }'s change ({ $paths }). { $author } will be told.
       *[other] You accepted { $author }'s { $count } changes ({ $paths }). { $author } will be told.
    }
decided-all-declined =
    { $count ->
        [one] You declined { $author }'s change ({ $paths }). { $author } will be told.
       *[other] You declined { $author }'s { $count } changes ({ $paths }). { $author } will be told.
    }
decided-some = You accepted { $accepted } of { $author }'s changes ({ $accepted-paths }) and declined { $declined } ({ $declined-paths }). { $author } will be told.
suggestion-file-added = { $path }: new file, { $after }
suggestion-file-changed = { $path }: changed, { $before } → { $after }
suggestion-file-changed-lines =
    { $path }: changed, { $before } → { $after }, { $added ->
        [one] 1 line added
       *[other] { $added } lines added
    } and { $removed ->
        [one] 1 removed
       *[other] { $removed } removed
    }
suggestion-file-deleted = { $path }: deleted (it was { $before })
suggestion-file-renamed = { $path }: renamed from { $from }, content unchanged ({ $after })
suggestion-files-hint = To accept some files only, and decline the others: pigeon accept { $id } FILE…
suggestion-outcome-accepted =
    { $count ->
        [one] { $by } accepted your change ({ $paths }), { $when }.
       *[other] { $by } accepted your { $count } changes ({ $paths }), { $when }.
    }
suggestion-outcome-declined =
    { $count ->
        [one] { $by } declined your change ({ $paths }), { $when }.
       *[other] { $by } declined your { $count } changes ({ $paths }), { $when }.
    }
suggestion-outcome-some = { $by } accepted { $accepted } of your changes ({ $accepted-paths }) and declined { $declined } ({ $declined-paths }), { $when }.
suggestion-outcome-someone = Someone
suggestion-outcome-note = Note from { $by }: “{ $note }”
suggestion-outcome-kept = The files declined stay as you saved them on this computer; nobody else has your version of them.
conflicts-none = Every file here is the group's version.
conflict-waiting = { $path }: your suggestion waits for a decision.
conflict-choose = { $path }: your version differs from the group's. Choose with: pigeon resolve
resolved-take-group = { $path } is the group's version again; yours stays in its history.
resolved-keep-copy = Your version of { $path } is kept as { $dest }; { $path } is the group's version again.
resolved-suggest-again = Your version of { $path } is suggested again.
web-conflicts = Files that differ from the group's version
web-conflict-row = { $path }: your copy here is different from the group's version. Open it to choose what to keep.
web-conflict-row-waiting = { $path }: your copy here is different from the group's version; your change waits for a decision.
web-conflict-heading = Your copy is different
web-conflict-explain = Your copy of { $name } is different from the group's version. Choose what to do; nothing is lost.
web-conflict-waiting-explain = Your copy of { $name } is different from the group's version: your change was suggested and waits for a decision. You can wait, or choose now; nothing is lost.
web-resolve-take-group = Take the group's version
web-resolve-take-group-help = This file becomes the group's version. Yours is not deleted: pigeon keeps it in its history, but no longer as a file in your folder. To keep it as a file, choose “Keep mine as a copy”.
web-resolve-take-group-waiting-help = Your suggestion is withdrawn and this file becomes the group's version. pigeon keeps yours in its history, but no longer as a file in your folder. To keep it as a file, choose “Keep mine as a copy”.
web-resolve-keep-copy = Keep mine as a copy
web-resolve-keep-copy-help = Your version is saved as a new file with the name below, next to this one, and shared with the group. This file becomes the group's version.
web-resolve-copy-name = Name of your copy
web-resolve-copy-suffix = my copy
web-resolve-suggest-again = Suggest again
web-resolve-suggest-again-help = Your version stays in this file and is suggested to the group again. The group's version stays as it is until your suggestion is accepted.
undo-free = Undo: pigeon keep { $folder }
undo-keep = Undo: pigeon free { $folder }
restore-result = { $path } is restored on this computer. Restoring older contents saves a new current version and keeps the existing history. Earlier erasure requests stay pending but do not cover this new version. Check Home for sharing and protection, and Suggestions if approval is needed.
restore-folder-result = { $path }: { $files } files restored on this computer. Newer files and saved history are kept. Check Home and Suggestions for sharing and approval.
problem-restore-folder-incomplete = { $path }: { $files } files restored; { $skipped } could not be restored and were left unchanged. The operation needs attention. Files to check: { $paths }. Close files in use and try again; use pigeon doctor for other problems. Check Home and Suggestions for sharing and approval.
problem-restore-in-use = { $path } changed or is being used. Pigeon left it unchanged. Open an old version with “Open a copy”, or close the file and try again.
problem-restore-waiting = Restoring { $path } has been requested, but its saved contents are not available on this computer. Local edits are kept. Check Home for available copies, and Suggestions if approval is needed. You can try restoring again when a copy becomes available.
problem-restore-folder-version = To restore a folder, choose a date with --at or omit the version option to restore its latest saved snapshot.
web-restore-folder = Restore this folder
web-files-arriving = Waiting for { $files } files to be available on this computer. Leave pigeon running; it will receive them when a copy is reachable.
web-restore-folder-help = Restore a whole deleted folder from its latest saved state. Newer files and saved history are kept. Individual files are listed below.
recovery-sheet-text = { $sheet }
recovered = This computer is back in { $group }. Its files arrive from the backup machines; then remove the lost computers, revoke the used recovery sheet with remove-computer, and print a new sheet.
not-shared-kind = { $path }: not shared ({ $kind })
doctor-lab-build = This is a lab build: its status reports are marked, and no release counts them.
doctor-version = { $program } { $found } (at least { $minimum } is needed)
doctor-ok = Everything pigeon needs is in place.
web-doctor = Check installation
problem-transport-helper-unavailable = Pigeon's transport helper is missing, cannot run, or belongs to a different release.
hint-transport-helper-unavailable = Reinstall this pigeon release with all its binaries, then restart pigeon and run pigeon doctor again.
created =
    The group “{ $group }” is created. To add someone, another computer or a backup machine, share this line; it installs pigeon and joins the group (on a backup machine, add --backup-machine at its end). Whoever has it can join, with the rights of the person they say they are:
    { $command }
invitation =
    To add someone to “{ $group }”, another computer or a backup machine, share this line; it installs pigeon and joins the group (on a backup machine, add --backup-machine at its end). Whoever has it can join, with the rights of the person they say they are:
    { $command }
joined =
    { $who ->
        [backup-machine] This backup machine now keeps copies for “{ $group }”.
       *[computer] This computer is now in “{ $group }”. The group's files arrive in its folder.
    }
joined-renamed =
    { $who ->
        [backup-machine] This backup machine now keeps copies for “{ $group }”.
       *[computer] This computer is now in “{ $group }”. The group's files arrive in its folder.
    } Another computer in the group was already called { $asked }, so this one is called { $name }.
removed-computer = { $name } is removed from the group; its copies no longer count.
removed-from-group = This computer is no longer in the group: this is the group as it last knew it. To share files again, ask someone in the group for its invitation line and join again.
bundle-written = The diagnostics are in { $path }. Nothing was sent.
machine-row = { $name }: { $kind }, last heard { $heard }
machine-kind-custodian = backup machine
machine-kind-computer = computer
person-row = { $name } ({ $role }): { $machines }
person-admin = admin
person-member = member
cmd-people = The group's members and their computers
cmd-deleted = The files deleted from the group, which restore brings back
machine-this = { $name }: this computer
version-row = { $number }. { $saved }: { $state }
waiting-row = { $path }: { $state }
confirm-remove-computer = Remove { $name } from the group? Its files stay on it but are no longer shared with the group, and its copies stop counting. You can invite it again later. [y/N]
confirm-remove-backup = Remove the backup machine { $name } from the group? The copies it keeps stop counting; the other backup machines take over. You can invite it again later. [y/N]
confirm-remove-backup-unsafe = Remove the backup machine { $name } from the group? { $left ->
        [0] After this, the group will have no backup machine. None of your files will be safe until you add one.
        [one] After this, the group will have only one backup machine, but it needs { $needed } for your files to be safe. Your files will not be safe until you add another one.
       *[other] After this, the group will have only { $left } backup machines, but it needs { $needed } for your files to be safe. Your files will not be safe until you add another one.
    } You can invite it again later. [y/N]
confirm-yes = yes
confirm-leave = Remove this computer from the group? Its files stay in the folder, but it stops sharing them. [y/N]
confirm-refound = Refound the group with you as its only admin? It becomes a new group: every other member joins it again by an invitation from you, and the old history stays readable. Completed erasures in that snapshot remain effective. Pending erasures do not continue in the new group; computers still in the old group may finish them there. Prepare erasing again after inviting your backups. [y/N]
confirm-join-refounded = { $group } was refounded by { $name } on { $date }: joining it means accepting { $name } as its admin. Join? [y/N]
web-join-refounded = { $group } was refounded by { $name } on { $date }. Joining it means accepting { $name } as its admin. Choose Join to continue.
confirm-held-share = Delete the files of { $batch } for everyone? They stay in the history. [y/N]

## Problems (10 §10.6, ADR 0023): problem-<code>, and hint-<code> when there is one

problem-code = Code: { $code }
problem-not-in-this-build = “{ $command }” is not in this build of pigeon yet.
hint-not-in-this-build = This early build has only the commands that keep files safe; the others come later.
problem-file-system-not-in-this-build = This folder is on { $fs }, which this build of pigeon does not support yet.
hint-file-system-not-in-this-build = Choose a folder on ext4 (Linux) or APFS (macOS).
problem-invite-unknown = This invitation is not one the group knows.
problem-invite-not-allowed = { $inviter } may not add this computer to the group.
problem-inviter-offline = { $inviter }'s computer can't be reached right now.
hint-inviter-offline = Try again when { $inviter }'s computer is on. Nothing was kept on this computer, so you can use the same folder.
problem-not-in-group = This computer is not a member of the group.
problem-not-allowed = You are not allowed to do this in this group.
problem-suggestion-unknown = No waiting suggestion is called { $id }.
hint-suggestion-unknown = See them with: pigeon suggestions
problem-suggestion-file-unknown = { $path } is not one of the files of this suggestion.
hint-suggestion-file-unknown = See its files with: pigeon suggestions
problem-no-files-chosen = Choose at least one file of the suggestion.
hint-no-files-chosen = See its files with: pigeon suggestions
problem-not-diverging = { $path } is the group's version already.
hint-not-diverging = See the files that differ with: pigeon conflicts
problem-changed-since = { $path } was saved again since it was recorded; nothing was replaced.
hint-changed-since = Your new save is suggested in turn; choose again once you are told.
problem-dest-exists = { $path } exists already; choose another name for the copy.
problem-needs-override = Nobody may make this change; an admin can still accept it, saying why.
hint-needs-override = Run: pigeon accept ID --override "the reason"
problem-needs-confirmation = This needs your confirmation, and pigeon may not ask here.
hint-needs-confirmation = Run it again in a terminal, or add --yes.
problem-backup-machine-needs-linux = A backup machine must run Linux with systemd.
problem-disk-full = { $machine } is full.
hint-disk-full = Free up space on it, or add another backup machine.
problem-no-capacity = { $machine } has no room for { $files } files: it needs { $missing_mb } MB more free space.
hint-no-capacity = Free up space on it, or add another backup machine.
problem-commit-unsigned = A change from { $machine } is not signed, so it was refused ({ $commit }).
problem-commit-forged = A change from { $machine } has a signature that does not match it, so it was refused ({ $commit }).
problem-commit-unknown-signer = A change from { $machine } was signed by a computer outside the group, so it was refused ({ $commit }).
problem-commit-not-allowed = A change from { $machine } changes what its person may not change, so it was refused ({ $commit }).
problem-commit-malformed = A change from { $machine } is not well formed, so it was refused ({ $commit }).
problem-commit-waits = A change from { $machine } waits for a newer pigeon ({ $commit }).
problem-repository-lost = The files of { $machine } were lost or replaced.
problem-daemon-not-running = pigeon is not running on this computer.
hint-daemon-not-running = Run pigeon open to start it, then check with: pigeon doctor
problem-version-mismatch = This pigeon command and the running pigeon are different versions.
hint-version-mismatch = Restart pigeon.
problem-not-installed = This pigeon was not installed from a release, so it cannot update itself.
hint-not-installed = Install a release with: curl -fsSL https://github.com/mariogeiger/pigeon/releases/latest/download/pigeon-install | sh
problem-git-annex-too-old = git-annex { $found } is too old; pigeon needs { $minimum } or newer.
problem-git-annex-untested = git-annex { $found } is newer than the versions pigeon was tested with.
problem-update-needed = The group uses a newer pigeon. Update pigeon on this computer.
problem-pigeon-too-old = This pigeon is too old for the group.
problem-peer-too-old = { $peer } runs a pigeon too old for the group.
problem-peer-too-new = { $peer } runs a newer pigeon that this one can't follow.
problem-settings-check = The group's settings on this computer were changed outside pigeon; freeing space waits until they are restored.
problem-receiving = A copy of this folder is still arriving.
problem-no-such-version = { $path } has no version { $version }.
problem-sheet-unreadable = This is not a recovery sheet, or it was mistyped.
hint-receiving = Try again in a moment.
problem-content-not-here = The content of { $path } is not on this computer.
hint-content-not-here = Keep its folder on this computer to read it here, or open a copy from its page.
problem-unknown-computer = The group has no computer named { $name }.
hint-unknown-computer = See the names with: pigeon machines
problem-usage = “{ $command }” was given the wrong arguments.
hint-usage = See: pigeon help
problem-not-a-pigeon-repository = This folder is not shared with pigeon.
problem-peer-unreachable = The other machine can't be reached right now.
problem-peer-refused = The other machine refused the connection.
problem-internal-error = Something went wrong inside pigeon.
hint-internal-error = Gather the details with: pigeon debug bundle

## The web interface (10 §10.3)
web-title = pigeon
web-nav-home = Home
web-nav-suggestions = Suggestions
web-nav-machines = People & machines
web-forbidden = Open pigeon from this computer with: pigeon open
web-login-used = This link was already used. Open pigeon again with: pigeon open
web-login = Open pigeon in your browser: { $url }
web-suggestion-decide = { $author } suggests a change of { $paths }, saved { $saved }.
web-suggestion-other = { $author } suggests a change of { $paths }, saved { $saved }; someone else decides.
web-suggestion-yours = Your change of { $paths }, saved { $saved }, waits for a decision.
web-suggestion-label = the change of { $paths } by { $author }
web-accept = Accept
web-override-reason = Why you accept it anyway (required)
web-managed-client = This is your client’s Pigeon peer on the bridge. Choose which files to keep offline in your Nextcloud app.
web-decline = Decline
web-decline-note = Why (optional)
web-suggestions-for-you = For you to decide
web-suggestions-yours = Your suggestions
web-suggestions-others = For someone else to decide
web-suggestions-decided = Your suggestions decided lately
web-suggestion-files = Files in this suggestion
web-suggestion-choose = Every file is selected. “Accept selected” accepts the selected files and declines the others; “Decline selected” declines the selected files and accepts the others.
web-review-file = Review { $path }
web-accept-selected = Accept selected
web-decline-selected = Decline selected
web-note-for = Note for { $author } about the files you decline (optional)
web-decide-all = { $count } suggestions wait for you. To decide them all at once, every file of each:
web-accept-all = Accept all
web-decline-all = Decline all
web-decided-all =
    { $count ->
        [one] You decided 1 suggestion.
       *[other] You decided { $count } suggestions.
    }
web-decide-all-skipped = { $author }'s suggestion changes something nobody may normally change: an admin can still accept it, saying why. Decide it on its own.
web-file-suggestion-waiting = Your change to this file, saved { $saved }, waits for a decision. Until then, the others keep the group's version.
web-back = Back
web-form-expired = This page is out of date. Reload it and try again.
web-nav-files = Files
web-setup = Join or create a group
web-setup-help = Ask someone in the group for an invitation, or create a group for your family or team. Choose an absolute folder path on this computer. A backup machine needs no shared folder.
web-join = Join the group
web-create = Create a group
web-invitation = Invitation
web-invitation-code = The line to share
web-command-line = Command-line instructions
web-local-folder = Folder on this computer
web-computer-name = This computer's name
web-group-name = Group name
web-backup-copies = Number of backup machines needed for safety
status-freshness = A safety check is recent for one hour. If no backup machine has been heard from for an hour, pigeon says it has not checked recently; that does not mean files have been lost. A machine silent for 24 hours stops counting toward safe copies. A new change may show Sending for up to ten minutes; it is safe only once enough backup machines hold it.
web-keep = Keep on this computer
web-free = Free up space
folders-none = The group has no folder yet.
folder-row =
    { $files ->
        [one] { $path }: { $state }, 1 file
       *[other] { $path }: { $state }, { $files } files
    }
folder-row-freed =
    { $files ->
        [one] { $path }: { $state }, 1 file, not on this computer
       *[other] { $path }: { $state }, { $files } files, not on this computer
    }
web-nav-settings = Settings
web-save = Save
web-setting-saved = { $key }: { $value }.
setting-row = { $key } = { $value }
setting-changed = { $key } is now { $value }.
setting-language = Language
setting-notifications = Notifications
setting-auto = Same as this computer
setting-true = On
setting-false = Off
language-en = English
language-fr = Français
cmd-settings = Show or change this computer's settings
problem-setting-unknown = “{ $key }” is not a setting pigeon knows.
hint-setting-unknown = See them with: pigeon settings
problem-setting-value = “{ $value }” is not a value “{ $key }” takes.
hint-setting-value = See the values with: pigeon settings
cmd-pause = Pause sharing for a while (such as 2h), or until resume
cmd-resume = Share again after a pause
headline-paused = Sharing is paused { $until }. Changes are saved on this computer only.
headline-paused-until-resume = Sharing is paused. Changes are saved on this computer only, until you resume.
paused-until = Sharing is paused { $until }.
paused-until-resume = Sharing is paused until you resume it.
resumed = Sharing again.
cmd-leave = Remove this computer from the group; its files stay
leaving = { $name } leaves the group once another machine has its removal. Its files stay in the folder.
leaving-already = This computer is already leaving the group.
left = { $name } has left the group “{ $group }”, where it was the only machine. Its files stay in the folder.
cmd-refound = Found the group again with you as its only admin, when no admin can sign
refounded = { $group } is refounded with { $name } as its only admin. Invite the others again: their folders join in place.
cmd-held = Changes pigeon holds until you choose: deletions to share or bring back, names to rename
held-none = Nothing is held.
held-row =
    { $files ->
        [one] { $id }: { $reason } (1 file, since { $since }): { $paths }
       *[other] { $id }: { $reason } ({ $files } files, since { $since }): { $paths }
    }
held-mass-deletion = { $files } files deleted at once
held-top-folder = the folder { $folder } removed
held-shared = { $files } deletions will reach the other computers.
held-brought-back = { $files } files come back.
held-name = the name { $path } does not work on every computer; suggested: { $suggested }
held-name-long = the path { $path } is too long for some computers; rename it
held-renamed = { $from } is now { $to }.
headline-held-names = { $files } files wait for a name that works on every computer.
headline-held = { $files } deleted files wait for you: delete them for everyone, or bring them back?
web-pause = Pause
web-resume = Share again
pause-hour = for an hour
pause-day = for a day
pause-forever = until I resume
problem-pause-duration = “{ $duration }” is not a duration pigeon understands.
hint-pause-duration = Write minutes, hours or days, such as 45m, 2h or 3d.
problem-restore-at = { $path } has no version saved at or before “{ $at }”.
hint-restore-at = Give a date such as 2026-03-03, or a time such as 2026-03-03T18:00:00+01:00, after its first version; pigeon history lists them.
problem-leave-last-admin = This computer is the group's last admin computer: it cannot leave.
hint-leave-last-admin = Make another person an admin, or add another computer of yours, first.
problem-held-unknown = No held batch has ID { $id }.
hint-held-unknown = See them with: pigeon held
problem-held-choice = Batch { $id } does not take that choice.
hint-held-choice = See its choices with: pigeon held
problem-held-rename-taken = { $to } already exists.
hint-held-rename-taken = Rename it yourself; pigeon captures it once its name works everywhere.
web-remove = Remove
web-remove-confirm = Remove { $name } from the group? Its files stay on it but are no longer shared with the group, and its copies stop counting. You can invite it again later.
web-remove-backup-confirm = Remove the backup machine { $name } from the group? The copies it keeps stop counting. You can invite it again later.
remove-backup-unsafe =
    { $left ->
        [0] After this, the group will have no backup machine. None of your files will be safe until you add one.
        [one] After this, the group will have only one backup machine, but it needs { $needed } for your files to be safe. Your files will not be safe until you add another one.
       *[other] After this, the group will have only { $left } backup machines, but it needs { $needed } for your files to be safe. Your files will not be safe until you add another one.
    }
web-invite = Invite
files-none = This folder has no file yet.
sync-none = The group has no folder yet.
sync-kept = { $folder }: on this computer
sync-not-kept = { $folder }: not on this computer
sync-chosen = (your choice)
sync-others = ({ $owner }’s folder, kept only if you choose: pigeon keep)
sync-default-result = { $folder } follows its default on this computer again.
file-row = { $path }: { $state }
web-restore = Restore this version
web-view = Download a copy
web-versions = Versions
web-version-current = current version
web-version-group-current = the group's current version
web-version-label = version { $number }, saved { $saved }
deleted-none = No deleted files to bring back.
deleted-row = { $path }, deleted { $deleted }
web-deleted = Deleted files
web-restore-deleted = Restore
notify-needs-you = pigeon needs you: open it to see what to do.
notify-not-safe-long = Some files have not been safe for more than a day.
notify-waiting = A suggestion or a conflict waits for you.
notify-open = Open pigeon
cmd-export = Write the files as plain files, without pigeond, network or key
export-done =
    { $files ->
        [one] 1 file written to { $dir }, as of { $commit }.
       *[other] { $files } files written to { $dir }, as of { $commit }.
    } Their list is MANIFEST.tsv.
export-deleted =
    { $files ->
        [one] 1 deleted file is in { $folder }.
       *[other] { $files } deleted files are in { $folder }.
    }
export-missing =
    { $files ->
        [one] 1 file is not in these repositories
       *[other] { $files } files are not in these repositories
    } ({ $damaged } damaged); they may be on other backup machines. Their list is { $list }.
problem-export-not-a-repository = { $path } is not a pigeon folder or a backup repository.
hint-export-not-a-repository = Give the folder, or the repository of a backup machine.
problem-export-no-version = No version “{ $at }” in these repositories.
hint-export-no-version = Give a commit, or a date such as 2026-10-01.
problem-export-not-empty = { $dir } is not empty.
hint-export-not-empty = Choose a new folder.
web-held = Changes waiting for you
web-held-share = Delete for everyone
web-held-bring-back = Bring them back
web-held-rename = Rename to { $suggested }
web-held-share-confirm =
    { $files ->
        [one] Delete this file for everyone ({ $paths })? It stays in the history, so it can still be brought back.
       *[other] Delete these { $files } files for everyone ({ $paths })? They stay in the history, so they can still be brought back.
    }
web-held-row = { $reason }, { $since }: { $paths }
web-leave = Leave the group
web-leave-confirm = Remove this computer from the group? Its files stay in the folder, but it stops sharing them.
cmd-free-without-checking = Remove this computer's copy even if it is the last one
drop-risk =
    { $files ->
        [one] 1 file
       *[other] { $files } files
    }: { $unsafe } not on enough backup machines, { $alone } on no other computer.
drop-preview = The content of these { $files } files would leave this computer, even where it is the only copy.
drop-done = { $files } files hidden, { $dropped } contents removed from this computer; a record of it is shared with the group.
confirm-drop = Remove this computer's copy anyway? What no other machine has is lost. [y/N]
cmd-view = Open a read-only copy of a version of a file, outside the folder
view-ready = A read-only copy is at { $path }; it is removed after seven days.
view-name-group = group's version
view-name-version = version { $number }
view-name-suggested = suggested version

setting-telemetry-device = Share diagnostics with
setting-off = Keep diagnostics on each computer
problem-join-history-unverified = Pigeon could not verify this group's history. Ask the person who invited you to update pigeon, then try again.
problem-export-history-unverified = Pigeon could not verify the archived history. Keep this repository and ask another member for a complete copy before exporting deleted files.

problem-recovery-not-allowed = This recovery sheet can no longer add computers. Use a current sheet, or ask an admin for an invitation.

machine-recovery = { $name }: paper recovery sheet printed on { $date }
machine-recovery-undated = { $name }: paper recovery sheet
web-revoke-sheet = Revoke recovery sheet

revoked-sheet = { $name } is revoked; this recovery sheet can no longer add computers.
web-revoke-confirm = Revoke { $name }? This recovery sheet can no longer add computers.

progress-restore = Restoring { $path }; waiting for this group’s checks and available copies.
progress-remove = Removing { $name }; waiting for this group’s checks.
progress-recovery-sheet = Preparing a recovery sheet and waiting for a backup machine to receive it.
progress-recover = Recovering this computer; checking the backup’s history and the new computer’s authority.

confirm-revoke-sheet = Revoke { $name }? This paper recovery sheet will no longer add computers. [y/N]

cmd-debug-allow = Allow one diagnostic bundle to a chosen computer for ten minutes
debug-allowed = { $name } may collect diagnostics for this group once, including file names. This permission expires in ten minutes. Restarting pigeon cancels this permission.
problem-debug-not-allowed = { $name } has not allowed a diagnostic part. Ask its person to open Settings and allow one bundle for this computer, then try again.
confirm-debug-bundle = Save diagnostics, including file names, in a private local file? Chosen peers share only with their own permission. [y/N]
confirm-debug-allow = Allow { $name } to collect one diagnostic part for this group, including file names, during the next ten minutes? [y/N]
progress-debug-bundle = Collecting diagnostics; chosen computers share only after their users allow it.
web-diagnostics = Share diagnostics once
web-diagnostics-consent = I agree to share this group’s diagnostics, including file names, once during the next ten minutes with the chosen computer.
web-diagnostic-peer = Computer receiving the diagnostic part
web-allow-diagnostics = Allow one diagnostic part

action-cancelled = Cancelled.

recovery-title = pigeon recovery sheet
recovery-group = Group: { $name } ({ $group })
recovery-person = Person: { $name }
recovery-sheet-name = This sheet: { $name }, as listed on the People & machines page
recovery-backups = Backup machines:
recovery-secret = Keep this sheet like a passport: whoever holds it can add computers for you. Print it and store it somewhere safe; do not send it to anyone.
recovery-instructions = On a new computer, save the code as a text file and run: pigeon recover THIS-FILE FOLDER
recovery-qr-too-large = This sheet has too much information for one QR code. Keep the complete text code on this sheet; it works in the same way.
recovery-recommendation = { $name } has only one signing computer. Print a recovery sheet or add a second computer so a lost computer does not lock you out.
web-print-recovery = Prepare a recovery sheet
web-recovery-print-help = Use your browser’s Print command to print this page. Keep the complete code and QR code together. Close this page when you have stored the sheet safely.

recovery-refresh-recommendation = The backup machines have changed since { $name }’s recovery sheets were prepared. Prepare a replacement sheet with the current backup machines.

problem-recovery-needs-backup = Add a backup machine before preparing a recovery sheet. A backup must receive the sheet’s permission before it can help you recover.
problem-recovery-backup-offline = No backup machine answered ({ $machines }). A backup machine must receive the sheet before it is printed. Turn one on and connect it, then prepare the sheet again. Nothing was changed.
problem-recovery-not-received = No backup machine received the sheet in time, so it was cancelled; there is nothing to print. Check that a backup machine is on, then prepare the sheet again.

problem-join-incomplete = The new group was verified, but recovery could not finish. Restart pigeon to finish the recorded operation. Your old history is kept.
problem-repository-busy = This repository has an unfinished file update. Let pigeon finish it, or restart pigeon before trying recovery again.
problem-folder-not-empty = This folder already has files in it. Choose an empty or new folder; to rejoin a refounded group, choose the folder of its existing repository.
# Group health from replicated state (ADR 0025).
web-health = Group health
web-health-current = Current files
web-health-retained = Other versions being kept
web-health-unavailable = Not checked yet; pigeon may be busy.
web-health-no-report = No report yet.
web-health-counts = Safe: { $protected } · Shared, not safe yet: { $published } · Only on this computer: { $local } · Not checked recently: { $uncertain } · Missing: { $missing } · Erased forever: { $erased }
headline-empty = No files yet. Add files to your shared folder to get started.
web-health-no-limit = not set
web-health-paper = A paper key keeps no files.
web-health-disk = Stored: { $used } · Free: { $free } · Limit: { $limit } · Reserved: { $reserve }
web-health-full = This disk needs more free space.
web-health-lost = This backup reports its repository lost.
web-health-checking = Checking stored files; these copies do not count as protected yet.
web-health-queue =
    { $deferred ->
        [one] Waiting to send: { $send } · To receive: { $receive } · Waiting for space: 1 item, { $missing } needed
       *[other] Waiting to send: { $send } · To receive: { $receive } · Waiting for space: { $deferred } items, { $missing } needed
    }
web-health-report = Storage and work report: { $when }.
web-health-report-old = Storage and work report is not recent: { $when }. New contact does not update these figures.
web-health-suggestions = Suggestions awaiting a decision: { $count }
web-health-repair = File contents marked for replacement copies: { $count }
web-health-updated = View updated: { $when }. Storage figures carry their own report times.
web-health-omitted = { $count } further file explanations are not listed here. Use Files to choose a folder.
web-health-machines = Computers, contact and storage
web-health-epochs = After computers were removed
web-health-no-epochs = No content-holding computer has been removed.
web-health-epoch = { $name } was removed: { $when }.

cmd-folder-rule = See folder sharing rules, or let an admin change who owns a folder or decides suggestions. Use --owner me for your own folder, and --per-person for a folder where each person owns the folder named after them.
folder-rule-owned = { $folder }: only { $owner } can change this folder.
folder-rule-add-only = { $folder }: everyone can add; other changes are decided by { $stewards }.
folder-rule-per-person = { $folder }: each folder in it belongs to the person it is named after ({ $folders }); anything else in it follows the rule above it.
folder-rule-result = The sharing rule for { $folder } is saved on this computer. Others use it after their next sharing check. View the folder's rule on another computer to confirm it has arrived.
problem-folder-rule-path = Choose a portable subfolder using its original spelling. The group root and pigeon's own files cannot receive a folder rule.
problem-folder-rule-person = No single current member matches “{ $name }”. Choose their name from People & machines, or use me for yourself.
problem-rules-raise = This sharing rule needs the group’s rules { $rules }. A computer whose pigeon does not know them waits for the group’s changes until it is updated. The others last reported: { $computers }.
hint-rules-raise = Update those computers first if you can, then confirm, or run it again with --yes.
confirm-rules-raise = Raise the group’s rules and save this sharing rule? [y/N]
web-folder-rule = Folder sharing rule
web-folder-rule-help = Choose who can change this folder. Everyone else can suggest changes. Subfolders inherit this rule unless they have their own.
web-folder-rule-admin = An admin can change this sharing rule.
web-folder-rule-owner = Only one person can change it
web-folder-rule-add-only = Everyone can add; selected people decide other changes
web-folder-rule-inherit = Inherit the rule of its parent folder
web-folder-rule-per-person = Each folder in it belongs to the person it is named after
web-folder-rule-confirm-rules = If this needs the group’s newer rules, raise them: a computer whose pigeon does not know them waits for changes until it is updated
web-folder-rule-stewards = People who decide changes
web-folder-rule-default-admins = Leave all unchecked to let the admins decide.

web-view-suggestion = Review suggested file
suggestion-deletes-file = This suggestion deletes { $path }. The shared file stays unchanged until the suggestion is accepted.

space-title = Backup storage
space-need =
    { $copies ->
        [one] Required storage for 1 backup copy: current files { $current }; history { $history }; suggestions { $suggestions }.
       *[other] Required storage for { $copies } backup copies: current files { $current }; history { $history }; suggestions { $suggestions }.
    }
space-offered = Configured capacity for this content: { $bytes }. Each backup computer counts for at most one copy, including computers with no recent storage report.
space-overflow = Extra copies: { $bytes }, from reported locations.
space-overflow-unknown = Extra copies have not been checked.
space-limit-low = A storage limit on { $machines } is smaller than one copy of the group’s files, so it cannot keep a copy of everything. Another backup machine, or a higher limit, makes up for it.
space-no-spare = There is no spare backup machine: if one stops working, your files are not safe until it is replaced. Adding another backup machine avoids this.
space-disk = Stored { $used }; free { $free }; storage limit { $limit }; reserve { $reserve }.
space-proposal = A suggestion saved { $when }, { $state }: { $bytes } more on each backup machine.
space-shared = Shared content counts once in the totals and can appear in several suggestion rows. Declined suggestions stay until you erase them.
space-open = awaiting a decision
space-adopted = accepted
space-rejected = declined
space-withdrawn = withdrawn
space-snapshot = { $group }, checked { $when }.

# Explicit, reference-aware erasure (08 §8.6).
erase-checking = Erasing is waiting for verified history and local recovery.
erase-prepare-needed = An admin must prepare erasing. Every current computer must acknowledge the update before any erasure can be confirmed.
erase-readiness-wait = Erasing is waiting for the current computers and backup machines to finish preparation.
erase-ready = Every current computer has acknowledged preparation. Review this scope before requesting erasure.
erase-waiting = Waiting for: { $names }.
erase-old-decisions = An earlier erasure must finish or be cancelled before the new backup can join the erasure quorum.
erase-preview-counts = This removes { $files } current files and requests erasing { $versions } observed versions, covering { $bytes } distinct bytes.
erase-warning = Erasing can permanently remove these versions. Each backup waits at least { $days } days after receiving the request, and every required backup must agree. Restoring a version before completion keeps it. This is not secure wiping of disks or copies outside Pigeon.
erase-shared = { $versions } selected versions share bytes retained elsewhere: { $paths }. Those bytes stay; the displayed size is not a promise of free disk space.
erase-needs-approval = This sends a suggestion to an admin. No erasure starts until the admin accepts it.
erase-preparing = Preparation requested. Nothing was erased or scheduled for erasure. Review and confirm again after every computer is ready.
erase-submitted = Erasure requested. Receipt: { $id }. Check Erasure requests for waiting backup machines and completion.
erase-submitted-suggestion = Erasure suggestion { $id } is awaiting an admin. The waiting period begins only after acceptance and receipt by each backup.
erase-none = No accepted erasure requests.
erase-decision = { $id }: { $paths }. Declared earliest completion: { $time }; each receiver’s own waiting period also applies.
erase-completed = Every original backup has sealed this scope. Uncovered or shared references still keep their bytes.
erase-pending = Waiting for the retention period and all required backup seals. This request covers the versions reviewed when it was made. Later saves and restores are kept; restoring does not cancel this request.
erase-membership-changed = The backup membership changed before completion. An admin must cancel this request and review a new one.
erase-cancel-warning = Cancel pending erasure { $id }? Cancelling cannot reconstruct bytes already removed. Current files are not restored automatically.
erase-cancelled = Erasure { $id } cancelled. Restore any current files you want back separately.
confirm-erase = Request this erasure? [y/N]
confirm-erase-prepare = Prepare erasing for this group without requesting an erasure? [y/N]
cmd-erase = Review permanent erasure of observed versions, or list its waiting state.
problem-erase-stale = New history or preparation acknowledgements arrived after your review. This attempt made no change. Open a fresh erasure preview, review its scope, then confirm again.
problem-erase-pending = Finish the interrupted local operation before requesting another erasure.
problem-erase-not-ready = Erasing is not ready. Check pigeon erase and the preview for the computers or backups still waiting.
problem-erase-checking = Erasing is waiting for verified history or local recovery. Check Doctor for details.
problem-erase-unknown = No unique pending erasure matches this ID. Run pigeon erase to see current requests.

web-erase-title = Erase observed versions
web-erase-list = Erasure requests
web-erase-confirm = Request this erasure
web-erase-suggest = Send erasure suggestion to an admin
web-erase-prepare = Prepare erasing only
web-erase-cancel = Cancel this pending erasure
web-erase-review-cancel = Review cancellation of
web-erase-suggestion = Review erasing this suggestion

# Integrity pass evidence and local progress (08 §8.8).
scrub-no-pass = No completed integrity pass has been reported.
scrub-last-pass = Last completed integrity pass: { $time }. Findings in that pass: { $failed }.
scrub-progress = Local integrity pass in progress: { $remaining } keys left. Findings so far: { $failed }.
scrub-unreadable = Integrity progress could not be read. Existing records are kept; check Doctor.
scrub-machine = Integrity checks on { $machine }:
problem-scrub-ledger = The integrity-check record on { $machine } could not be read. Keep it and gather a debug bundle before repairing local state.
scrub-report-time = Reported { $when }. This observation is not refreshed by new contact.
erase-needs-backup = Add a backup machine before preparing erasing.
erase-preparation-split = Computers agreed to different preparations, so erasing cannot start. An admin can create a separate group with pigeon refound, invite its computers again, then prepare erasing there. The old group’s requests remain in the old group.
problem-erase-needs-backup = Add a backup machine before preparing erasing.

web-health-suggestions-unavailable = Suggestions awaiting a decision: not checked yet.
web-health-separator = {": "}
web-not-found = There is no page here.
web-noscript = This page updates by itself only with JavaScript. Reload it to see the latest state.
web-skip = Skip to content
problem-folder-not-absolute = “{ $folder }” is not a full folder path. Enter one that starts with /, such as /home/you/Family.
problem-name-empty = Give the group and this computer a name.
problem-copies-zero = A group needs at least one backup copy.

ask-join-folder = Folder for “{ $group }” [{ $folder }]:
ask-join-person = Your name in the group [{ $person }]:
confirm-join-existing = { $person } is already in “{ $group }”. Add this computer to { $person }, with the same rights? [y/N]
problem-person-exists = { $person } is already in the group.
problem-already-in-group = This computer is already in this group.
problem-in-another-group = This computer is already in the group “{ $group }”, and a computer can be in only one group.
hint-in-another-group = To leave it, run pigeon leave, or choose Leave the group in Settings in pigeon open; its files stay in its folder. Then try again.
leaving-wait = It is leaving that group: try again once another machine of the group has its removal.
web-your-name = Your name in the group
web-join-existing = { $person } is already in “{ $group }”. Choose Join to add this computer to { $person }, with the same rights, or go back to give another name.
web-copy = Copy
web-invite-help = To add someone to “{ $group }”, another computer or a backup machine, share this line; it installs pigeon and joins the group. On a backup machine, add --backup-machine at its end. Whoever has it can join, with the rights of the person they say they are.
progress-join = Joining: waiting for the computer of an admin of the group to answer…
web-sections = Sections
web-status = Status:
web-breadcrumb = Location
web-size-b = { $size } B
web-size-kb = { $size } kB
web-size-mb = { $size } MB
web-size-gb = { $size } GB
web-size-tb = { $size } TB
web-decimal-separator = {"."}
web-folder-all-safe = All safe
web-folder-not-safe =
    { $count ->
        [one] 1 not safe yet
       *[other] { $count } not safe yet
    }
web-folder-files =
    { $files ->
        [one] 1 file
       *[other] { $files } files
    }
web-files-lead = Your shared folder on this computer: { $folder }
web-files-empty-help = Files you put in { $folder } appear here once pigeon has seen them.
web-folder-freed-note = This folder is not on this computer: its files are kept safe on the backup machines. Keep it on this computer to get it back, complete.
web-filter = Filter by name
web-filter-placeholder = Filter by name…
web-filter-none = No name here contains what you typed.
web-files-caption = Folders and files, with their state, size and last change
web-col-kind = Kind
web-col-name = Name
web-col-state = State
web-col-size = Size
web-col-modified = Changed
web-col-deleted = Deleted
web-col-actions = Actions
web-not-here = Not on this computer
web-kept-here = Kept on this computer
web-details = Details
web-details-of = Details of { $name }
web-details-close = Close the details of { $name }
web-versions-of = Versions of { $name }
web-version-number = Version { $number }
web-saved = Saved
web-current-version = current version
web-detail-files = Contains
web-detail-here = On this computer
web-detail-rule = Who can change it
web-open-folder = Open
web-all-versions = All versions
web-readme-too-long = { $name } is too long to show here ({ $size }).
web-readme-not-here = { $name } is not on this computer, so it is not shown here. Keep this folder on this computer to read it here.
web-readme-open = See its versions and open a copy
web-task-done = Done:
web-task-open = To do:
web-deleted-lead = Deleted files stay safe: restore one to bring back its last version, with its history.
web-suggestions-empty-help = When someone changes a file that you decide, their change waits here for you.
web-people = People
web-machines = Computers and backup machines
web-copied = Copied
web-settings-computer = This computer
web-settings-sharing = Sharing
web-leave-help = This computer stops sharing; its files stay in the folder.
web-rule-owned = Only { $owner }; others' changes become suggestions
web-rule-add-only = Everyone can add; other changes are decided by { $stewards }
web-rule-per-person = Each folder in it belongs to the person it is named after
web-shortcuts = Go to
web-here-others = { $owner }’s folder, not on this computer
web-here-chosen = Your choice
web-forget = Use the default
web-folder-others-note = This is { $owner }’s folder: it is not on this computer unless you keep it. Its files stay safe on the backup machines.
web-sync-title = Folders on this computer
web-sync-lead = Tick the folders to keep on this computer. A folder you tick or untick takes the folders in it along; you can still choose for each of them.
web-sync-help = The files of a folder that is not on this computer stay safe on the backup machines, and pigeon still shows them. Freeing a folder waits until each of its files is safe there; keeping it again downloads it, complete.
web-sync-partly = some folders in it are not on this computer
web-sync-inside =
    { $count ->
        [one] 1 folder in { $name }
       *[other] { $count } folders in { $name }
    }
web-sync-apply = Apply
web-sync-unchanged = Nothing to change: every folder already is as ticked.

headline-erased-restoration-missing =
    { $files ->
        [one] A restored file’s contents are missing. Try another saved version or bring back a computer with a copy.
       *[other] Restored contents are missing for { $files } files. Try another saved version or bring back a computer with a copy.
    }
why-erased-restoration-missing = An earlier erasure covered these contents. They are needed again, but no copy is currently known.
