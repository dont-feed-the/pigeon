# 1. Concepts

> Mission: define every term the other files use, once. People never see most of
> these words; the right-hand column says what they see instead (ux.md §2.2, ADR 0027).

## 1.1 The group and its members

| Term | Definition | People see |
|---|---|---|
| **group** | the people who share one folder tree, identified by the hash of its genesis commit (*group ID*) | the group's name ("Famille Geiger") |
| **person** | a member of the group, with a short immutable ID (`paul`) and a display name | the name |
| **device** | one installation of pigeon on one machine, identified by its ed25519 public key, the *device ID* (ADR 0007) | "Paul's laptop", "Paul's NAS" |
| **roster** | the signed list of persons, devices, roles, folder rules and group settings, stored in `.pigeon/roster.toml` (ADR 0009) | "members" |
| **admin** | a person who may change the roster | admin |
| **founder** | a person who signed the genesis commit; a founder is an admin at genesis | — |

A person has one or more devices. Roles that concern decisions (admin, owner,
steward) belong to **persons**; roles that concern storage (laptop, custodian)
belong to **devices** (ADR 0007).

## 1.2 Device roles

| Role | What it does | Counts toward K | People see |
|---|---|---|---|
| **laptop** | a person's computer: holds editable *working copies* of the folders the person keeps, at about one copy of disk space once protected, plus its pending history (ADR 0041) | no | computer |
| **custodian** | a machine dedicated to storage, Linux only in v1, holding read-only *custody copies* of what its capacity allows, history included (ADRs 0002, 0003, 0032) | yes | backup machine |

"Laptop" means any person's computer, desktop included. A device has exactly
one role. **K** is the group's number of custody copies, default 2 (ADR 0002);
people see "number of safe copies".

## 1.3 Folders and their rules

The shared tree has **top-level folders** (`Photos/`, `Paul/`, `Lab data/`). Each
path of the tree is governed by exactly one **folder rule** of the roster
([04 §4.3](04-roster-and-authority.md)):

| Rule | Who may add a file | Who may change or delete an existing file | People see |
|---|---|---|---|
| **owned** by person P | P | P | "Paul's folder — only Paul can change it" |
| **add-only** with stewards S | every member | the stewards S | "everyone can add; Roy decides changes" |

A change that the rule does not allow becomes a **proposal** (people see
*suggestion*). An **owner** or **steward** *decides* a proposal: **accept** or
**reject** (people see *accept* or *decline*).

## 1.4 Versions and the catalogue

| Term | Definition |
|---|---|
| **commit** | a signed git commit of the shared branch `main`; its tree is a state of the shared folder |
| **key** | git-annex's content address of a file's content (backend `SHA256E`) |
| **version** | a pair (path, key) introduced by an accepted commit; the *versions of a file* are the successive keys of one path |
| **accepted catalogue** | the set of commits that are valid under the authority rule (P4), with the tree of their merge |
| **logical catalogue** | the paths, versions and states that the accepted catalogue defines, identical on every device that has the same accepted history (P5) |
| **physical tree** | the files actually on one device's disk: the logical catalogue restricted to the folders kept there, plus unresolved divergences |
| **proposal** | a signed commit that the authority rule does not accept, kept under `refs/pigeon/proposals/<device-id>/<commit>` (ADR 0009) |
| **decision** | a signed commit that accepts a proposal (a merge) or rejects it (`.pigeon/decisions/<commit>.toml`) |
| **divergence** | a path whose file on this device's disk differs from the catalogue's version and from what pigeon last installed there, because a person saved it (ADR 0034) |
| **conflict** | a divergence that needs the person: their suggestion was declined, or the group's version changed under it; people see "your version differs from the group's" |
| **concurrent edit** | two accepted versions of one path made without knowledge of each other; both are kept, under deterministic names ([05 §5.3](05-catalogue-and-names.md)); people see "edited at the same time; two versions kept" |

## 1.5 Version states

A version goes through three states, computed per device from what that device
has observed ([06](06-status.md)):

| State | Definition | People see |
|---|---|---|
| **local** | captured on this device; no other device is known to have its commit | Only on this computer |
| **published** | another device is known to have its commit, but fewer than K custodians are known to hold verified copies | Shared, not safe yet |
| **protected** | at least K distinct custodians hold verified copies, as observed at time T | Safe |
| **uncertain** | the last observation is too old to say (P6) | Not checked since … |

## 1.6 Three removals, never conflated

| Removal | What it is | Effect on safety | People see |
|---|---|---|---|
| **unlink** | deleting a path: a new commit without the file | none: custodians keep every version (ADR 0018) | delete (can be undone) |
| **evict** | removing this device's working copy of protected versions | none: allowed only for protected versions (ADR 0031) | free up space on this computer |
| **release** | a **purge**: a signed decision that releases the **references** it covers; a key goes once no other reference holds it and every custodian has **sealed** the decision after the **retention period** (ADR 0042) | the covered versions are gone from every device of the group; copies outside it and names in history stay | erase forever; waiting period |

## 1.7 The two escapes

Every unsafe action is one of two primitives, both signed, flagged and recorded
(proposal §2):

| Escape | What it does | People see |
|---|---|---|
| **Force** | an admin commit that the folder rules would turn into a proposal, accepted because it is marked forced ([04 §4.6](04-roster-and-authority.md)) | admin override |
| **Drop** | removing a copy without the verified-copy check of git-annex (`--force`), outside a purge | remove without checking (CLI only) |

## 1.8 Software parts

| Part | Definition |
|---|---|
| **`pigeond`** | the one daemon per user that runs everything: watchers, loops, the iroh endpoint, git and git-annex processes, the web interface (ADR 0011) |
| **`pigeon`** | the CLI, a thin client of `pigeond` (ADRs 0011, 0024) |
| **`git-annex-p2p-iroh`** | the transport helper that git-annex calls, a thin client of `pigeond` (ADR 0006) |
| **core** | the pure functions of pigeon (authority, catalogue, names, states, headline), with laws (ADR 0012) |
| **shell** | the code that performs I/O: git, git-annex, the file system, the network, behind traits (ADR 0012) |
| **loop** | one of the protocols `pigeond` runs: capture, accept, place and repair, free and keep, purge, install |
| **reference** | a path at which a commit introduces a key: an add, edit, restore, rename or conflict copy; a purge releases references, not keys (ADR 0042) |
| **seal** | a custodian's statement, in its status report, that a purge's waiting period has ended for it, with the `main` it had then (ADR 0042) |

## 1.9 Words people never see

git, annex, commit, merge, branch, remote, key, hash, numcopies, trust, wanted,
sync, pointer file and ref appear only in diagnostics (`pigeon doctor --details`)
and logs (ux.md §2.2). Every user-facing word comes from the Fluent catalogue
(ADR 0028), which follows the vocabulary table of ux.md §2.2.
