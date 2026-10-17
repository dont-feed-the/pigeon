# Using pigeon

> Mission: everything a person needs to run pigeon, from installing it to
> its everyday commands.

pigeon keeps the files of a small group (a family, a lab, a team) on
everyone's computers, without a central server. Drop a file in the shared
folder and it reaches the others. Backup machines keep safe copies. pigeon
always tells you whether your files are safe, and why not when they aren't.

Under the hood pigeon is a layer over
[git-annex](https://git-annex.branchable.com/), which stores the versions,
counts the copies and refuses unsafe deletions. pigeon adds the parts people
see: who belongs to the group, which machines keep the safe copies, and a
status everybody can read.

> **Status: alpha.** It runs on Linux and macOS, from release
> binaries or built from source ([roadmap](roadmap.md)). Try it, but
> keep another copy of anything you care about.

To start a new group on a custodian server and connect Nextcloud apps, follow
the [first custodian and bridge quick start](nextcloud-quick-start.md).

## Review a suggestion before deciding

On **Suggestions**, each suggestion lists its files and what changes in
each: a new file, a change with its size before and after (and, for a
small text file, the lines added and removed), a deletion or a rename.
Choose **Review** beside a file: the browser downloads a read-only copy
of the proposed bytes. It does not replace the shared file or accept the
suggestion. The command line equivalent is
`pigeon view PATH --suggestion ID` (IDs come from `pigeon suggestions`).

Every file is ticked. **Accept selected** accepts the ticked files and
declines the others; **Decline selected** declines the ticked files and
accepts the others. A note tells the person why. With several
suggestions, **Accept all** and **Decline all** decide them at once. The
person who suggested sees, on their own Suggestions page and on each
file's page, which files were accepted, which declined, and your note.
On the command line, name the files after the ID:
`pigeon accept ID recipes.txt soup.txt` or
`pigeon decline ID groceries.txt --note "why"`.

Nested folder pages offer **Free up space** and **Keep on this computer**
for that folder. Free keeps protected copies on backup machines; Keep
requests local copies and can wait for a reachable source. Free says first
which files will leave this computer and the backup machines they stay on,
and asks a second time when no backup machine has been in touch for an hour;
a freed file then reads "Safe, not on this computer".

## Saves and history

pigeon waits for a file to stop changing before capturing a background version.
Several quick saves can become one history version containing the final save.
A file saved repeatedly over an hour is captured less often, while other files
continue. Keep an editor's undo history for intermediate changes; `pigeon history`
lists the versions pigeon has captured. Status still counts a newly saved file
as local until that version has safe backup copies.

## What "safe" means

A version of a file goes through three states, and pigeon always says which:

| State | Meaning |
|---|---|
| local | only on the computer where it was saved |
| published | the group knows about it, but too few safe copies exist yet |
| protected | at least K backup machines hold a verified copy |

Deleting can be undone until an explicit, signed purge. The full list of
guarantees (P1–P6) is in [spec/02-guarantees.md](spec/02-guarantees.md).

## Install

On Linux (x86_64) or macOS (Apple silicon), with git 2.38 or later and curl:

```sh
curl -fsSL https://github.com/dont-feed-the/pigeon/releases/latest/download/pigeon-install | sh
```

This fetches the latest release and checks it against the release's
`SHA256SUMS`. It puts pigeon in `~/.local/share/pigeon` (macOS:
`~/Library/Application Support/pigeon`) with the git-annex it runs, links
`pigeon` into `~/.local/bin`, and starts the daemon, `pigeond`, as a systemd
user service (macOS: a launchd agent), so it runs again after a reboot.

pigeon updates itself. Once a day, at a time that differs from one computer
to the next, it installs the latest release; and when a computer of the
group runs a newer release than this one, this one updates within minutes.
The update does not go through the installed pigeon: it downloads the
latest `pigeon-install` from GitHub, as the line above does, so a release
that broke pigeon is replaced by the next. It shows nothing; a failed
update is tried again the next day. Its log is in
`journalctl --user -u pigeon-update` (macOS:
`~/Library/Logs/pigeon/update.log`; a backup machine:
`journalctl -u pigeon-update`). To turn it off, or back on:

```sh
pigeon update --auto off
pigeon update --auto on
```

The choice is kept across updates. To install with it off, run the line
above as `curl … | sh -s -- --auto off`. A build of main is never replaced
by the automatic update. Otherwise, a release installed with `--version`
or `--rollback` lasts until the next automatic update.

To update now, or to go back to the release you had before:

```sh
pigeon update               # the latest release; or: pigeon update --version vX.Y.Z
pigeon update --rollback    # back to the previous release
pigeon update --main        # main's latest commit, built on this computer
```

They restart `pigeond`. `pigeon --version` tells which release runs; a
build of main adds its commit, as in `pigeon 0.3.2+g1a2b3c4d5e6f`.

`--main` needs git and Rust ([rustup](https://rustup.rs)). It keeps a clone
in pigeon's directory, under `src`, and builds there; the first build takes
a few minutes, later ones less. It takes pigeon's git-annex from the
latest release; only when main has changed it since does it build it,
which takes about an hour, and if that build fails it takes the official
git-annex instead. A plain `pigeon update` goes back to the
latest release.

On a Linux backup machine, run the line `pigeon invite` prints (see
[Use it with other people](#use-it-with-other-people)) as root, with
`--system` before `--join` and `--backup-machine` at its end. It installs
the system service, then joins:

```sh
curl -fsSL https://github.com/dont-feed-the/pigeon/releases/latest/download/pigeon-install | sudo sh -s -- --system --join pgn2-… --backup-machine
```

The service uses its own `pigeon` account. Group repositories and its key
live in `/var/lib/pigeon`; logs are in `/var/log/pigeon`. It updates itself
as a person's computer does, as root; `sudo /opt/pigeon/bin/pigeon update
--auto off` turns that off.

To uninstall a personal installation, run its `bin/pigeon-install
--uninstall` (Linux: `~/.local/share/pigeon/bin/pigeon-install --uninstall`;
macOS: `"$HOME/Library/Application Support/pigeon/bin/pigeon-install"
--uninstall`). On a backup machine use `sudo /opt/pigeon/bin/pigeon-install
--system --uninstall`. This stops the service and removes the programs;
shared folders, repositories, configuration and the device key stay.

## Open pigeon

Home separates **Current files** from **Other versions being kept**.
Editing or deleting a file does not release its older saved bytes. A
recently saved change may be only on this computer while its previous
version is already protected. Other versions also include suggestions
awaiting approval, which can be newer than the group's accepted files.
The headline includes both inventories and counts file versions.

Open **Machines, storage and work** for capacity and queued transfers.
Those figures carry the time of each machine's signed report: a new
connection does not make an old disk measurement current. A machine
checking restored storage does not count toward protection yet. Unknown
counts mean pigeon could not check the inventory; they do not mean zero
files. Accepted computer removals are listed separately with their times.

Restoring a version confirms its bytes on this computer. Check Home for
sharing and protection; a change requiring approval appears in Suggestions.

Run `pigeon open` to open the web interface. If the personal daemon is
not running, this starts the copy installed beside the command line.
The Home page says whether the files are safe. Files lists folders and
files at the top of the shared folder; each file links to its versions.
Settings lets you choose English or French. On a new computer, choose
*Join or create a group*, paste the invitation, give your name and choose
a local folder. A computer of an admin of the group must be on when you
join.

For a source build, run the `pigeon` and `pigeond` from the same `target/debug`
or `target/release` directory, with git-annex on `PATH`. When using the
patched git-annex with a stock standalone archive also on `PATH`, put the
system's git before that archive: the archive's git wrapper selects its
own git-annex.

## When a safety check gets old

Only *protected* means safe. A new change may show *Sending* for up to
ten minutes while it reaches the backup machines. With too few backup
machines configured, pigeon asks you to add one instead.

If no backup machine has been heard from for **one hour**, previously
safe files show *Not checked recently*. This means pigeon cannot confirm
their safety now; it does not mean they have been lost. A backup machine
silent for **24 hours** stops counting toward the required safe copies.
People & machines shows when each machine was last heard from. Keep
another copy while the status says anything other than safe. Settings
also explains these times.

pigeon has its own build of git-annex, with fixes for two bugs in
git-annex's P2P protocol ([ADR 0069](adr/0069-pigeon-runs-a-pinned-patched-git-annex.md)),
shipped with each release. Where it does not run (Linux before glibc 2.39),
the installer takes the official build, and pigeon works around both bugs.

## Use it with other people

Every computer in the group runs the daemon, `pigeond`, which the
installer starts. `pigeon doctor` checks that it runs.

**1. Create the group** on the first computer. `--k` is how many backup
machines must hold a copy before a version counts as protected:

```sh
pigeon create Family ~/Family --k 1 --name mario-laptop
```

Use a folder on a local disk: ext4, btrfs or xfs on Linux, APFS on macOS.
Network shares, FUSE, FAT/exFAT and HFS+ are refused before creating a
repository. On Linux, `df -T /path/to/folder` shows the file system under
*Type*; for a new folder, check its existing parent. On a Mac, Disk Utility
shows a volume’s *Format*. `pigeon doctor` checks folders already in a group.

It prints the line that adds someone to the group. `pigeon invite` prints
it again, on any computer of the group. The line is the same for everyone
and never changes:

```sh
curl -fsSL https://github.com/dont-feed-the/pigeon/releases/latest/download/pigeon-install | sh -s -- --join pgn2-…
```

**2. Invite people.** Send them the line. On their computer it installs
pigeon (or updates it), then asks which folder to use (`~/Family` by
default) and their name in the group (their account's by default). A
computer of an admin of the group must be on to let them in.

To add another computer of yours, run the line there and give your own
name, as `pigeon people` lists it: pigeon asks whether this computer is
yours, and adds it to you rather than to a new person.

**3. Add a backup machine.** Any computer that stays on will do: an old
laptop, a mini PC, a server. Run the line there with `--backup-machine` at
its end, and give the name of the person it belongs to. On a Linux server,
`loginctl enable-linger $USER` keeps `pigeond` running when nobody is
logged in.

Whoever has the line can join the group, under any name, with the rights
of the person of that name: share it only with the group. It does not
change when you remove someone from the group; only
[refounding the group](#recover-after-losing-every-laptop) gives a new one.

Files you put in `~/Family` now reach every computer of the group. The
computers find each other on the local network, and anywhere else through
[n0's public relays](https://www.iroh.computer/) of
[iroh](https://github.com/n0-computer/iroh), which only pass on encrypted
traffic. To stay on your own network, put `relays = "off"` under
`[network]` in pigeon's `config.toml`, which is in `~/.config/pigeon/`
on Linux and `~/Library/Application Support/pigeon/` on macOS.

Anyone in the group may add files. A change to a file someone else wrote,
or its deletion, waits as a suggestion until an admin accepts it: the
person who created the group.

```sh
pigeon suggestions          # list them
pigeon accept ID            # or: pigeon decline ID --note "why"
pigeon accept ID FILE…      # accept only these files, decline the others
```

An admin can choose a different rule for a folder. In Files, open the
folder and choose **Folder sharing rule**. Give it an owner, or choose add-only
and the people who decide suggestions. Anyone may add a new file in an
add-only folder; changes and deletions wait for those people. Leaving the
steward selection empty means the current admins.

The same controls are available from the command line:

```sh
pigeon folder-rule                         # list the group's rules
pigeon folder-rule ~/Family/Photos         # show the effective rule
pigeon folder-rule ~/Family/Photos --owner me
pigeon folder-rule ~/Family/Lab --add-only --steward Laurent
pigeon folder-rule ~/Family/Photos --default
```

Use a person's exact current name or their ID from `pigeon folder-rule --json`.
`--default` removes only this folder's direct rule: the nearest parent
rule applies again, or add-only with the admins if there is none. Deeper
rules stay in place. Use the folder's saved spelling, including case;
rules in this format match that spelling. Only admins can change rules.
Other computers use the new rule after their next sharing check. In the
usability sessions this sometimes took about a minute. Check the effective
rule on another computer before relying on the change there.

### Everyday commands

| Command | What it does |
|---|---|
| `pigeon status` | says whether your files are safe, and why not |
| `pigeon machines` | lists the group's computers and backup machines |
| `pigeon history FILE` | lists the versions of a file |
| `pigeon restore FILE --version N` | brings back an older version, or a deleted file |
| `pigeon conflicts` | lists your files that differ from the group's version |
| `pigeon resolve FILE --take-group` | takes the group's version (or `--keep-copy DEST`) |
| `pigeon free FOLDER` / `pigeon keep FOLDER` | frees a folder's space here, or keeps it here |
| `pigeon recovery-sheet` | prints the sheet that recovers the group on a new computer |
| `pigeon doctor` | checks the installation and explains what is wrong |
| `pigeon update` | installs the latest release (`--main`: main built here; `--rollback`: the one before; `--auto on\|off`: the automatic update) |

`pigeon --help` lists them all, and `--json` prints any answer as JSON.

### Tab completion

pigeon completes its commands and options, paths, and what the daemon
lists: suggestion IDs, held batches, the group's machines,
settings and their values. Add the line for your shell to its startup file:

```sh
source <(COMPLETE=zsh pigeon)     # ~/.zshrc
source <(COMPLETE=bash pigeon)    # ~/.bashrc
COMPLETE=fish pigeon | source     # ~/.config/fish/config.fish
```

Existing startup files that use `pigeon completions zsh` (or `bash`, `fish`,
`elvish`, `powershell`) work too; they print the same current script.

## What comes next

pigeon is built milestone by milestone ([roadmap](roadmap.md)):

- **v0.4:** installation and relays checked between two networks; the
  system service is implemented in the current source;
- **v0.5:** the web interface, French translation and usability work;
  the first two are available in the current source;
- **v0.6:** deleting for good (purge) and a monthly check of every copy
  (scrub);
- **v0.7:** large and busy files, and resource limits;
- **v1.0:** after weeks of daily use with no data lost.

## Recover after losing every laptop

Keep a recovery sheet somewhere separate from your computers. Anyone who
has it can add computers for its admin. Save the output of
`pigeon recovery-sheet` privately or print it; pigeon does not keep the key.
You can also open **People & machines → Prepare a recovery sheet**, then
print the page with your browser. It includes the complete text code and
a QR code carrying exactly that code. The instructions use your language.
When there are too many backup addresses for one QR code, keep the full
text instead. People & machines recommends a sheet or a second computer
when an admin has only one signing device, and a replacement after the
backup machines have changed.
The command waits until a backup has received the sheet's roster entry.

On a fresh computer, save the sheet's complete text in a private file, then
run `pigeon recover /path/to/sheet /path/to/new-empty-folder`. Keep the
surviving backup online. The new computer checks the group's signed history,
uses the sheet to sign its new device, and waits for the backup to fetch and
verify that addition. Files then arrive into the new folder. A backup owned
by another member works too; it cannot sign as you.

Run `pigeon machines` to find the lost computers and the used recovery
sheet. Remove each with `pigeon remove-computer NAME --yes`, including the
sheet: its key has now been typed into a computer. Print a replacement with
`pigeon recovery-sheet`, and keep it separately. Use `pigeon history PATH`
and `pigeon restore PATH --version NUMBER` for older versions.

For an accepted deleted folder, choose **Restore this folder** in Deleted
files, or run `pigeon restore /path/to/Family/deleted-folder`. This brings
back its latest saved state, including nested files. To choose an earlier
moment, use `pigeon restore FOLDER --at 2026-10-01` (end of that day, UTC),
or an RFC 3339 time. Newer files absent from that saved state stay in place,
and the more recent history remains readable. If a file is open or has
changed during the restore, pigeon leaves it alone and reports an
incomplete restore; close it or open a read-only old copy before retrying.
While saved contents are arriving or waiting for resources, you can still
check status and keep editing. A newer save is kept; retry the restore if you
still want the older version. An ordinary restore does not remove a file forever.

Free and Keep schedule local work. Files may remain present briefly after
Free, and missing files still need to arrive after Keep. The Files page
says when requested local copies are still arriving; an offline backup
must reconnect before they can be downloaded.

Both computers need a version supporting the sheet-signed recovery exchange.
An older recovery exchange is refused. Recovery metadata transfers have a
120-second deadline and a 64 MiB limit; file content arrives separately
through the normal, checked annex transport.

If every admin key and recovery sheet is lost, a surviving member can
start a new group with `pigeon refound /path/to/repository --yes`. Review
this choice carefully: that member becomes the new group's sole admin,
and owned folders belong to them. Existing history and files remain
available. The new group has its own line to share: the others join it with
that line, and the old one no longer works.

To reuse an existing folder, join with the new line and that folder's
path and confirm the refounding. A backup can reuse its bare repository
with `pigeon join --backup-machine INVITATION /path/to/old.git --yes`.
Pigeon verifies the new group before switching; existing annex objects
and identity are retained. If it reports an interrupted join, restart
the daemon so it can finish from its local journal. Do not delete the
repository or reinitialize annex. File history includes the verified
old group.

## Export files without running pigeon

Run `pigeon export --to /path/to/new-empty-folder --deleted /path/to/backup.git`
to copy current and deleted files from a backup, without a daemon, network
or secret key. After refounding it follows the signed archive links too.
Deleted files go under `.deleted/`; `MANIFEST.tsv` records exported files.
Every annex object is checked against its size and SHA-256.

Give several repositories to recover files spread across incomplete
backups. `MISSING.tsv` lists missing or damaged content and destination
conflicts; keep the original repositories until those entries are resolved.
Export does not follow an output symlink as a directory or overwrite an
already exported file. If an archive link cannot be checked, deleted-file
export explains the problem rather than silently claiming completeness.
Current files can still be exported without `--deleted`.

While a backup is running, `pigeon history /path/to/backup.git/folder/file`
and `pigeon view /path/to/backup.git/folder/file --version 1` read its
retained catalogue. These are virtual paths: the backup has no working
folder, and view makes a separate read-only copy.

## Gather diagnostics with another member

Diagnostics can contain file names and member activity. `pigeon debug bundle`
asks before saving them in a private local file; it sends no diagnostic
file to an external service.

To include another computer, ask its person to run
`pigeon debug allow YOUR-COMPUTER-NAME` on that computer. They confirm that
its file names may be included. They can instead open Settings, choose your
computer under Share diagnostics once, check the consent box and press
Allow one diagnostic part. Then run `pigeon debug bundle --from THEIR-NAME`
on your own computer and confirm the local collection.

Permission covers only the shared group, works once during the next ten
minutes and disappears when pigeon restarts. If collection fails after the
permission is consumed, ask for a new permission. A refusal exports nothing.
The local file includes the chosen parts; review it before sharing it with
someone else. `--yes` supplies explicit confirmation for scripted commands.
