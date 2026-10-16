# 0076. pigeon updates itself from the latest release, daily and when a peer is newer

- **Status:** Accepted, 16 October 2026, by Mario.
- Extends spec 14 §14.4 and §14.5; removes the Homebrew tap.

## Context

Until now an installation changed only when someone ran `pigeon update`. A
group whose computers run different releases waits for the slowest person,
and a backup machine nobody logs in to stays behind for good. ADR 0048
already stops a device that meets a format it does not know, and asks its
owner to update; few owners will.

## Decision

- **On by default.** The installer writes, beside `pigeond`'s unit or
  agent, a daily timer at a time fixed per machine and a watch on
  `<state>/update-wanted`: systemd `pigeon-update.timer` and
  `pigeon-update.path` running `pigeon-update.service` (user units, or
  system units run as root on a backup machine), or the launchd agent
  `org.pigeon.update` on macOS.
- **Independent of the installation.** The update downloads the latest
  release's `pigeon-install` from GitHub, whole, then runs it with
  `--unattended`; no program of the installation takes part. A release
  that breaks pigeon, even its installer, is repaired by publishing the
  next release. There is no automatic rollback.
- **Trust is GitHub's.** A release is checked against its `SHA256SUMS`, as
  a manual update is; there is no signature (spec 14 §14.5). Whoever can
  publish a release runs code on every installation within a day. The
  group accepts this risk for simplicity.
- **Peers hasten it.** A status report carries `versions.pigeon` (spec 06
  §6.3). When `pigeond` keeps a newly published report whose release is
  newer than its own, it writes that release in `update-wanted`, and the
  watch starts the update at once. A pre-release or a build of main
  (`X.Y.Z+g<commit>`), on either side, asks for nothing; a report asks
  each time it is new, with no further limit.
- **The person chooses.** `pigeon update --auto off` writes
  `auto-update-off` in pigeon's directory and removes the timer and the
  watch; `--auto on` puts them back. Every installation keeps the choice.
  An unattended run installs nothing where pigeon is not installed, where
  the update is off, or where a build of main runs.
- **Silent.** The update shows no notification (ADR 0029: nothing for the
  person to do); its output goes to the supervisor's log, and a failed
  run waits for the next. It ignores battery and metered networks
  (ADR 0037), which the supervisors cannot see without pigeon.
- **No Homebrew tap.** A formula would install a second pigeon beside the
  installer's and update on its own schedule; macOS installs with the
  installer only.

## Consequences

- A fix reaches every installation within a day, and within minutes of the
  first computer of a group that takes it.
- A broken release reaches them as fast; recovery is the next release.
- `--version` and `--rollback` last until the next automatic update; to
  stay on a release, turn the update off.
- Installations made before this ADR need one `pigeon update` by hand.
- Homebrew users uninstall with `brew uninstall pigeon` and install with
  the installer; group data is not touched.
