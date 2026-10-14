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
headline-machine-missing =
    { $days ->
        [one] { $machine } has been missing for 1 day.
       *[other] { $machine } has been missing for { $days } days.
    } Replace it or add another backup machine.
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
cmd-invite = Print a one-time invitation, valid 24 hours
cmd-join = Join a group with an invitation
cmd-status = Say whether your files are safe, and why not
cmd-machines = List the group's computers and backup machines
cmd-history = List the versions of a file
cmd-restore = Restore a version, or a deleted file
cmd-free = Free up a folder's space on this computer
cmd-keep = Keep a folder on this computer
cmd-remove-computer = Remove a lost or retired computer from the group
cmd-recovery-sheet = Print the recovery sheet for this group
cmd-recover = Recover the group on a new computer from its backup machines
cmd-doctor = Check the installation and explain what is wrong
cmd-debug-bundle = Gather diagnostics for someone helping you
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
undo-free = Undo: pigeon keep { $folder }
undo-keep = Undo: pigeon free { $folder }
restore-result = { $path } is restored; the newer version stays in its history.
not-shared-kind = { $path }: not shared ({ $kind })
doctor-lab-build = This is a lab build: its status reports are marked, and no release counts them.
doctor-version = { $program } { $found } (at least { $minimum } is needed)
doctor-ok = Everything pigeon needs is in place.
created = The group “{ $group }” is created. Add a backup machine with: pigeon invite --backup-machine
invitation = On the backup machine, within 24 hours, run: pigeon join --backup-machine { $code }
joined = This backup machine now keeps copies for “{ $group }”.
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
problem-needs-confirmation = This needs your confirmation, and pigeon may not ask here.
hint-needs-confirmation = Run it again in a terminal, or add --yes.
problem-backup-machine-needs-linux = A backup machine must run Linux with systemd.
problem-disk-full = { $machine } is full.
hint-disk-full = Free up space on it, or add another backup machine.
problem-same-domain = { $machine } shares a disk or a machine with { $other }, so they count as one safe copy.
problem-independence-unknown = pigeon can't tell whether { $machine } is independent of the other backup machines.
hint-independence-unknown = Declare where it is, so that pigeon can count it.
problem-repository-lost = The files of { $machine } were lost or replaced.
problem-daemon-not-running = pigeon is not running on this computer.
hint-daemon-not-running = Start it, then check with: pigeon doctor
problem-version-mismatch = This pigeon command and the running pigeon are different versions.
hint-version-mismatch = Restart pigeon.
problem-git-annex-too-old = git-annex { $found } is too old; pigeon needs { $minimum } or newer.
problem-git-annex-untested = git-annex { $found } is newer than the versions pigeon was tested with.
problem-update-needed = The group uses a newer pigeon. Update pigeon on this computer.
problem-pigeon-too-old = This pigeon is too old for the group.
problem-peer-too-old = { $peer } runs a pigeon too old for the group.
problem-peer-too-new = { $peer } runs a newer pigeon that this one can't follow.
problem-settings-check = The group's settings on this computer were changed outside pigeon; freeing space waits until they are restored.
problem-receiving = A copy of this folder is still arriving.
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
