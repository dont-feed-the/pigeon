# Report: reach and installation readiness

M3 work toward the [roadmap](../roadmap.md), following
[people-3.md](people-3.md). This report distinguishes implemented work
from the CI and release evidence still required to tag the milestone.

## Implemented

- n0 relays and discovery are already the default; an optional group relay
  remains supported. The existing cross-runner reach scenario passed in
  [run 38750098867](https://github.com/dont-feed-the/pigeon/actions/runs/38750098867).
- Personal Linux installation uses a systemd user unit. A backup machine
  can install with `pigeon-install --system`: its own unprivileged account,
  `/var/lib/pigeon`, `/var/log/pigeon`, `/var/cache/pigeon`, and a system
  unit invoking `pigeond --service`. systemd creates the runtime directory.
- macOS installation uses a launchd agent. `Formula/pigeon.rb` makes this
  repository a Homebrew tap on Apple Silicon and includes patched git-annex
  and a launcher. The formula is generated from published SHA256SUMS;
  releases update it only after assets are published.
- `--uninstall` stops the appropriate service and removes programs and
  launchers while retaining group data, configuration and the device key.
  A rollback can use the previous helper when the older release has none.
- Archive installs, idempotence, updates, rollback, failed downloads,
  supervisor path escaping and data-preserving uninstall have automated
  lifecycle checks. CI runs them on Linux and macOS. The release workflow
  also installs and removes the real Linux assets before publication.
- Publication waits for the candidate tag's CI, lab, slice and reach;
  an older green run or a green branch cannot authorize different assets.

## Dogfood and usability evidence

Independent agent participants created groups, invited and joined other
devices, shared files, restored versions, freed and kept folders, and
preserved concurrent edits. The user accepted these sessions as human
usability evidence; [usability-5.md](usability-5.md) records tasks and
retests. The devices were isolated processes on one Linux host, not
physical computers owned by Mario, Guillaume and Laurent. The relay
scenario supplies the separate-network evidence.

## Release gates

M3 remains untagged until the integrated candidate passes CI on both
platforms, installation/uninstallation checks and the relay scenario.
The Homebrew tap initially points to the already published alpha; the
release workflow replaces its formula after publishing the candidate.
Linux arm64 and Intel Mac archives, a fully static backup-machine archive,
and verification of a real Homebrew install remain packaging gaps against
the complete platform specification. No seven-night model streak,
72-hour soak or two-week release-candidate dogfood is claimed here.
