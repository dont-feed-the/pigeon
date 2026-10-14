# 0047. A group is recovered from its custodians: rejoin, recovery sheets, refounding, and export without pigeon

- **Status:** Accepted, 14 October 2026, by Guillaume (chat: the review
  sheet of the four Proposed ADRs); proposed by the agent for issue #11
  (Guillaume's review: "Specify and test recovery onto fresh machines from
  surviving custodians"), on the evidence of experiment E21.

## Context

ADR 0046 places everything a group needs on its custodians. Using it again
also takes authority: a new machine takes part only once a signed roster
commit adds it (ADRs 0007, 0009, 0017). ADR 0017 lets an admin add any device,
and a device of person P add devices of P. A custodian is a device of its
owner, so it carries that person's authority, and an admin's custodian carries
an admin's.

The disaster drill of the issue begins with every laptop lost. Four cases
follow, from the most common to the rarest:

1. a device of the person survives, often the custodian at their home;
2. a device of an admin survives;
3. no admin device survives, but the group prepared for it;
4. nothing that can sign for an admin survives.

A rule that let a surviving custodian, or anyone holding a disk, sign for the
lost admins would be the implicit bypass the issue excludes. The issue also
asks for a way to read the files when pigeon itself cannot run.

## Decision

**Rejoin by invitation.** A fresh machine recovers like any new device: by an
invitation (09 §9.6) from a surviving device of its person, or of an admin.
A custodian makes one with `pigeon invite`, run on it directly or over ssh.
The new machine fetches from the custodians and validates the history from
the genesis named by the invitation. It gets the content of the folders it
keeps, as on any join. Once joined, its person removes the lost devices
(*I lost a computer*, 09 §9.7).

**Recovery sheets** for case 3.

- `pigeon recovery-sheet` (People → *Print a recovery sheet*, admins only)
  generates an ed25519 key. It adds the key to the roster as a device of that
  admin with `role = "recovery"`, in a commit allowed by ADR 0017, and prints
  a sheet. The sheet holds the group's name and ID, the person, the key as
  base32 with a checksum and as a QR code, the device IDs of the custodians
  at that moment, and the instructions. No computer keeps the key.
- A recovery device has no `annex_uuid`, no content and no remote for content.
  It never counts and is never shown as offline. Its commits are accepted
  only if they add or remove devices of its own person, and nothing else
  (04 §4.3).
- `pigeon recover SHEET [FOLDER]` on a fresh machine runs as the recovery
  device until it is done. It connects to a custodian named on the sheet, or
  one given with `--from`; custodians admit it, since it is in the roster. It
  fetches and validates `main` from the genesis named on the sheet. It
  generates the machine's own key, signs with the sheet's key a roster commit
  that adds the machine as a device of the admin, and stays reachable until a
  custodian has fetched that commit (pull only, [09 §9.4](../spec/09-network-and-joining.md)). From then on the
  machine is that admin's device and continues as after a join.
- Its first steps are offered at once: remove the lost devices and the used
  recovery device, whose key has now been typed into a machine, then print a
  new sheet. The People page recommends a sheet, or a second admin device,
  while an admin has only one device that can sign. pigeon also offers a new
  sheet when the custodians named on the old one have changed.

**Refounding** for case 4, as the last resort.

- `pigeon refound [REPOSITORY]` runs on any device that holds a repository of
  the group, a custodian's included. It writes the genesis of a new group: its
  tree is the last accepted tree of `main`, with a roster that names the
  refounder as the only admin. It carries the trailer
  `Pigeon-Refound: <old group id> <old main>`. It is a new group with a new
  ID.
- The old history stays readable as `refs/pigeon/archive/<old group id>/main`.
  It replicates like proposals, and its keys stay in `keys()`, so no version
  of the old history loses its protection.
- Every other member joins the new group only by an invitation from it. The
  invitation says "*Famille Geiger* was refounded by Paul on 3 March: joining
  it means accepting Paul as its admin". A member's existing folder joins in
  place: the same `SHA256E` keys mean nothing is sent again.
- The old group's history is never extended by a commit its rules reject;
  every signature in either group is checked as before.

**Reading files without pigeon.**

- `pigeon export --to DIR [--at DATE|COMMIT] [--deleted] [REPOSITORY…]` needs
  no `pigeond`, no network and no key. It reads one or more local
  repositories, laptops or custodians, bare or not, so that several partial
  custodians can be combined (ADR 0003). It checks the SHA-256 of every object
  before it writes the file, and writes plain files with their execute bit.
  Its `MANIFEST.tsv` gives the path, size, SHA-256, commit, author and date of
  each file, and it lists what it could not find ("312 files are on other
  backup machines").
- Each custodian keeps a `RECOVERY.txt` next to its repository, outside git and
  rewritten when the roster changes. It gives the group's name and the names
  of the other custodians, and the commands that read the files with git and
  git-annex alone, as E21 runs them: `git clone <custodian> <folder>`,
  `git annex init`, then `git remote add` and `git fetch` for each other
  custodian, then `git annex get .` for the current files. `git annex get
  --all` adds the history; it reports failures for keys that only a laptop
  ever held, which RECOVERY.txt says to expect.

**Replacing a custodian.** A failed custodian is removed (09 §9.7) and its
replacement joins by invitation as a new device. A surviving disk is reused
under a new UUID and counts only after a hashing fsck (ADR 0046).

## Consequences

- No case lets anything sign for an admin without a key that admin created:
  a device, or a sheet. The absence of both is visible beforehand, as a
  recommendation on the People page.
- A sheet is a key on paper. Whoever holds it can add devices for its admin,
  and through them act as that admin until they are removed. It is kept like
  a passport; keys are not stolen (02 §2.2), and a sheet is a key.
- A refounded group keeps every file, every version and the old signed
  history. It loses only the old group ID: every member must accept the new
  group explicitly.
- The roster gains `role = "recovery"`; the CLI gains `recovery-sheet`,
  `recover`, `refound` and `export`; the `.pigeon/` table gains a row for
  recovery devices.
- E21 is the disaster drill. It loses every laptop, recovers from custodians
  alone on a fresh machine, and checks hashes, names, deleted-file history,
  suggestion content and every signature against the roster at its parents.
  It also checks a recovery sheet's roster commit and the plain git-annex
  export.
