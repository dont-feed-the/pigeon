# 0049. Copies count once per failure domain: shared disks, machines and declared domains

- **Status:** Proposed, 14 October 2026, by the agent, for issue #12
  (Guillaume's review: "Distinguish independent failure domains from distinct
  custody repositories"); revised 14 October 2026 on the evidence of the
  `Repair` model (issue #10), absorbing part of the former ADR 0059 (domain
  repair counts only members that can take a key; epochs counted on intact
  copies). Refines how ADR 0002 counts its K custody copies.

## Context

ADR 0002 makes a version protected when K distinct custodians hold it.
git-annex counts repository UUIDs: `numCopiesCount` in `Annex/NumCopies.hs`
is the length of the UUID list, cluster UUIDs aside. Two UUIDs are not two
backups. Two custodian repositories can sit on one disk, two virtual machines
on one host, and a reinitialized repository has a new UUID while its old UUID
still lists the same objects in the location log. Two machines in one home
burn together. The earlier review (REVIEW.md, S7) noted this, but the
specification still counted repositories. E22 shows the effect: git-annex
counts two repositories on one disk as two copies, even when
`annex.hardlink` makes them hold one inode, and a verified drop that relies
on them leaves one physical copy. Only a `git clone --shared` is made
untrusted (`initSharedClone` in `Annex/Init.hs`).

Custodians run on Linux with systemd (ADR 0032), so pigeon can read their
disks and machine identity. It cannot see through a hypervisor, and it cannot
know where a machine stands.

## Decision

**One chain of identity.** A custodian is one device key, which has one
repository per group (03 §3.1), which has one UUID for its whole life (ADR 0046),
which lives on one storage. A new UUID is a new device. The custodian's
`pigeond` refuses to serve a repository whose `annex/objects` directory is
on another file system than the repository itself. Repositories reach each
other only through the P2P transport (`p2p-annex::`, ADR 0006), never by a
local path, and pigeon sets `annex.hardlink=false`, so no two repositories
share an inode or copy-on-write extents (`wantHardLink` and `mkFileCopier` in
`Remote/Git.hs` link or reflink only between local paths). Before each status
report and each served session, the custodian's `pigeond` checks that its
repository is still there, with its UUID and its storage fingerprint. If not,
it stops serving and reports `repository-lost`: E22 shows git-annex, which
checks a remote's UUID once per process (`Remote/Git.hs`), writing the next
key into a deleted repository's path and counting it there.

**Failure domains.** Two custodians are in the same failure domain when:

1. their **storage** fingerprints intersect: some disk or network share holds
   both repositories;
2. their **host** fingerprints are equal: they run on one machine, containers
   and chroots included;
3. or the roster gives them the same `domain` label: hardware pigeon cannot
   see, such as virtual machines on one host or disks behind one controller.

A failure domain is a connected component of this relation. Domains are a pure
function of the roster and the accepted status reports, computed by
`pigeon-core`. Detection and declarations can only merge custodians into one
domain; nothing splits custodians that a fingerprint joins. Two disks in one
machine are one domain, since a theft, a power surge or a fire takes both.

**Fingerprints.** Each custodian reports them in its status report
([06 §6.3](../spec/06-status.md)), recomputed at start and every hour:

- `storage`: every whole disk under the file system of the repository. The
  mount comes from `/proc/self/mountinfo`, and
  `lsblk -s -n -P -o NAME,TYPE,WWN,SERIAL <device>` walks partitions,
  device-mapper, LVM and md down to the rows of type `disk` (`-P`, since the
  plain form loses empty columns). Each disk is named by its WWN, else its
  serial, else its `/dev/disk/by-id` name; a serial is never used alone when
  the WWN exists, since CI's NVMe disks all report `SN: 00000` (E22). A loop
  device is followed to its backing file (`/sys/block/loopN/loop/backing_file`)
  and named by the storage of that file. A network file system is named by
  its server and export. Else the file system's UUID.
- `host`: `/etc/machine-id`.
- `virtual`: what `systemd-detect-virt` says (`""` on bare metal).
- A value pigeon cannot find is reported as unknown, never guessed (ADR
  0048), and the custodian gets the `independence-unknown` notice below.
  E22 shows why the mount source is not a fallback: every tmpfs has the
  source `tmpfs`, and only the device number, which a remount changes, tells
  them apart. A repository on `tmpfs` or `ramfs` is refused outright, since a
  reboot empties it.

Each value is reported as the first 16 bytes of SHA-256 over
`pigeon-domain`, the group ID and the identifier. Members learn which
custodians share hardware, not serial numbers, and hashes differ between
groups. E22 asserts that two repositories on one volume get one storage and
one host fingerprint, that a loop mount of an image file gets the
fingerprint of the disk holding the file, and that a tmpfs gets another. It
also records the macOS identifiers (`diskutil` DiskUUID of the APFS physical
store, `ioreg` IOPlatformUUID), for the day custodians run there.

**Sites.** The roster may give a custodian a `site`, a label for where it
stands ("Paul's home"), and the group a `min_sites`, by default 1. pigeon
says "in 2 places" only when sites are declared. Without them it says "on 2
backup machines" and nothing about places. Declarations follow the authority
of `maxsize`: admins, and the owner of the device for its own entry.

**Counting.** H ([06 §6.2](../spec/06-status.md)) keeps its members, and a
version is protected when the members of H span **K failure domains** and at
least `min_sites` declared sites. Its time T is the K-th most recent among the
domains' latest `heard`. When H has K custodians but fewer domains or sites,
the version stays *published* with reason `same-domain` or `same-site`.

**Drops lock one copy per domain.** Every verified drop (laptop eviction,
freeing a folder, rebalancing) is run so that git-annex can only lock copies
in distinct domains. `pigeond` chooses K custodians that hold the key, in K
domains and spanning `min_sites` sites, preferring reachable ones. It runs
`git annex drop --key <key> --numcopies K --mincopies K` with
`-c remote.<name>.annex-ignore=true` for every other remote. git-annex verifies
only remotes that are not ignored (`verifiableCopies` in `Annex/NumCopies.hs`
calls `remoteLocations` with `IncludeIgnored False`, and `remoteLocations'` in
`Remote.hs` filters out ignored remotes), so the K copies it locks are in K
domains: E22 shows the drop refusing while only remotes of one domain hold
the key, and succeeding once another domain does. If no such choice exists, the drop waits. Raising
`--numcopies` instead would race: a holder that appears during the drop
changes the worst case.

**Domain repair.** `sizebalanced` stays the placement (ADR 0003) and knows
nothing of domains, so it may give a key to two custodians of one domain.
When the custodians that can take a key span d < K domains, the
custodians of the other domains with room for it (free space above
`annex.diskreserve`, within `maxsize`) rank themselves by SHA-256 of the key
and their device ID. The best-ranked custodian of each of the first K − d
domains in that order wants the key too. This is a pure function of the
observations, so every custodian computes the same choice. Rebalancing never
drops a key its custodian wants this way. It is not a planner: it only fills
the deficit that `sizebalanced` leaves. The custodians that *can take* a key
are those that hold it, are downloading it, or want it and have room above
their reserve for it, as their status reports say: `sizebalanced`'s want
ignores real free space (`maxsize` is advisory, ADR 0050), and git-annex
refuses a download past the reserve (`checkDiskSpace'` in
`Annex/Content/LowLevel.hs`). Counting every member that wants the key, the
`Repair` model ([13 §13.2](../spec/13-verification.md)) finds a full custodian
in a missing domain that hides the deficit forever (`Repair_wantall`,
property `Restores`).

**Telling people.**

- Two custodians in one domain (`same-domain`): *Needs you* for the admins
  and the owners:
  "Paul's NAS and Paul's old PC are on one machine; they count as one backup.
  Move one to another machine, or remove it."
- A custodian in a virtual machine or container with no declared `domain`,
  or whose storage or host pigeon cannot identify (`independence-unknown`): a notice to its owner and the admins, and a line on its page: "pigeon
  cannot see which hardware runs Paul's NAS. If it shares a machine with
  another backup machine, say so." `pigeon doctor` says the same.
- A custodian whose repository is gone (`repository-lost`): *Needs you* for
  its owner and the admins, "Paul's NAS no longer finds its backup folder; it
  counts for nothing until it is set up again." Its copies stop counting at
  once, not after `offline_after`.
- A group whose declared sites are all one: the group page says "All backup
  machines are at Paul's home."

**Guarantee.** Assumption F ([02 §2.2](../spec/02-guarantees.md)) is
restated over failure domains: within a repair epoch of V, fewer than K of
the failure domains holding V lose or corrupt it. A repair epoch for V
starts when V's *intact* custody copies span fewer than K domains, from the
moment a copy rots, not when a scrub finds it; a version not yet protected is
in an epoch too (02 §2.1). Counted on verified copies, a rot before the epoch
and one loss within it destroy V while F still holds. Failures that take a whole site are
inside F only when the group declares sites and sets `min_sites` ≥ 2.
pigeon never infers geography from the number of machines.

## Consequences

- Fewer versions are *Safe* where custodians share hardware, and the reason
  says why. This is P6: the count was optimistic before.
- A reinitialized repository has a new UUID, but it reports the same storage
  as the old one, so the two count once while the old device is not yet
  removed. The old one also stops being heard, and stops counting after
  `offline_after`.
- A group whose custodians all run on one host cannot reach K ≥ 2. It is
  told so, instead of believing it has two backups.
- Virtual machines on one host with virtual disks look independent unless
  someone declares a `domain`. The notice, `pigeon doctor` and 02 §2.4 say so.
- Domain repair may store a key beyond `sizebalanced`'s share. Capacity
  ([08 §8.9](../spec/08-custody-loops.md), ADR 0050) counts it.
- Each drop command carries one `-c` option per ignored remote. Groups have
  few custodians, so the command stays short.
- Laws: the number of domains is at most the number of custodians; adding a
  fingerprint or a declaration never raises a version's state; with one
  custodian per domain, the state equals the state of ADR 0002; the drop's
  choice puts the K locked remotes in K distinct domains; domain repair is
  deterministic and wants no key that already spans K domains.

## Alternatives considered

- *Untrust all but one custodian of a domain.* Trust is per UUID, so the
  others' copies would never count, even for keys held only there. Rejected.
- *git-annex clusters.* A cluster groups nodes behind one remote. Its nodes
  still count as distinct copies, so it does not express a shared disk.
- *Raise `--numcopies` to the worst case* (the K − 1 largest domains plus
  one). A holder that appears during the drop breaks the bound. Rejected for
  the exact choice above.
- *A failure-domain-aware placement planner.* Out of scope (ADR 0003, issue
  #12). Domain repair is the minimum that lets protection recover.
- *Guess sites from network addresses.* Two custodians behind one public
  address are often in one home, but not always. This is left as a possible
  hint in 16 §16.4, and is never counted.
