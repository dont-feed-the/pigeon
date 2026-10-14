# 6. Status

> Mission: define how a device computes each version's state from what it has
> observed, with the time of the observation (P6), and how everything reduces to
> one headline with a reason and a next step (ADR 0027). All of it is pure core
> code over observations; nothing here is stored (ADR 0036).

## 6.1 Observations

A device knows only what it has observed. Each observation carries the time at
which it was true.

| Observation | Source | Time |
|---|---|---|
| `has_commit(d, c)`: device *d* has commit *c* | *c* is, or precedes, the `main` or a `proposals` tip of *d*'s status report (§6.3), or a ref *d* advertised to this device's own fetch | the report's `at`, or when fetched |
| `holds(c, k)`: custodian *c* holds key *k* | git-annex location log (`whereis --json`), merged from every peer | the log entry's timestamp |
| `verified(c, k)` | content is verified on arrival (`annex.verify=true`); a scrub that finds it bad moves it out and logs it absent | the last completed scrub pass in *c*'s report |
| `heard(d)`: last news from device *d* | a connection, a fetch of *d*'s refs, or a newer report of *d* relayed by anyone | the latest of these |
| local queues | this device's capture, install and transfer queues | now |

Observations only grow, and each (device, key) pair keeps its latest
observation: a join-semilattice (ADR 0012).

## 6.2 Version states

For a version with key *k*, introduced by commit *c*, on this device, at time *now*:

1. Let **H** be the custodians *x* of the roster with `holds(x, k)` and
   `has_commit(x, c)` that were heard of within `offline_after` (default 24
   hours **[no ADR]**) and do not report `checking` or a lost repository
   (ADR 0046). A custodian silent for
   longer no longer counts, and its silence becomes the reason. Content
   without the commit that names it is not protected: lost with the laptop,
   it would be a key without a name (ADR 0046).
2. If H holds at least K distinct custodians, each counted once by its device
   (ADR 0046): the version is **protected**, *observed at* T, the K-th most
   recent of their latest `heard(x)` ("Safe. Checked 3 hours ago").
3. If this device has heard of no custodian within `stale_after` (default 1 hour
   **[no ADR]**), a version that would be protected is **uncertain**, "not checked
   since T": pigeon cannot reach the group, so it cannot confirm.
4. Else, if some other device has *c* (`has_commit`) or some custodian holds *k*:
   **published**, with its reason (§6.4).
5. Else, if this device holds *k*: **local**.
6. Else: **missing**, with reason `content-missing`: no device is known to hold
   the content; or `erased`, when a purge released it ([08 §8.6](08-custody-loops.md)).

These states apply to suggestions as to accepted versions: a proposal commit
is *c*. A version is called safe only when *protected*. On the laptop that
captured it, a version stays an immutable copy until then
([07 §7.8](07-capture-and-install.md), [07 §7.8](07-capture-and-install.md)).

A version under capture (not yet in a commit) counts as *local*; a held-back
change (mass deletion, non-portable name) counts as *local* with its reason.
Laptops are untrusted (ADR 0002), so they never enter H.

Laws (`pigeon-core` `state`, 13 §13.13): the state is monotone in presence
observations, except through time (staleness), and depends only on its key's
observations and the commit's spread, not on paths.

## 6.3 Status reports **[no ADR]**

Each device publishes its own view of itself as `refs/pigeon/status/<device-id>`:
a parentless commit signed by the device, replaced at each publication, holding
`status.toml`. It answers what git and git-annex do not record: when a device was
last heard of, its free space, its last scrub, its queue.

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

A custodian refuses a repository on `tmpfs` or `ramfs`. Before each report
and each served session it checks that its repository is still there with its
UUID; if not, it reports `repository = "lost"` and stops serving.

A device publishes when its state changes and at least every 10 minutes while
it runs. A report is accepted only if its signer is the device it names, listed
in the roster of this device's `main`, if its `format` and every field are
known (a report this release cannot read is ignored, which errs toward
keeping, ADR 0048), and if its `at` is later than the last
accepted report of that device; reports are relayed by every peer, so news of a
custodian reaches a laptop through any device.

## 6.4 Reasons and next steps

Every version that is not *protected* has one **reason**, computed with the
state, so that the headline can say why and what happens next (ADR 0027):

| Reason | Example sentence (en) |
|---|---|
| `sending(eta)` | "Sending 12 photos to the group… (2 min left)" |
| `custodian-offline(names, since)` | "Paul's NAS has been offline since Monday 18:02; they will be safe when it is back." |
| `no-capacity(missing_bytes, machines, who)` | "The backup machines are full: 40 GB are missing. Paul can free space on Paul's NAS, or an admin can add a backup machine." (ADR 0003, [08 §8.9](08-custody-loops.md)) |
| `repository-lost(device)` | "Paul's NAS no longer finds its backup folder; it counts for nothing until it is set up again." (ADR 0046) |
| `disk-full(device)` | "Paul's NAS is full; it waits until 3 GB are freed." ([08 §8.9](08-custody-loops.md)) |
| `too-few-custodians(have, k)` | "The group keeps 2 safe copies but has only 1 backup machine." |
| `paused(by)` | "Sharing is paused until 18:00." / "Waiting for a non-metered network." / "Waiting: this computer's disk is busy." (ADR 0037) |
| `group-unreachable(since)` | "pigeon can't reach the group since 14:10." |
| `held(cause)` | "You deleted 1,204 files in *Photos*: delete them for everyone, or bring them back?" |
| `history-full(bytes)` | "This computer keeps 18 GB of changes that no backup machine has yet; new changes wait until some are sent." ([07 §7.8](07-capture-and-install.md), [07 §7.8](07-capture-and-install.md)) |
| `content-missing` | "This version of *budget.xlsx* is lost: no computer has its content any more." |
| `erased(decision)` | "This version of *scan-12.pdf* was erased on 12 November 2026 (Roy's decision)." ([08 §8.6](08-custody-loops.md)) |
| `in-use(path)` | "*tarte.docx* is open in another application; pigeon will update it when it is closed." ([07 §7.5](07-capture-and-install.md), [07 §7.5](07-capture-and-install.md)) |
| `settings-check` | "Checking the group's settings; freeing space waits until it is done." ([03 §3.5](03-repository-and-roles.md)) |

The custodians that *would* hold a key are those that `sizebalanced` assigns it
to; pigeon asks git-annex which ones want it (`find --want-get` evaluated for each
custodian, through `whereis`/`find --json`) and names them **[no ADR: method]**.

## 6.5 The headline

The headline reduces every item to one of five levels (ADR 0027, ux.md §4):

| Level | Shown when |
|---|---|
| **Needs you** | an item only a person can fix: disk full; a backup machine not heard of for more than 7 days (for admins and its owner); a held mass deletion; a held non-portable name; a conflict waiting for this person (ADR 0034); a suggestion waiting for this person's decision for more than 3 days; this computer removed from the group; custody capacity short (ADR 0003, [08 §8.9](08-custody-loops.md), for admins and custodian owners); a backup machine whose repository is lost (ADR 0046, for admins and its owner); erased content came back (for the deciders, [08 §8.6](08-custody-loops.md)) |
| **Not safe yet** | some version is *local* or *published* for longer than the grace period (default 10 minutes) |
| **Not checked recently** | pigeon cannot confirm: no custodian heard of within `stale_after` |
| **Working** | something is being captured, sent or received, all of it younger than the grace period |
| **Safe** | every version is *protected*, observed within `stale_after`; if some items of the folder are not shared, a second sentence says so: "2 items of your folder aren't shared" ([07 §7.9](07-capture-and-install.md)) |

**Precedence:** Needs you, then Not safe yet, then Not checked recently, then
Working, then Safe; the first that applies wins. A *protected* version whose
time T is older than `stale_after` counts as *Not checked recently*; a
*missing* version (`content-missing`) as *Not safe yet* after the grace
period; an *erased* one not at all: it was released on purpose (13 §13.13). ADR 0027 lists the levels as
Needs you, Not safe yet, Working, Safe, Not checked recently; this specification
reads *Not checked recently* as the honest form of *Working* or *Safe* when
pigeon cannot confirm, never as better than them **[no ADR]**. A version known to
be only on this computer is always *Not safe yet* after the grace period, even
when the group is unreachable, so nobody reads grey as safe (ux.md §9 pass
criterion).

Within a level, the sentence names the most important case and counts the rest:
"3 files are not safe yet. Paul's NAS has been offline since Monday 18:02."
A headline is at most two sentences: what, then why and what next, with the time
of the last check ("Checked 4 minutes ago"). The owner of a missing machine and
the admins see the same fact from their own devices, so "Paul has been told" means
that Paul's pigeon shows it to him too.

Laws: the headline is a join (maximum in the precedence order) over items, so it
is commutative, associative and idempotent; adding an item never lowers the level.

## 6.6 Folders, files and times

- Items pigeon does not share (links, nested repositories, live libraries,
  unreadable paths; [07 §7.9](07-capture-and-install.md)) have no state: they
  are never *protected*. The folder row adds "2 items not shared", the
  *Not shared* list and `pigeon status --not-shared` name them, and one
  notification tells of each when it first appears ([07 §7.9](07-capture-and-install.md)).
- A folder row aggregates its files: "All safe", "3 not safe yet", "Not checked
  since …"; a folder not kept on this computer shows its state all the same.
- A file's page shows, per version, who, when and the state; *Details* lists
  which backup machines hold it and when each was last heard of (ux.md §6).
- Times: relative under a day ("4 minutes ago"), absolute beyond ("Monday 18:02"),
  in local time with `jiff`, rendered by Fluent (ADR 0028); `--json` carries RFC
  3339 times and the internal state names.

## 6.7 Thresholds

| Threshold | Default | Source |
|---|---|---|
| grace period before *Not safe yet* | 10 minutes | ADR 0027 |
| notification for files not safe | 24 hours | ADR 0029 |
| backup machine missing → *Needs you* | 7 days | ux.md §4 |
| suggestion waiting → *Needs you* | 3 days | ux.md §4 |
| mass deletion | more than 100 files, or a whole top-level folder | ux.md §8.6 |
| `stale_after`: no custodian heard of → *uncertain* | 1 hour | **[no ADR]** |
| `offline_after`: a silent custodian stops counting | 24 hours | **[no ADR]** |
| status report period | 10 minutes | **[no ADR]** |

All are settings with tested defaults, to be tuned by the usability test
(ux.md §11).
