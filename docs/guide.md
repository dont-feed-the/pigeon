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

> **Status: alpha (v0.3).** It runs on Linux and macOS, from release
> binaries or built from source ([roadmap](roadmap.md)). Try it, but
> keep another copy of anything you care about.

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

To update, or to go back to the release you had before:

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

On an Apple Silicon Mac, you can use Homebrew instead:

```sh
brew tap dont-feed-the/pigeon https://github.com/dont-feed-the/pigeon
brew install pigeon
brew services start pigeon
```

Homebrew installations update with `brew upgrade pigeon`.

On a Linux backup machine, download `pigeon-install` from the release,
then install the system service as root:

```sh
sudo sh pigeon-install --system
sudo /opt/pigeon/bin/pigeon join --backup-machine pgn1-…
```

The service uses its own `pigeon` account. Group repositories and its key
live in `/var/lib/pigeon`; logs are in `/var/log/pigeon`.

To uninstall a personal installation, run its `bin/pigeon-install
--uninstall` (Linux: `~/.local/share/pigeon/bin/pigeon-install --uninstall`;
macOS: `"$HOME/Library/Application Support/pigeon/bin/pigeon-install"
--uninstall`). On a backup machine use `sudo /opt/pigeon/bin/pigeon-install
--system --uninstall`. This stops the service and removes the programs;
shared folders, repositories, configuration and the device key stay.
Homebrew installations use `brew services stop pigeon` and
`brew uninstall pigeon`.

## Open pigeon

Run `pigeon open` to open the web interface. If the personal daemon is
not running, this starts the copy installed beside the command line.
The Home page says whether the files are safe. Files lists folders and
files at the top of the shared folder; each file links to its versions.
Settings lets you choose English or French. On a new computer, choose
*Join or create a group*, paste the invitation and choose a local folder.
The inviter must still be online when you join.

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

**2. Add a backup machine.** Any computer that stays on will do: an old
laptop, a mini PC, a server. Install pigeon there (on a Linux server,
`loginctl enable-linger $USER` keeps `pigeond` running when nobody is logged
in), then:

```sh
pigeon invite --backup-machine          # on your computer: prints a line to run
pigeon join --backup-machine pgn1-…     # on the backup machine
```

**3. Invite people.** Each invitation works once, for 24 hours:

```sh
pigeon invite Laurent                    # on your computer
pigeon join pgn1-… ~/Family --name laurent-laptop   # on Laurent's computer
```

To add another computer for yourself, invite your existing name (as
`pigeon people` lists it), then join from that computer. This adds a
computer to the same person rather than creating another person.

Files you put in `~/Family` now reach every computer of the group. The
computers find each other on the local network, and anywhere else through
[n0's public relays](https://www.iroh.computer/) of
[iroh](https://github.com/n0-computer/iroh), which only pass on encrypted
traffic. To stay on your own network, put `relays = "off"` under
`[network]` in pigeon's `config.toml`, which is in `~/.config/pigeon/`
on Linux and `~/Library/Application Support/pigeon/` on macOS.

Anyone in the group may add files. A change to a file someone else wrote,
or its deletion, waits as a suggestion until an admin accepts it: the
person who created the group, or anyone invited with `pigeon invite --admin`.

```sh
pigeon suggestions          # list them
pigeon accept ID            # or: pigeon decline ID --note "why"
```

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
| `pigeon update` | installs the latest release (`--main`: main built here; `--rollback`: the one before) |

`pigeon --help` lists them all, and `--json` prints any answer as JSON.

### Tab completion

pigeon completes its commands and options, paths, and what the daemon
lists: suggestion IDs, held batches, invitations, the group's machines,
settings and their values. Add the line for your shell to its startup file:

```sh
source <(COMPLETE=zsh pigeon)     # ~/.zshrc
source <(COMPLETE=bash pigeon)    # ~/.bashrc
COMPLETE=fish pigeon | source     # ~/.config/fish/config.fish
```

## What comes next

pigeon is built milestone by milestone ([roadmap](roadmap.md)):

- **v0.4:** installation and relays checked between two networks; the
  system service and Homebrew tap are implemented in the current source;
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

Both computers need a version supporting the sheet-signed recovery exchange.
An older recovery exchange is refused. Recovery metadata transfers have a
120-second deadline and a 64 MiB limit; file content arrives separately
through the normal, checked annex transport.
