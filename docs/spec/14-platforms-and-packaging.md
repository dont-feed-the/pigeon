# 14. Platforms and packaging

> Mission: platforms, file systems, git and git-annex, supervision and
> packaging, and the Rust workspace: crates, dependencies, build, licence.

## 14.1 Platforms (ADRs 0005, 0032)

| Role | v1 | Later |
|---|---|---|
| a person's computer (laptop) | Linux x86-64 and arm64; macOS on Apple Silicon and Intel | native Windows in v1.1 (E11); phones as read-only viewers, not designed (ux.md §11) |
| backup machine (custodian) | Linux x86-64 and arm64 with systemd | macOS; NAS vendor packages (Synology, QNAP) |

- WSL is not Windows support. Unix-only code stays behind `pigeon-sys`
  (§14.6) and no protocol or format assumes it, so Windows can join in
  v1.1; names are portable from day one ([05 §5.4](05-catalogue-and-names.md)).
- Minimum OS versions are those of the CI runners (Ubuntu LTS, the oldest
  macOS runner) until a test says otherwise **[no ADR]**.

## 14.2 File systems

A laptop's folder must be on a local file system with inodes stable across
renames, hard links, and an atomic exclusive rename (`RENAME_NOREPLACE`,
`RENAME_EXCL`), which install needs ([07 §7.5](07-capture-and-install.md)):

| Supported | Refused, with a Problem at `create` or `join` and in `pigeon doctor` |
|---|---|
| ext4, btrfs, xfs (Linux); APFS (macOS) | FAT, exFAT, NTFS through FUSE, network file systems (NFS, SMB), FUSE in general, HFS+ |

**[no ADR: list]**. Case-insensitive APFS is supported (names are folded at
capture). A backup machine may use any local Linux file system git-annex
supports (ext4, xfs, btrfs, ZFS): its repository is bare.

## 14.3 git and git-annex (ADRs 0004, 0010, 0069)

- **pigeon's git-annex** (ADR 0069): 10.20261006 with the patches of
  `third_party/git-annex/`, reporting `10.20261006-pigeon1`, built by CI
  for Linux x86-64 (on Ubuntu 24.04, so glibc 2.39 or later) and macOS on
  Apple Silicon, with the names it dispatches on (`git-remote-p2p-annex`
  among them). The installer, `packaging/install-git-annex`, puts it in
  `git-annex/bin` of pigeon's directory, the one that holds pigeon's `bin/`
  (`~/.local/share/pigeon`, `/opt/pigeon` on a backup machine,
  `~/Library/Application Support/pigeon` on macOS until the formula of
  §14.5), after checking the archive's sha256 and the version it reports.
  `pigeond` finds it beside its own directory and puts it first in its
  children's `PATH`, after the directory of its links; `git-annex.yml`
  checks this on both runners with the stock build first in the `PATH`
  `pigeond` inherits (`packaging/check-install`). On Linux it runs with the system's git,
  since the standalone build's `git` would put its own git-annex first;
  a distribution with glibc 2.39 ships git 2.38 or later (ADR 0010).
  Linux arm64, macOS on Intel and older C libraries run the stock builds
  below until CI builds them **[no ADR]**.
- **Linux, stock:** the installer (`--stock`, or where pigeon's build does
  not run) fetches git-annex's standalone build at its pinned sha256 into
  `~/.local/share/pigeon/git-annex.linux` (`/opt/pigeon/git-annex.linux` on
  a backup machine) and uses the git it bundles (≥ 2.38, ADR 0010), first
  in the `PATH` of `pigeond`'s children; the person's git is not touched.
- **macOS, stock:** where pigeon's git-annex does not run, `brew install
  git-annex`. Homebrew installs no `git-remote-p2p-annex` (E12), so
  `pigeond` links that name to its `git-annex` in its own directory, first
  in its children's `PATH`.
- **Minimum versions:** the oldest CI tests, **10.20261006** (the pinned
  standalone build) on Linux and **10.20260901** (Homebrew, when E20 ran) on
  macOS. Below them `pigeond` refuses to run (`git-annex-too-old`,
  [10 §10.6](10-interfaces.md)).
- **Untested versions** (ADR 0048): the first time `pigeond` meets a git or
  git-annex newer than CI tested, it runs the contract self-test
  ([13 §13.6](13-verification.md)) on a scratch repository, under 30 s at
  idle priority. If a check fails it keeps capturing and serving, suspends
  every drop, and shows *Needs you* (`git-annex-untested`).
- **Upgrades are deliberate:** CI records the version it tested and the
  installer verifies what it fetched. A missing git-annex feature or a
  fix goes upstream as a patch (ADR 0004), carried in
  `third_party/git-annex/` until a release has it (ADR 0069).
- `ssh-keygen` is required for signing (ADR 0007).

## 14.4 Supervision (ADR 0011)

| Machine | Supervisor |
|---|---|
| a person's Linux computer | a **systemd user unit**: `ExecStart=%h/.local/share/pigeon/bin/pigeond`, `Restart=on-failure`, `RestartSec=5`, `Nice=10`, `IOSchedulingClass=idle`, `CPUWeight=20`, `IOWeight=20`, `MemoryHigh=512M`, `WantedBy=default.target` (ADR 0037, [15 §15.1](15-resource-use.md)) |
| a backup machine | a **system unit** `pigeond.service`, user `pigeon`, normal priority, scrubs in the idle I/O class (ADRs 0032, 0037) |
| macOS | a **launchd agent** `~/Library/LaunchAgents/org.pigeon.pigeond.plist` (`KeepAlive`, `RunAtLoad`, `ProcessType = Background`) |

The supervisor restarts `pigeond` after a panic (`panic = "abort"`, ADR
0023); on exit `pigeond` stops its git-annex children and closes its
endpoint. `MemoryHigh` is a default for the budgets of
[15 §15.7](15-resource-use.md) to check **[no ADR: value]**. On a person's
computer `packaging/pigeon-install` writes the unit or the agent and
(re)starts it (§14.5). `--system` installs the system unit through
`packaging/pigeon-service`, creates the unprivileged `pigeon` account and
its state directories, and runs `pigeond --service`. Its runtime directory
is created by systemd; uninstall keeps its account and state.

## 14.5 Packaging and installation

| Platform | Package | Holds |
|---|---|---|
| Linux, people | a tarball and an install script | `pigeon`, `pigeond`, `git-annex-p2p-iroh`, the installers; the script installs pigeon's git-annex, or fetches git-annex's standalone build (§14.3), and enables the user unit; later the `.desktop` launcher and icon, man pages, shell completions |
| Linux, backup machines | the tarball and a system unit (ADR 0032) | the same binaries, installed as root with `pigeon-install --system` into `/opt/pigeon`; a dedicated `pigeon` account runs the daemon |
| macOS | a tarball and the install script, or the Homebrew formula in `Formula/pigeon.rb` **[no ADR]** | the binaries, the launchd agent, pigeon's git-annex (§14.3); the formula adds the small launcher app bundle (ADR 0029) |

Release archives come with SHA-256 checksums, built by CI from a tag
**[no ADR: signing]**: `release.yml` runs on a tag `v` + the workspace's
version, builds the programs for Linux x86-64 (on Ubuntu 22.04, glibc 2.35
or later) and macOS on Apple Silicon, calls `git-annex.yml` for pigeon's
git-annex, installs the Linux archive as a person would, then publishes
`pigeon-<tag>-<os>-<arch>.tar.gz`, `git-annex-pigeon-<os>-<arch>.tar.gz`,
`SHA256SUMS` and `pigeon-install` as the latest release. People install
with `curl -fsSL …/releases/latest/download/pigeon-install | sh`.
`pigeon-install` fetches the release, checks it, and hands over to the
release's own copy of itself, which places it: the programs in `bin/`, the
ones they replace in `bin.previous/`, a `pigeon` link in `~/.local/bin`,
git-annex by `install-git-annex`, and the unit or agent of §14.4.
`pigeon update [--version TAG | --rollback]` runs the `pigeon-install`
beside it; a release already installed is not fetched again, and
`--rollback` swaps `bin/` and `bin.previous/`. Paul installs (ux.md §11). Uninstalling keeps the
folder, the repository and the configuration unless the person asks.

## 14.6 The Rust workspace

| Crate | Kind | Holds | May do I/O |
|---|---|---|---|
| `pigeon-core` | lib | the pure core: roster, authority rule, catalogue merge, names, states, headline, queue order, journal replay, filter state machine, trace event enums (ADR 0012) | no |
| `pigeon-proto` | lib | the request and response types of the API and the socket framing (ADR 0021) | no |
| `pigeon-i18n` | lib | Fluent bundles, language choice, the completeness test (ADR 0028, 10 §10.5) | reads embedded files only |
| `pigeon-git` | lib | drivers for git and git-annex: batch processes, JSON parsing, contract tests (ADRs 0004, 0010) | yes |
| `pigeon-sys` | lib | the platform layer: renames, signals, `fadvise`, pressure, battery and metered probes, sockets (ADR 0037) | yes |
| `pigeon-telemetry` | lib | the only crate depending on `opentelemetry*` (ADR 0025) | yes |
| `pigeon-web` | lib | axum routes and maud pages (ADR 0021) | through `pigeon-proto` |
| `pigeond` | bin | the daemon: loops, iroh endpoint, socket, web server | yes |
| `pigeon` | bin | the CLI (ADR 0024) | socket only; `update` runs the installer beside it (§14.5) |
| `git-annex-p2p-iroh` | bin | the transport helper (ADR 0006) | socket and stdio only |
| `pigeon-lab` | lib and bin, never shipped | the slice's scenarios as typed values, their model interpreter over `pigeon-core` and their process executor ([13 §13.12](13-verification.md)) | yes: it runs `pigeond` and `pigeon` and mounts disk images |

Each crate lives in `crates/<name>/`, created when its work starts
(`pigeon-web`, `pigeon-telemetry` not yet). `pigeon-core` does no I/O:
`scripts/check-core-deps` checks `cargo tree` against
`crates/pigeon-core/allowed-deps.txt` in CI.

## 14.7 Dependencies

Pinned in `Cargo.toml` to one minor version. The crates for logs, telemetry,
the CLI, the web, errors, messages, times and notifications are those of
ADRs 0021–0025, 0028 and 0029. Others:

| Need | Crate | ADR |
|---|---|---|
| async runtime | `tokio` | 0011 |
| transport | `iroh` (default features off, `tls-ring`; relays and global discovery with ADR 0016's work), `iroh-mdns-address-lookup`; `iroh-relay` for self-hosting | 0006, 0016 |
| keys and signatures | `ssh-key` (SSHSIG), `ed25519-dalek` | 0007 |
| join proof | `hmac` (the first build computes HMAC-SHA256 over `sha2`, checked against RFC 4231), `sha2`, `getrandom`, `data-encoding` (base32), `qrcode` | 0017 **[no ADR: crates]** |
| change hints | `notify` | 0013 |
| local state | `dirs`, `tempfile`, `toml`, `toml_edit`, `serde`, `serde_json` | 0036 |
| platform | `rustix` (`unsafe` only in the macOS writer scan), `libc` on macOS (`libproc`), `zbus` (UPower, NetworkManager) | 0037 **[no ADR: crates]** |
| property tests (dev) | `hegeltest` | 0012 |

Not used: `gix` (ADR 0010), `redb` and `rusqlite` (ADR 0036), `anyhow`
outside tests and tools (ADR 0023). `governor` and `starship-battery` are
alternatives for [15](15-resource-use.md) if needed.

## 14.8 Build rules

- Edition 2024; the stable toolchain pinned in `rust-toolchain.toml`.
- `panic = "abort"` in release; `unwrap_used`, `expect_used`, `panic` and
  `print_*` denied outside tests (`Cargo.toml` lints, `clippy.toml`), except
  next to a stated invariant (ADR 0023).
- `unsafe` only in `pigeon-sys`, each block with a safety comment; no
  English text for people in Rust code (ADR 0028).
- `cargo deny` (ADR 0019) for the four targets of ADR 0005; `deny.toml`
  records each licence exception and ignored advisory with its reason.
  `Cargo.lock` is exempt from the 1000-line rule.

## 14.9 Licence (ADR 0019)

pigeon v2 is MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`), and every
dependency must be compatible. git-annex (AGPL-3.0) runs as a separate
program; an installer that ships it keeps its licence and source offer, as
an aggregate.
