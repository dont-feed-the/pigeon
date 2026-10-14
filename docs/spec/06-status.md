# 6. Status

> Mission: each version's state from timed observations (P6), and one
> headline with a reason and a next step (ADR 0027); computed, never stored
> (ADR 0036). Normative: `pigeon-core::state` (§6.2) and
> `pigeon-core::headline` (§6.5), with their laws.

## 6.1 Observations

Each observation carries the time at which it was true (`pigeon-core::time`).

| Observation | Source | Time |
|---|---|---|
| `has_commit(d, c)` | *c* is or precedes the `main` or a `proposals` tip of *d*'s status report (§6.3), or a ref *d* advertised to this device's fetch | the report's `at`, or when fetched |
| `holds(x, k)`: custodian *x* holds key *k* | the git-annex location log, merged from every peer | the log entry's time |
| `verified(x, k)` | verified on arrival (`annex.verify=true`); a scrub that finds it bad logs it absent | *x*'s last completed scrub |
| `heard(d)` | a connection, a fetch of *d*'s refs, or a newer report of *d* relayed by anyone | the latest |
| local queues | capture, install and transfer | now |

Observations only grow; each (device, key) keeps its latest (ADR 0012).

## 6.2 Version states

For key *k* introduced by commit *c*, at *now* (`state`, ADR 0046), the first
rule that holds:

| # | Rule | State |
|---|---|---|
| 1 | **H**: roster custodians with `holds(x, k)` and `has_commit(x, c)`, heard within `offline_after` (24 h **[no ADR]**), not `checking` nor lost; content without its commit does not count | — |
| 2 | \|H\| ≥ K, each custodian counted once by its device; observed at the K-th most recent `heard(x)` ("Safe. Checked 3 hours ago") | **protected** |
| 3 | no custodian heard within `stale_after` (1 h **[no ADR]**), for a version that would be protected | **uncertain**, "not checked since T" |
| 4 | another device has *c*, or a custodian holds *k* | **published**, with its reason (§6.4) |
| 5 | this device holds *k* | **local** |
| 6 | otherwise | **missing**: `content-missing`, or `erased` after a purge ([08 §8.6](08-custody-loops.md)) |

Proposals have states like accepted versions. Only *protected* is called
safe; until then the laptop keeps an immutable copy ([07 §7.8](07-capture-and-install.md)).
A version under capture or held back (mass deletion, non-portable name) is
*local* with its reason. Laptops are untrusted (ADR 0002), never in H.
Laws (`tests/state.rs`): `more_presence_never_lowers_the_state`,
`a_second_entry_for_one_device_changes_nothing`,
`custodian_order_does_not_matter`, `protected_when_k_distinct_custodians_count`,
`protected_is_observed_within_offline_after`, `a_new_holder_never_lowers_the_state`.

## 6.3 Status reports **[no ADR]**

Each device publishes `refs/pigeon/status/<device-id>`: a parentless commit
it signs, replaced at each publication, holding `status.toml`:

```toml
format = 1
device = "b71e…"
at = 2026-10-13T12:02:00Z          # RFC 3339, UTC
main = "<commit id of its main>"
proposals = ["<commit id>"]         # tips of the proposal chains it keeps outside main (ADR 0046)
checking = false                    # custodians: true while a restored repository is re-checked (ADR 0046)
versions = { pigeon = "1.0.0", git_annex = "10.20261006", git = "2.55.0" }

[disk]                              # bytes
used = 812_000_000_000
free = 1_100_000_000_000
maxsize = 2_000_000_000_000
reserve = 22_000_000_000            # annex.diskreserve (08 §8.9)
full = false                        # true while writes are paused by ENOSPC (08 §8.9)

[custody]                           # custodians only (ADR 0046)
repository = "ok"                   # "lost": its repository, UUID or storage is gone; counts for nothing

[scrub]                             # custodians only
last_pass_completed = 2026-10-12T05:40:00Z
keys_failed = 0

[[purge]]                           # custodians only: seals (08 §8.6)
decision = "purge-7f3a"
main = "<commit id of its main when it sealed>"
held = []                           # covered keys it keeps for a reference outside main

[queue]
bytes_to_send = 0
bytes_to_receive = 0
paused = ""                         # "", "person", "battery", "metered", "disk-busy" (ADR 0037)
admits = "all"                      # custodians: "endangered" while they keep room for repair (08 §8.9)
```

Not yet coded. A custodian refuses a repository on `tmpfs` or `ramfs`, and
before each report and served session checks its repository and UUID; if
lost, it reports `repository = "lost"` and stops serving. A device publishes
on each change and at least every 10 minutes. A report counts only if signed
by the roster device it names, with a known `format` and fields (else
ignored, ADR 0048), and an `at` later than the last; every peer relays it.

## 6.4 Reasons and next steps

Every version not *protected* has one **reason** (ADR 0027):

| Reason | Example sentence (en) |
|---|---|
| `sending(eta)` | "Sending 12 photos to the group… (2 min left)" |
| `custodian-offline(names, since)` | "Paul's NAS has been offline since Monday 18:02; they will be safe when it is back." |
| `no-capacity(missing_bytes, machines, who)` | "The backup machines are full: 40 GB are missing. Paul can free space on Paul's NAS, or an admin can add a backup machine." ([08 §8.9](08-custody-loops.md)) |
| `repository-lost(device)` | "Paul's NAS no longer finds its backup folder; it counts for nothing until it is set up again." |
| `disk-full(device)` | "Paul's NAS is full; it waits until 3 GB are freed." |
| `too-few-custodians(have, k)` | "The group keeps 2 safe copies but has only 1 backup machine." |
| `paused(by)` | "Sharing is paused until 18:00." / "Waiting for a non-metered network." (ADR 0037) |
| `group-unreachable(since)` | "pigeon can't reach the group since 14:10." |
| `held(cause)` | "You deleted 1,204 files in *Photos*: delete them for everyone, or bring them back?" |
| `history-full(bytes)` | "This computer keeps 18 GB of changes that no backup machine has yet; new changes wait." ([07 §7.8](07-capture-and-install.md)) |
| `content-missing` | "This version of *budget.xlsx* is lost: no computer has its content any more." |
| `erased(decision)` | "This version of *scan-12.pdf* was erased on 12 November 2026 (Roy's decision)." |
| `in-use(path)` | "*tarte.docx* is open in another application; pigeon will update it when it is closed." ([07 §7.5](07-capture-and-install.md)) |
| `settings-check` | "Checking the group's settings; freeing space waits until it is done." ([03 §3.5](03-repository-and-roles.md)) |

The custodians that *would* hold a key are those `sizebalanced` assigns it
to, as git-annex's `find --want-get` says for each **[no ADR: method]**.

## 6.5 The headline

Five levels, the headline being their join (`headline`, ADR 0027, ux.md §4):

| Level | Shown when |
|---|---|
| **Needs you** | only a person can fix it: disk full; a backup machine silent for over 7 days (for admins and its owner); a held mass deletion or name; a conflict for this person (ADR 0034); a suggestion waiting over 3 days for this person; this computer removed; capacity short (admins and custodian owners, [08 §8.9](08-custody-loops.md)); a lost repository (admins and its owner); erased content came back (deciders, [08 §8.6](08-custody-loops.md)) |
| **Not safe yet** | a version *local* or *published* for longer than the grace period (default 10 min); a *missing* one after it |
| **Not checked recently** | no custodian heard within `stale_after`; a *protected* version older than it |
| **Working** | capture or transfer under way, all younger than the grace period |
| **Safe** | every version *protected* within `stale_after`; "2 items of your folder aren't shared" when so ([07 §7.9](07-capture-and-install.md)) |

Precedence is the table's order; *Not checked recently* is never better than
*Working* or *Safe*, unlike ADR 0027 **[no ADR]** (16 §16.2); an *erased*
version counts for nothing. Laws (`tests/headline.rs`):
`join_is_a_semilattice` (*Safe* the identity, *Needs you* absorbing),
`headline_of_a_union_is_the_join`, `adding_an_item_never_lowers_it`,
`only_here_is_not_safe_yet_after_grace_even_unreachable` (ux.md §9).

The sentence names the most important case and counts the rest, in at most
two sentences (what; why and what next) with the time of the last check; the
owner of a missing machine and the admins see the same fact.

## 6.6 Folders, files and times

- Items not shared ([07 §7.9](07-capture-and-install.md)) have no state; the
  folder row, the *Not shared* list and `pigeon status --not-shared` name
  them, and one notification tells of each when it first appears.
- A folder row aggregates its files; a folder not kept here shows its state.
- A file's page shows each version's author, time and state; *Details* lists
  the backup machines holding it and when each was last heard (ux.md §6).
- Times are relative under a day, absolute beyond, local, via `jiff` and
  Fluent (ADR 0028); `--json` carries RFC 3339 times and internal names.

## 6.7 Thresholds

| Threshold | Default | Source |
|---|---|---|
| grace period before *Not safe yet* | 10 minutes | ADR 0027 |
| notification for files not safe | 24 hours | ADR 0029 |
| backup machine missing → *Needs you* | 7 days | ux.md §4 |
| suggestion waiting → *Needs you* | 3 days | ux.md §4 |
| mass deletion | more than 100 files, or a whole top-level folder | ux.md §8.6 |
| `stale_after` → *uncertain* | 1 hour | **[no ADR]** |
| `offline_after`: a silent custodian stops counting | 24 hours | **[no ADR]** |
| status report period | 10 minutes | **[no ADR]** |

All are settings with tested defaults for the usability test (ux.md §11).
