# 1. Concepts

> Mission: every term, once, with what people see instead (ux.md §2.2).

## 1.1 The group and its members

| Term | Definition | People see |
|---|---|---|
| **group** | the people who share one folder tree, identified by the hash of its genesis commit (*group ID*) | the group's name |
| **person** | a member, with a short immutable ID (`paul`) and a display name | the name |
| **device** | one installation of pigeon, identified by its ed25519 public key, the *device ID* (ADR 0007) | "Paul's laptop" |
| **roster** | the signed list of persons, devices, roles, folder rules and settings, `.pigeon/roster.toml` (ADR 0009) | "members" |
| **admin** | a person who may change the roster; every **founder** (signer of the genesis commit) is one at genesis | admin |

Decision roles (admin, owner, steward) belong to persons; storage roles
(laptop, custodian) belong to devices (ADR 0007).

## 1.2 Device roles

| Role | What it does | Counts toward K | People see |
|---|---|---|---|
| **laptop** | a person's computer (desktops too): editable *working copies* of the folders the person keeps, plus pending history ([07 §7.8](07-capture-and-install.md)) | no | computer |
| **custodian** | a storage machine, Linux only in v1, holding read-only *custody copies* and history (ADRs 0002, 0003, 0032) | yes | backup machine |

A device has one role. **K**, the number of custody copies, is 2 by default
(ADR 0002); people see "number of safe copies".

## 1.3 Folders and their rules

Each path has exactly one **folder rule** ([04 §4.3](04-roster-and-authority.md)):

| Rule | Who may add | Who may change or delete | People see |
|---|---|---|---|
| **owned** by person P | P | P | "only Paul can change it" |
| **add-only** with stewards S | every member | S | "everyone can add; Roy decides changes" |

A change the rule does not allow is a **proposal** (*suggestion*), which an
owner or steward **accepts** or **rejects** (*accept*, *decline*).

## 1.4 Versions and the catalogue

| Term | Definition |
|---|---|
| **commit** | a signed git commit of the shared branch `main` |
| **key** | git-annex's content address of a file (backend `SHA256E`) |
| **version** | a pair (path, key) introduced by an accepted commit |
| **reference** | a path at which a commit introduces a key (add, edit, restore, rename, conflict copy); a purge releases references, not keys ([08 §8.6](08-custody-loops.md)) |
| **accepted catalogue** | the commits valid under the authority rule (P4), with the tree of their merge |
| **logical catalogue** | the paths, versions and states the accepted catalogue defines, the same on every device with the same accepted history (P5) |
| **physical tree** | the files on one device's disk: the logical catalogue for the folders kept there, plus unresolved divergences |
| **proposal** | a signed commit the authority rule does not accept, under `refs/pigeon/proposals/<device-id>/<commit>` (ADR 0009) |
| **decision** | a signed commit that accepts a proposal (a merge) or rejects it (`.pigeon/decisions/<commit>.toml`) |
| **divergence** | a path whose file on disk differs from the catalogue's version and from what pigeon last installed, because a person saved it (ADR 0034) |
| **conflict** | a divergence that needs the person; people see "your version differs from the group's" |
| **concurrent edit** | two accepted versions of one path made without knowledge of each other; both are kept under deterministic names ([05 §5.3](05-catalogue-and-names.md)) |

## 1.5 Version states

Per device, from its own observations ([06 §6.2](06-status.md), `pigeon-core::state`):

| State | Definition | People see |
|---|---|---|
| **local** | no other device is known to have its commit | Only on this computer |
| **published** | another device has its commit; fewer than K custodians are known to hold it | Shared, not safe yet |
| **protected** | K distinct custodians, counted once per device, hold the content and the commit that names it, on disk (ADR 0046) | Safe |
| **uncertain** | the last observation is too old to say (P6) | Not checked since … |

## 1.6 Three removals, never conflated

| Removal | What it is | Effect on safety | People see |
|---|---|---|---|
| **unlink** | a new commit without the path | none: custodians keep every version (ADR 0018) | delete (can be undone) |
| **evict** | removing this device's working copy of protected versions (ADR 0031) | none | free up space |
| **release** | a **purge**: a signed decision releasing references; a key goes once nothing else references it and every custodian has **sealed** it after the retention period ([08 §8.6](08-custody-loops.md)) | gone from every device of the group | erase forever |

## 1.7 The two escapes

Every unsafe action is one of two primitives, signed, flagged and recorded:

| Escape | What it does | People see |
|---|---|---|
| **Force** | an admin commit accepted although a folder rule would make it a proposal ([04 §4.6](04-roster-and-authority.md)) | admin override |
| **Drop** | removing a copy without git-annex's verified-copy check, outside a purge | remove without checking (CLI only) |

## 1.8 Software parts

| Part | Definition |
|---|---|
| **`pigeond`** | the one daemon per user: watchers, loops, the iroh endpoint, git and git-annex processes, the web interface (ADR 0011) |
| **`pigeon`** | the CLI, a thin client of `pigeond` (ADRs 0011, 0024) |
| **`git-annex-p2p-iroh`** | the transport helper git-annex calls, a thin client of `pigeond` (ADR 0006) |
| **core** | `pigeon-core`: pure algebras with laws, without I/O (ADR 0012); [13 §13.13](13-verification.md) lists them |
| **shell** | the code that does I/O behind traits: git, git-annex, the file system, the network |
| **loop** | a protocol of `pigeond`: capture, accept, place and repair, free and keep, purge, install |
| **seal** | a custodian's statement that a purge's waiting period has ended for it ([08 §8.6](08-custody-loops.md)) |

## 1.9 Words people never see

git, annex, commit, merge, branch, remote, key, hash, numcopies, trust, wanted,
sync, pointer file and ref appear only in `pigeon doctor --details` and logs.
Every user-facing word comes from the Fluent catalogue (ADR 0028).
