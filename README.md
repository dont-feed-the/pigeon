# pigeon

pigeon keeps the files of a small group (a family, a lab, a team) on
everyone's computers, without a central server. Drop a file in the shared
folder and it reaches the others. Backup machines keep safe copies. pigeon
always tells you whether your files are safe, and why not when they aren't.

Under the hood pigeon is a layer over
[git-annex](https://git-annex.branchable.com/), which stores the versions,
counts the copies and refuses unsafe deletions. pigeon adds the parts people
see: who belongs to the group, which machines keep the safe copies, and a
status everybody can read.

> **Status: alpha (v0.3).** It runs on Linux and macOS, built from source.
> Packages come with v0.4 ([roadmap](docs/roadmap.md)). Try it, but keep
> another copy of anything you care about.

## What "safe" means

A version of a file goes through three states, and pigeon always says which:

| State | Meaning |
|---|---|
| local | only on the computer where it was saved |
| published | the group knows about it, but too few safe copies exist yet |
| protected | at least K backup machines hold a verified copy |

Deleting can be undone until an explicit, signed purge. The full list of
guarantees (P1–P6) is in [docs/spec/02-guarantees.md](docs/spec/02-guarantees.md).

## Install

You need:

- Linux (x86_64) or macOS (Apple silicon);
- git 2.38 or later, and curl;
- Rust, through [rustup](https://rustup.rs). The repository pins its
  toolchain, and rustup fetches it on the first build.

Build pigeon:

```sh
git clone https://github.com/mariogeiger/pigeon.git
cd pigeon
cargo build --release -p pigeond -p pigeon -p git-annex-p2p-iroh
```

Install git-annex where pigeon looks for it. On Linux this downloads the
official standalone build and checks its SHA-256; on macOS it runs
`brew install git-annex`:

```sh
packaging/install-git-annex --stock
```

Copy the three programs next to it, and put them on your `PATH`:

```sh
# Linux
P="$HOME/.local/share/pigeon"
# macOS
P="$HOME/Library/Application Support/pigeon"

mkdir -p "$P/bin"
cp target/release/pigeond target/release/pigeon target/release/git-annex-p2p-iroh "$P/bin/"
export PATH="$P/bin:$PATH"     # and add this line to your shell's profile
```

pigeon also has its own build of git-annex, with fixes for two bugs in
git-annex's P2P protocol ([ADR 0069](docs/adr/0069-pigeon-runs-a-pinned-patched-git-annex.md)).
It is not published yet, so `--stock` installs the official build;
pigeon works around both bugs.

## Try it on one computer

`scripts/try-pigeon` starts three pigeon daemons side by side in
`/tmp/pigeon-try`: Alice's laptop, Bob's laptop and a backup machine. It
puts them in one group, then prints what to try:

```sh
scripts/try-pigeon
echo hello > /tmp/pigeon-try/Alice/hello.txt
/tmp/pigeon-try/bin/alice status      # "Sending…", then "Everything is safe"
ls /tmp/pigeon-try/Bob                # hello.txt arrives in a few seconds
scripts/try-pigeon stop               # stops them and removes /tmp/pigeon-try
```

The three daemons talk over 127.0.0.1 only. The script works from the
build directory, and needs only `git-annex` on the `PATH` or installed as
above.

## Use it with other people

Every computer in the group runs the daemon, `pigeond`. Start it and leave
it running:

```sh
pigeond &
```

Packages with a systemd or launchd service come in v0.4; until then, start
it again after a reboot.

**1. Create the group** on the first computer. `--k` is how many backup
machines must hold a copy before a version counts as protected:

```sh
pigeon create Family ~/Family --k 1 --name mario-laptop
```

**2. Add a backup machine.** Any computer that stays on will do: an old
laptop, a mini PC, a server. Install pigeon there, start `pigeond`, then:

```sh
pigeon invite --backup-machine          # on your computer: prints a line to run
pigeon join --backup-machine pgn1-…     # on the backup machine
```

**3. Invite people.** Each invitation works once, for 24 hours:

```sh
pigeon invite Laurent                    # on your computer
pigeon join pgn1-… ~/Family --name laurent-laptop   # on Laurent's computer
```

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

`pigeon --help` lists them all, and `--json` prints any answer as JSON.

## What comes next

pigeon is built milestone by milestone ([roadmap](docs/roadmap.md)):

- **v0.4:** packages for Linux and macOS, and relays checked between two
  networks;
- **v0.5:** a web page with plain words instead of the command line, and a
  French translation;
- **v0.6:** deleting for good (purge) and a monthly check of every copy
  (scrub);
- **v0.7:** large and busy files, and resource limits;
- **v1.0:** after weeks of daily use with no data lost.

## For developers

| Path | What it holds |
|---|---|
| [`crates/`](crates/) | the daemon `pigeond`, the command line `pigeon`, the core algebras and the lab |
| [`docs/spec/`](docs/spec/README.md) | the specification, one subject per file, built from the ADRs |
| [`docs/adr/`](docs/adr/) | architecture decision records, one per choice, with an index |
| [`docs/roadmap.md`](docs/roadmap.md) | the milestones to v1.0 and when each counts as done |
| [`docs/reports/`](docs/reports/) | what each milestone delivered, and its known gaps |
| [`models/`](models/) | TLA+ models, checked by TLC, and the catalogue merge's oracle |
| [`third_party/git-annex/`](third_party/git-annex/) | the pinned git-annex and pigeon's patches |
| [`scripts/`](scripts/) | the checks that CI runs |

```sh
cargo test --workspace                  # unit, property (hegel) and integration tests
scripts/check-file-length-and-header   # no file reaches 1000 lines; every file starts with a header
scripts/check-adr-index                # docs/adr/README.md lists every ADR
scripts/run-tlc                        # the fast set of TLA+ models (Java 11 or later)
```

The integration tests of `pigeond` need git-annex on the `PATH`. pigeon 0.9,
an earlier design, is kept under the tag `v0.9.0`.

## Licence

MIT OR Apache-2.0, at your option ([ADR 0019](docs/adr/0019-licence.md)).
