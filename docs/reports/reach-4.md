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
retests. The devices were isolated processes on one Linux host. The user
instructed us to assume the members' dogfood has been done and not to let
it block progress. That gate is treated as satisfied by this instruction,
rather than claimed as a measured field trial. The relay scenario supplies
the separate-network evidence.

## Release gates

The M3 release candidate is 0.4.0. It remains untagged until it passes CI on both
platforms, installation/uninstallation checks and the relay scenario.
The Homebrew tap initially points to the already published alpha; the
release workflow replaces its formula after publishing the candidate.
CI also installs the tap's real published binaries on macOS, and the
release workflow starts the actual Linux system service as its dedicated
account before uninstalling it and checking that its key remains. Linux
arm64 and Intel Mac archives and a fully static backup-machine archive
remain packaging gaps against the complete v1.0 platform specification.
No seven-night model streak,
72-hour soak or two-week release-candidate dogfood is claimed here.

The first real Homebrew installation in run 38826115651 installed and
executed pigeon but found annex at the wrong depth: Homebrew strips the
archive's single `bin/` directory. The formula now handles both extraction
layouts and its own `brew test` executes patched git-annex. The corrected
formula must pass the macOS job before release.

Review of the recovery bootstrap found that `fetch_welcome` checked the
expected genesis and membership but did not run the normal authority judge
before adopting the fetched main. All join and recovery paths now call the
same history judge as normal synchronization, requiring an accepted tip,
an accepted welcome commit and current membership. Regressions cover an
unsigned commit, a forged signature, an unknown signer, a forbidden path,
a wrong genesis and membership removed after the welcome; rejected history
never moves main. This check is part of the candidate before publication.

The formula uses Homebrew's version scheme to upgrade from the legacy
`2.0.0-alpha.0` to the roadmap's `0.4.0`, despite the smaller major number.
Both installation jobs passed in run 38827160363, including real macOS
binary execution. That run's daemon tests still used stock git-annex and
the mass-deletion case failed once; CI now requires the project's cached
patched build and its system-git shim, as the release and lab already do.
The corrected candidate still needs a green run before tagging.

Upstream 0.3.5's macOS dependency-build fix is included: git-annex builds
on macOS 15 while keeping the existing artifact/cache names for newer
macOS runners. The old candidate's Linux Rust job and real macOS Homebrew
installation passed in run 38830164002. The combined candidate is checked
again after integrating upstream; publication still waits for its tag.

The combined run 38834010811 passed packaging and model checks but its
Rust job stopped before tests because GitHub had evicted the required
dependency cache. The successful main-branch producer's artifact remained
available. A cache miss now checks successful producers from this
repository whose commits are ancestors of the candidate, compares all
build inputs, verifies GitHub's archive digest and validates the native
architecture and exact patched version before installing it. Twelve
regressions cover source drift, provenance, failed builds, archive escapes
and credential stripping on download redirects. This avoids treating an
evicted cache as a reason to use stock git-annex. The repaired workflow
still needs its own green run.
