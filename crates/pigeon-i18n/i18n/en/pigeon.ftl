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

## Headline levels (ux.md §4)

level-needs-you = Needs you
level-not-safe = Not safe yet
level-working = Working
level-safe = Safe
level-not-checked = Not checked recently

## Headlines: what, then why and what next (ux.md §4)

headline-safe = Everything is safe. Checked { $ago }.
headline-not-safe =
    { $files ->
        [one] 1 file is not safe yet.
       *[other] { $files } files are not safe yet.
    }
headline-working =
    { $files ->
        [one] Sending 1 file to the backup machines…
       *[other] Sending { $files } files to the backup machines…
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
why-machine-offline = { $machine } has been offline since { $since }.
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

## Commands of the safety slice (13 §13.12)

cmd-create = Create a group with this computer as its first member
cmd-invite-person = Print a one-time invitation for a person's computer, valid 24 hours
cmd-invite = Print a one-time invitation, valid 24 hours
cmd-join = Join a group with an invitation
cmd-status = Say whether your files are safe, and why not
cmd-machines = List the group's computers and backup machines
cmd-open = Open pigeon in your browser
cmd-history = List the versions of a file
cmd-restore = Restore a version, or a deleted file
cmd-free = Free up a folder's space on this computer
cmd-keep = Keep a folder on this computer
cmd-suggestions = List the suggested changes waiting for a decision
cmd-accept = Accept a suggested change into the group's files
cmd-decline = Decline a suggested change, saying why if you like
cmd-conflicts = List your files that differ from the group's version
cmd-resolve = Choose for a file that differs: the group's version, yours as a copy, or suggest it again
cmd-remove-computer = Remove a lost or retired computer from the group
cmd-recovery-sheet = Print the recovery sheet for this group
cmd-recover = Recover the group on a new computer from its backup machines
cmd-doctor = Check the installation and explain what is wrong
cmd-debug-bundle = Gather diagnostics for someone helping you
cmd-update = Install the latest release of pigeon, a given one, or the one before
opt-json = Print the result as JSON, for scripts
opt-backup-machine = For a backup machine rather than a person's computer
opt-not-shared = List what is not shared, and why
help-usage = Usage: pigeon COMMAND [ARGUMENTS]
help-commands = Commands:

## Results

free-result = { $safe } of { $total } files are safe and their space is freed.
free-waiting =
    { $files ->
        [one] 1 file is not safe yet; pigeon will free it once it is.
       *[other] { $files } files are not safe yet; pigeon will free them once they are.
    }
keep-result = { $folder } is kept on this computer.
suggestions-none = No suggestion is waiting.
suggestion-decide = { $id }  { $author } suggests a change of { $paths }, saved { $saved }. Accept it with: pigeon accept { $id }
suggestion-other = { $id }  { $author } suggests a change of { $paths }, saved { $saved }; someone else decides.
suggestion-yours = { $id }  Your change of { $paths }, saved { $saved }, waits for a decision.
accepted-suggestion = { $author }'s suggestion is now in the group's files.
declined-suggestion = { $author }'s suggestion is declined; they will be told.
conflicts-none = Every file here is the group's version.
conflict-waiting = { $path }: your suggestion waits for a decision.
conflict-choose = { $path }: your version differs from the group's. Choose with: pigeon resolve
resolved-take-group = { $path } is the group's version again; yours stays in its history.
resolved-keep-copy = Your version of { $path } is kept as { $dest }; { $path } is the group's version again.
resolved-suggest-again = Your version of { $path } is suggested again.
undo-free = Undo: pigeon keep { $folder }
undo-keep = Undo: pigeon free { $folder }
restore-result = { $path } is restored; the newer version stays in its history.
recovery-sheet-text = { $sheet }
recovered = This computer is back in { $group }. Its files arrive from the backup machines; then remove the lost computers and print a new recovery sheet.
not-shared-kind = { $path }: not shared ({ $kind })
doctor-lab-build = This is a lab build: its status reports are marked, and no release counts them.
doctor-version = { $program } { $found } (at least { $minimum } is needed)
doctor-ok = Everything pigeon needs is in place.
created = The group “{ $group }” is created. Add a backup machine with: pigeon invite --backup-machine
invitation =
    { $who ->
        [backup-machine] On the backup machine, within 24 hours, run: pigeon join --backup-machine { $code }
       *[computer] On the new computer, within 24 hours, run: pigeon join { $code } FOLDER
    }
joined =
    { $who ->
        [backup-machine] This backup machine now keeps copies for “{ $group }”.
       *[computer] This computer is now in “{ $group }”. The group's files arrive in its folder.
    }
removed-computer = { $name } is removed from the group; its copies no longer count.
bundle-written = The diagnostics are in { $path }. Nothing was sent.
machine-row = { $name }: { $kind }, last heard { $heard }
machine-kind-custodian = backup machine
machine-kind-computer = computer
machine-this = { $name }: this computer
version-row = { $number }. { $saved }: { $state }
waiting-row = { $path }: { $state }
confirm-remove-computer = Remove { $name } from the group? Its copies will stop counting. [y/N]
confirm-yes = yes

## Problems (10 §10.6, ADR 0023): problem-<code>, and hint-<code> when there is one

problem-code = Code: { $code }
problem-not-in-this-build = “{ $command }” is not in this build of pigeon yet.
hint-not-in-this-build = This early build has only the commands that keep files safe; the others come later.
problem-file-system-not-in-this-build = This folder is on { $fs }, which this build of pigeon does not support yet.
hint-file-system-not-in-this-build = Choose a folder on ext4 (Linux) or APFS (macOS).
problem-invite-expired = This invitation has expired (it was valid until { $until }).
hint-invite-expired = Ask { $inviter } for a new one.
problem-invite-used = This invitation has already been used.
hint-invite-used = Ask { $inviter } for a new one.
problem-invite-unknown = This invitation is not one the group knows.
problem-invite-not-allowed = { $inviter } may no longer invite to this group.
problem-inviter-offline = { $inviter }'s computer can't be reached right now.
hint-inviter-offline = Try again when { $inviter }'s computer is on.
problem-not-in-group = This computer is not a member of the group.
problem-not-allowed = You are not allowed to do this in this group.
problem-suggestion-unknown = No waiting suggestion is called { $id }.
hint-suggestion-unknown = See them with: pigeon suggestions
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
hint-daemon-not-running = Start it, then check with: pigeon doctor
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
web-accept = Accept
web-decline = Decline
web-decline-note = Why (optional)
web-back = Back
web-form-expired = This page is out of date. Reload it and try again.
web-nav-files = Files
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
        [one] { $path }: { $state }, 1 file; its space is freed on this computer
       *[other] { $path }: { $state }, { $files } files; their space is freed on this computer
    }
web-nav-settings = Settings
web-save = Save
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
web-remove = Remove
web-remove-confirm = Remove { $name } from the group? Its copies will stop counting.
web-invite = Invite
web-invite-person = The person's name
web-invite-admin = as an admin
web-invite-backup = Invite a backup machine
files-none = This folder has no file yet.
file-row = { $path }: { $state }
web-restore = Restore this version
web-versions = Versions
deleted-none = No file was deleted.
deleted-row = { $path }, deleted { $deleted }
web-deleted = Deleted files
web-restore-deleted = Restore
