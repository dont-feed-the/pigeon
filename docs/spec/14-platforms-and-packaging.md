# 14. Platforms and packaging

> Mission: where pigeon runs, on which file systems, how git and git-annex
> reach each machine, how `pigeond` is supervised and installed, and how the
> Rust code is organised: crates, dependencies, build rules, licence.

## 14.1 Platforms (ADRs 0005, 0032)

| Role | v1 | Later |
|---|---|---|
| a person's computer (laptop) | Linux x86-64 and arm64; macOS on Apple Silicon and Intel | native Windows in v1.1 (E11); phones as read-only viewers, not designed (ux.md §11) |
| backup machine (custodian) | Linux x86-64 and arm64 with systemd | macOS; NAS vendor packages (Synology, QNAP) |

- WSL is not Windows support. Unix-only code (`renameat2`, `SIGSTOP`,
  `posix_fadvise`, unix sockets) stays behind `pigeon-sys` (§14.6) and no
  protocol or format assumes it, so Windows can join in v1.1; names are
  portable from day one ([05 §5.4](05-catalogue-and-names.md)).
- Minimum OS versions are those of the CI runners (Ubuntu LTS, the oldest
  macOS runner) until a test says otherwise **[no ADR]**.

## 14.2 File systems

A laptop's folder must be on a local file system with inodes stable across
renames, hard links, and an atomic exclusive rename (`RENAME_NOREPLACE`,
`RENAME_EXCL`), which install needs ([07 §7.5](07-capture-and-install.md)):

| Supported | Refused, with a Problem at `create` or `join` and in `pigeon doctor` |
|---|---|
| ext4, btrfs, xfs (Linux); APFS (macOS) | FAT, exFAT, NTFS through FUSE, network file systems (NFS, SMB), FUSE in general, HFS+ |

**[no ADR: list]**. A case-insensitive APFS volume is supported, since names
are case-folded for collisions at capture. What the folder may hold is in
[07 §7.9](07-capture-and-install.md). A backup machine may use any local
Linux file system git-annex supports (ext4, xfs, btrfs, ZFS): its repository
is bare.

## 14.3 git and git-annex (ADRs 0004, 0010, 0020)

- **Linux:** the installer fetches git-annex's standalone build into
  `~/.local/share/pigeon/git-annex.linux` (`/opt/pigeon/git-annex.linux` on
  a backup machine) and uses the git it bundles (≥ 2.38, ADR 0010), first in
  the `PATH` of `pigeond`'s children; the person's git is not touched.
- **macOS:** `brew install git-annex` (ADR 0020). Homebrew installs no
  `git-remote-p2p-annex` (E12), so `pigeond` links that name to its
  `git-annex` in its own directory, first in its children's `PATH`.
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
  installer verifies what it fetched. A missing git-annex feature goes
  upstream as a patch (ADR 0004).
- `ssh-keygen` is required for signing (ADR 0007).

## 14.4 Supervision (ADR 0011)

A person's Linux computer runs a **systemd user unit** (resource settings:
ADR 0037, [15 §15.1](15-resource-use.md)):

```ini
[Unit]
Description=pigeon: share and keep the group's files

[Service]
ExecStart=%h/.local/share/pigeon/bin/pigeond
Restart=on-failure
RestartSec=5
Nice=10
IOSchedulingClass=idle
CPUWeight=20
IOWeight=20
MemoryHigh=512M

[Install]
WantedBy=default.target
```

A backup machine runs a **system unit** `pigeond.service` as a dedicated
user `pigeon`, at normal priority, scrubs in the idle I/O class (ADRs 0032,
0037). macOS runs a **launchd agent**
`~/Library/LaunchAgents/org.pigeon.pigeond.plist` (`KeepAlive`,
`RunAtLoad`, `ProcessType = Background`). The supervisor restarts `pigeond`
after a panic (`panic = "abort"`, ADR 0023); on exit `pigeond` stops its
git-annex children and closes its endpoint. `MemoryHigh` is a default for
the budgets of [15 §15.7](15-resource-use.md) to check **[no ADR: value]**.

## 14.5 Packaging and installation

| Platform | Package | Holds |
|---|---|---|
| Linux, people | a tarball and an install script | `pigeon`, `pigeond`, `git-annex-p2p-iroh`, the user unit, the `.desktop` launcher and icon, man pages, shell completions; the script fetches git-annex (§14.3) and enables the unit |
| Linux, backup machines | a static tarball and a system unit (ADR 0032) | the same binaries, the system unit, a `pigeon` user created by the script |
| macOS | a Homebrew formula in pigeon's tap **[no ADR]** | the binaries, the launchd agent, the small launcher app bundle (ADR 0029); depends on `git-annex` |

Release archives come with SHA-256 checksums, built by CI from a tag
**[no ADR: signing]**. Installers for non-technical people are not designed
in v1: Paul installs (ux.md §11). Uninstalling keeps the folder, the
repository and the configuration unless the person asks.

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
| `pigeon` | bin | the CLI (ADR 0024) | socket only |
| `git-annex-p2p-iroh` | bin | the transport helper (ADR 0006) | socket and stdio only |
| `pigeon-lab` | lib and bin, never shipped | the slice's scenarios as typed values, their model interpreter over `pigeon-core` and their process executor ([13 §13.12](13-verification.md)) | yes: it runs `pigeond` and `pigeon` and mounts disk images |

Each crate lives in `crates/<name>/`, created when its work starts
(`pigeon-web` and `pigeon-telemetry` are not yet). `pigeon-core` depends on
no crate that does I/O: `scripts/check-core-deps` checks `cargo tree`
against `crates/pigeon-core/allowed-deps.txt` in CI.

## 14.7 Dependencies

Each is pinned in `Cargo.toml` to one minor version and upgraded
deliberately. The crates for logs, telemetry, the CLI, the web, errors,
messages, times and notifications are those of ADRs 0021–0025, 0028 and
0029. Others:

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

- Rust edition 2024, stable toolchain pinned in `rust-toolchain.toml`.
- `panic = "abort"` in release; clippy's `unwrap_used`, `expect_used`,
  `panic` and `print_*` denied outside tests, except next to a stated
  invariant (ADR 0023).
- `unsafe` only in `pigeon-sys`, each block with a safety comment; no
  English text for people in Rust code (ADR 0028).
- `cargo deny` (ADR 0019) for the four targets of ADR 0005; `deny.toml`
  records each licence exception (MPL-2.0 for `option-ext`,
  CDLA-Permissive-2.0 for `webpki-roots` and `webpki-root-certs`) and each
  ignored advisory with its reason. `Cargo.lock` is exempt from the
  1000-line rule.

## 14.9 Licence (ADR 0019)

pigeon v2 is MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`), and every
dependency must be compatible. git-annex (AGPL-3.0) runs as a separate
program; an installer that ships it keeps its licence and source offer, as
an aggregate.
