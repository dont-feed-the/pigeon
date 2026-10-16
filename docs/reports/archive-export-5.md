# Report: archived recovery access

This 0.4.7 candidate addresses the two access problems found in the
independent backup sessions of [refound-5.md](refound-5.md): archived deleted
files needed an internal ref passed to offline export, and history called
a joined bare backup a non-member.

## Behavior

`pigeon export --deleted` now follows nested signed refounding links from
the selected history, requiring the exact canonical archive ref and old
genesis. It exports an archived deleted path once and keeps current paths
in preference to old deletions. Missing, substituted, unsigned or ambiguous
links refuse explicitly. Current-only export remains available.

The offline command checks archive-link signatures and annex content
size/SHA-256, without a daemon, network or secret key. It does not run the
daemon's complete authority fold. The normal online join path separately
validates that history before adopting it.

`history` and read-only `view` accept virtual file paths below a bare
backup, including nested folders that do not exist on disk. Path traversal
outside that repository is refused. View writes a separate read-only copy.

Output names never replace an exported user file. Unique temporary files
avoid a collision with a legitimate `.NAME.pigeon-part` file. Metadata
names avoid case-folded user paths on every platform. A deleted symlink
cannot be followed as an output directory; incompatible historical
file/folder paths are recorded in `MISSING.tsv` as `destination-conflict`,
with originals retained for separate export.

## Validation

- Four archive-link laws ran 1000 cases each; the existing five export
  laws also ran 1000 cases each, including portable metadata-name folding.
- Five local archive integrations passed: nested deleted content, broken
  links, symlink conflicts, temporary-name collisions and metadata names.
- Existing real git-annex partial/damaged-object export coverage passed.
- A real two-daemon integration checked two old versions and an exact
  read-only copy on a rejoined bare backup; it passed in 6.19 seconds.
- Bare-path escape checks, translation/catalogue checks and full-target
  Clippy passed.
- The independent participant reran both original admin-owned and
  member-owned backup fixtures with the normal 0.4.7 build. Default
  `--deleted` export recovered the exact archived deleted bytes without
  an internal ref in 54–55 milliseconds. Current and older exports stayed
  exact. History and read-only views exposed current, older and deleted
  versions through virtual paths absent on disk. Copies were mode 0444,
  outside the repository; annex token, object bytes and inodes were
  unchanged. The full harness exited successfully and cleaned its daemons.

The user accepted agent participants as usability evidence. No cross-OS
CI, nightly streak or soak result is claimed by these local checks.
