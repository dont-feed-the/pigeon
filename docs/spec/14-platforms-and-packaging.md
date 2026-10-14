# 14. Platforms and packaging

> Mission: specify where pigeon runs, on which file systems, how git and
> git-annex reach each machine, how `pigeond` is supervised and installed, and
> how the Rust code is organised: crates, dependencies, build rules, licence.

## 14.1 Platforms (ADRs 0005, 0032)

| Role | v1 | Later |
|---|---|---|
| a person's computer (laptop) | Linux x86-64 and arm64; macOS on Apple Silicon and Intel | native Windows in v1.1 (E11); phones as read-only viewers, not designed (ux.md §11) |
| backup machine (custodian) | Linux x86-64 and arm64 with systemd | macOS; NAS vendor packages (Synology, QNAP) |

- WSL does not count as Windows support (ADR 0005).
- Nothing in the protocol or the data formats may assume a Unix-only
  primitive; Unix-only code (`renameat2`, `SIGSTOP`, `posix_fadvise`, unix
  sockets) stays behind a platform layer in `pigeon-sys` (§14.6), so that
  Windows can join in v1.1 (ADR 0005). The catalogue keeps portable names
  from day one ([05 §5.4](05-catalogue-and-names.md)).
- Minimum OS versions are those the CI runners provide (Ubuntu LTS and the
  oldest macOS runner) until a test says otherwise **[no ADR]**.

## 14.2 File systems

A laptop's shared folder must be on a local file system that keeps inodes
stable across renames, supports hard links and an atomic exclusive rename
(`renameat2(RENAME_NOREPLACE)` on Linux, `renamex_np(RENAME_EXCL)` on macOS),
which install needs ([07 §7.5](07-capture-and-install.md)).

| Supported | Refused, with a Problem at `create` or `join` and in `pigeon doctor` |
|---|---|
| ext4, btrfs, xfs (Linux); APFS (macOS) | FAT, exFAT, NTFS through FUSE, network file systems (NFS, SMB), FUSE in general, HFS+ |

**[no ADR: list]**. A case-insensitive APFS volume (the macOS default) is
supported because names are case-folded for collisions at capture. What the
folder may hold, whatever the file system, is in [07 §7.9](07-capture-and-install.md)
(ADR 0044). On APFS, extended attributes, resource forks and Finder tags are
not shared. On every file system, links are not followed and hard links are
not kept. A backup
machine may use any local Linux file system git-annex supports (ext4, xfs,
btrfs, ZFS), since its repository is bare and no person edits it.

## 14.3 git and git-annex (ADRs 0004, 0010, 0020)

- **Linux:** the installer fetches git-annex's official standalone build into
  pigeon's own directory (`~/.local/share/pigeon/git-annex.linux` for a person,
  `/opt/pigeon/git-annex.linux` for a backup machine) and uses the git it
  bundles, which meets git ≥ 2.38 (ADR 0010). `pigeond` puts that directory
  first in the `PATH` of its children; a person's own git is not touched.
- **macOS:** the installer runs `brew install git-annex` (ADR 0020), which
  brings a recent git. Homebrew installs no `git-remote-p2p-annex` (E12), so
  `pigeond` links that name to the `git-annex` it uses, in its own directory
  first in its children's `PATH`; the binary dispatches on its name
  (`CmdLine/Multicall.hs`, ADR 0056).
- **Checks at start.** `pigeond` runs `git annex version` and `git --version`
  and refuses to run below the minimum versions CI tests, with
  `git-annex-too-old` ([10 §10.6](10-interfaces.md)). The minimum is the
  oldest version CI tests on each platform (ADR 0067): on
  Linux the pinned standalone build, **10.20261006**; on macOS the version
  Homebrew provides, which was **10.20260901** when E20 ran in `lab.yml`.
- **Untested versions** (ADR 0048). A git or git-annex newer than the newest
  CI tested is *untested*. The first time `pigeond` starts with a version it has
  not run before, it runs the contract self-test: a subset of the contract
  tests ([13 §13.6](13-verification.md)) on a scratch repository, under 30
  seconds, at idle priority. If a check fails, it keeps capturing and serving
  but suspends every drop, and shows *Needs you* (`git-annex-untested`) until
  a version passes.
- **Upgrades are deliberate.** The standalone build publishes "current" only;
  CI records the version it tested, the installer verifies the version it
  fetched, and a new git-annex is supported once the contract tests pass on
  it ([13 §13.6](13-verification.md)). A missing git-annex feature is sent
  upstream as a patch, not worked around (ADR 0004).
- `ssh-keygen` is required for signing (ADR 0007); it is part of the base
  system on Linux distributions with OpenSSH and on macOS.

## 14.4 Supervision (ADR 0011)

On a person's Linux computer, a **systemd user unit** (ADR 0037 for the
resource settings, [15 §15.1](15-resource-use.md)):

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

On a backup machine, a **system unit** `pigeond.service` running as a
dedicated user `pigeon`, at normal priority, with scrubs moved to the idle I/O
class by `pigeond` itself (ADRs 0032, 0037). On macOS, a **launchd agent**
`~/Library/LaunchAgents/org.pigeon.pigeond.plist` with `KeepAlive`,
`RunAtLoad` and `ProcessType = Background`.

- `pigeond` is restarted by its supervisor after a panic (release builds use
  `panic = "abort"`, ADR 0023); recovery at any instant is required anyway
  (P2, [11](11-local-state.md)).
- On exit, `pigeond` stops its git-annex children (`remotedaemon`, batch
  processes) and closes its endpoint.
- The `MemoryHigh` value is a default to be checked by the budgets of
  [15 §15.7](15-resource-use.md) **[no ADR: value]**.

## 14.5 Packaging and installation

| Platform | Package | Holds |
|---|---|---|
| Linux, people | a tarball and an install script | `pigeon`, `pigeond`, `git-annex-p2p-iroh`, the user unit, the `.desktop` launcher and icon, man pages, shell completions; the script fetches git-annex (§14.3) and enables the unit |
| Linux, backup machines | a static tarball and a system unit (ADR 0032) | the same binaries, the system unit, a `pigeon` user created by the script |
| macOS | a Homebrew formula in pigeon's tap **[no ADR]** | the binaries, the launchd agent, the small launcher app bundle (ADR 0029); depends on `git-annex` |

- Release archives come with SHA-256 checksums and are built by CI from a tag
  **[no ADR: signing]**.
- Installers for non-technical people (a signed `.pkg`, distribution packages)
  are not designed in v1: Paul installs (ux.md §11).
- Uninstalling removes the binaries and the unit and keeps the folder, the
  repository and the configuration, unless the person asks.

## 14.6 The Rust workspace (ADR 0058, Proposed)

| Crate | Kind | Holds | May do I/O |
|---|---|---|---|
| `pigeon-core` | lib | the pure core: roster, authority rule, catalogue merge, names, states, headline, queue order, journal replay, filter state machine, trace event enums (ADR 0012) | no |
| `pigeon-proto` | lib | the request and response types of the API and the socket framing (ADR 0021) | no |
| `pigeon-i18n` | lib | Fluent bundles, language choice, the completeness test (ADRs 0028, 0068) | reads embedded files only |
| `pigeon-git` | lib | drivers for git and git-annex: batch processes, JSON parsing, contract tests (ADRs 0004, 0010) | yes |
| `pigeon-sys` | lib | the platform layer: renames, signals, `fadvise`, pressure, battery and metered probes, sockets (ADR 0037) | yes |
| `pigeon-telemetry` | lib | the only crate depending on `opentelemetry*` (ADR 0025) | yes |
| `pigeon-web` | lib | axum routes and maud pages (ADR 0021) | through `pigeon-proto` |
| `pigeond` | bin | the daemon: loops, iroh endpoint, socket, web server | yes |
| `pigeon` | bin | the CLI (ADR 0024) | socket only |
| `git-annex-p2p-iroh` | bin | the transport helper (ADR 0006) | socket and stdio only |

Each crate lives in `crates/<name>/` and is created when its work starts.
`pigeon-core` depends on no crate that performs I/O; `scripts/check-core-deps`
enforces it in CI (`cargo tree` against `crates/pigeon-core/allowed-deps.txt`).

## 14.7 Dependencies

Versions as surveyed on 13 October 2026; each is pinned to one minor version
and upgraded deliberately.

| Need | Crate | ADR |
|---|---|---|
| async runtime | `tokio` 1.53 | 0011 |
| transport | `iroh` 1.3 (default features off, `tls-ring`), `iroh-mdns-address-lookup` 0.6; `iroh-relay` 1.3 for self-hosting | 0006, 0016, 0066 |
| keys and signatures | `ssh-key` 0.6 (SSHSIG), `ed25519-dalek` 3.0 | 0007 |
| join proof | `hmac` 0.13, `sha2` 0.11; `data-encoding` 2.11 (base32); `qrcode` 0.14 | 0017 **[no ADR: crates]** |
| change hints | `notify` 8.2 | 0013 |
| CLI | `clap` 4.6, `anstream` 1.0 / `anstyle` 1.0, `indicatif` 0.18 | 0024 |
| web | `axum` 0.8, `maud` 0.27 | 0021 |
| errors | `thiserror` 2.0 | 0023 |
| logs | `tracing` 0.1.44, `tracing-subscriber` 0.3, `tracing-appender` 0.2 | 0022 |
| telemetry | `opentelemetry` 0.33, `tracing-opentelemetry` 0.34 | 0025 |
| messages | `fluent-bundle` 0.16, `fluent-langneg` 0.13 (the version `fluent-bundle` uses), `fluent-syntax` 0.12 (tests) | 0028, 0068 |
| notifications, launcher | `notify-rust` 4.18, `open` 5.4; `tray-icon` 0.26 in v1.1 | 0029 |
| times | `jiff` 0.2 | 0028 |
| local state | `dirs` 7.0, `tempfile` 3.27, `toml` 1.1, `toml_edit` 0.25, `serde` 1.0, `serde_json` 1.0 | 0036 |
| platform | `rustix` 1.1 (renames, `fadvise`, signals), `zbus` 5.19 (UPower, NetworkManager) | 0037, 0060 |
| property tests (dev) | `hegeltest` 0.49 | 0012 |

Not used: `gix` (ADR 0010: the git CLI only), `redb` and `rusqlite`
(ADR 0036: plain files), `anyhow` outside tests and tools (ADR 0023).
`governor` 0.10 (a rate limiter) and `starship-battery` 0.12 are alternatives
for [15](15-resource-use.md) if the shaper or the battery probe needs them.

## 14.8 Build rules

- Rust edition 2024, stable toolchain 1.99.0 pinned in `rust-toolchain.toml`
  (ADR 0058, Proposed).
- `panic = "abort"` in release; clippy's `unwrap_used`, `expect_used`,
  `panic` and `print_*` denied outside tests, except next to a stated
  invariant (ADRs 0023, 0058).
- `unsafe` only in `pigeon-sys`, each block with a safety comment.
- English text meant for people never appears in Rust code (ADR 0028).
- `cargo deny` checks licences and advisories in CI (ADR 0019).

## 14.9 Licence (ADR 0019)

pigeon v2 is licensed MIT OR Apache-2.0, at the user's option (`LICENSE-MIT`,
`LICENSE-APACHE`). Every dependency must be compatible with that dual licence.
git-annex (AGPL-3.0 for the whole program) runs as a separate program; where
an installer ships a git-annex build, it stays under its own licence with its
source offer, as an aggregate.
