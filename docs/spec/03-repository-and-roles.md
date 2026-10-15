# 3. Repositories and roles

> Mission: what git and git-annex hold on each device, how each role is
> configured, and which commands pigeon may run. git-annex is the engine
> (ADR 0001), driven through its CLI (ADR 0004); git through its CLI only
> (ADR 0010).

## 3.1 One repository per group and device

| Role | Repository | Where (default) |
|---|---|---|
| laptop | non-bare; the working tree *is* the shared folder | chosen at join, e.g. `~/Famille Geiger` |
| custodian | bare: objects only, nothing exposed to applications (ADR 0002) **[no ADR]** | `/var/lib/pigeon/<group-id>.git` |

E9 and E10 ran custodians non-bare with locked files; the bare layout must
pass the same experiments before release ([16](16-open-questions.md)).

## 3.2 Refs

| Ref | Holds | Written by |
|---|---|---|
| `refs/heads/main` | the accepted catalogue, signed commits | this device's accept and capture loops only |
| `refs/heads/git-annex` | location logs, trust, groups, preferred content, metadata | git-annex |
| `refs/pigeon/proposals/<device-id>/<commit>` | one ref per proposal (ADR 0009) | the proposing device; fetched |
| `refs/pigeon/peers/<device-id>/main` | each peer's last fetched `main`, before validation **[no ADR]** | `git fetch` |
| `refs/remotes/<remote>/git-annex` | the peer's git-annex branch, for `git annex merge` | `git fetch` |
| `refs/pigeon/archive/<group-id>/main` | the history of the group this one was refounded from, read-only (ADR 0047) | the refounding device; fetched |
| `refs/pigeon/status/<device-id>` | each device's latest signed status report ([06 §6.3](06-status.md)) **[no ADR]** | its device; relayed by every peer that counted it |
| `refs/pigeon/reports/<peer-id>/<device-id>` | the reports last fetched from a peer, before they count **[no ADR]** | `git fetch` |

`main` only moves forward: `update-ref` with the expected old value, to a
descendant of it.

## 3.3 What is annexed

`.gitattributes`, written in the genesis commit, then `.git/info/attributes`,
which git ranks above any nested `.gitattributes` copied into the folder:

```gitattributes
* annex.largefiles=anything
.gitattributes annex.largefiles=nothing
.pigeon/** annex.largefiles=nothing
```

```gitattributes
* filter=annex annex.largefiles=anything annex.backend=SHA256E -text -eol -ident -working-tree-encoding
/.gitattributes annex.largefiles=nothing
/.pigeon/** annex.largefiles=nothing
```

Every file is annexed (`SHA256E`), dotfiles included; pigeon's own files are
plain blobs, covered by commit signatures (ADR 0009). Only regular files and
their execute bit are shared ([07 §7.9](07-capture-and-install.md)).

## 3.4 Configuration by role

### Laptop (ADR 0002)

| Setting | Value | Why |
|---|---|---|
| `annex.addunlocked` | `true` | normal, editable files |
| `annex.thin` | `true`; capture adds with `-c annex.thin=false` | received files cost one copy (E8); a captured version stays an immutable copy until protected, then is re-thinned (E18, [07 §7.8](07-capture-and-install.md)) |
| `annex.autocommit`, `annex.synccontent`, `annex.resolvemerge` | `false` | pigeon alone commits, moves content and merges (E9) |
| `annex.dotfiles`, `annex.backend` | `true`, `SHA256E` | every file annexed; verifiable keys only ([07 §7.9](07-capture-and-install.md)) |
| `core.attributesFile`, `core.excludesFile` | an empty file of pigeon's | the person's git attributes and excludes do not apply (§3.7) |
| `annex.jobs` | `1` | background work stays light (ADR 0037) |
| `annex.diskreserve` | max(1 GB, 1 % of the file system) | git-annex refuses downloads that cross it; so does capture ([08 §8.9](08-custody-loops.md)) |
| trust, groups | `untrust`, `laptop` | its copies never count toward K |
| `wanted` | `include=<folder>/* or …` over the kept top-level folders | selective sync ([08 §8.5](08-custody-loops.md)) |
| sparse checkout | the kept folders and `/.pigeon/` (E15) | folders not kept are absent (ADR 0031) |
| `core.fsmonitor`, `core.precomposeunicode` | `true` on macOS | faster scans (ADR 0013); NFC names ([05 §5.4](05-catalogue-and-names.md)) |
| `gpg.format`, `user.signingkey` | `ssh`, the device key file | signed commits (ADR 0007) |
| `core.fsync`, `core.fsyncMethod` | `objects,reference`, `fsync` | commits and refs reach the disk (ADR 0046) |

### Custodian (ADRs 0002, 0003, 0014 and 0018, [08 §8.6](08-custody-loops.md))

| Setting | Value | Why |
|---|---|---|
| repository | bare (§3.1) | never exposed to applications |
| trust, groups | `semitrust`, never `trust`; `custodian` | every drop verifies it; `sizebalanced` and status count the group |
| `wanted` | `sizebalanced=custodian:K` | balanced by capacity; `pigeond` applies it key by key to the whole history (E9), never to released keys ([08 §8.3](08-custody-loops.md), [08 §8.6](08-custody-loops.md)) |
| `maxsize` | from the device's roster entry | a share for `sizebalanced` (ADR 0003), never read as free space ([08 §8.9](08-custody-loops.md)) |
| `annex.diskreserve` | max(5 GB, 2 % of the file system) | room for git, the annex journal, databases and a recovery `fsck`; git-annex refuses transfers that cross it ([08 §8.9](08-custody-loops.md), E22) |
| `annex.jobs` | `cpus` | a dedicated machine |
| `core.fsync`, `core.fsyncMethod` | `objects,reference`, `fsync`; objects synced by `pigeond` before any positive answer ([09 §9.4](09-network-and-joining.md)) | what counts toward K is on disk (ADR 0046) |
| `annex.hardlink` | `false` | repositories share no inode or extents; `pigeond` refuses to serve one whose `annex/objects` is on another file system (ADR 0046, E22) |
| serving | append-only: no remote `REMOVE` (ADR 0014) | [09 §9.4](09-network-and-joining.md) |

### Group-wide (git-annex branch)

`numcopies = mincopies = K` (ADR 0002). A device removed from the roster is
marked `dead`, so its copies stop counting and repair starts
([08 §8.4](08-custody-loops.md)).

## 3.5 Settings follow the roster, or drops stop

git-annex's shared settings (each device's trust, groups, `wanted`,
`maxsize`; `numcopies`, `mincopies`) are a function of the roster, which
maps device IDs to annex UUIDs ([04 §4.2](04-roster-and-authority.md)).
After every merge of the git-annex branch and change of `main`, `pigeond`:

| # | Step |
|---|---|
| 1 | computes the expected settings from the roster of its `main` |
| 2 | reads the actual ones (`git annex info --json`, `wanted`, `group`, `trust`) |
| 3 | if they differ, **suspends every drop** (evict, purge, rebalancing) and re-applies them **[no ADR: re-apply]**; a missing `pre-receive` hook or a set `core.hooksPath` also suspends serving ([09 §9.4](09-network-and-joining.md)); a difference in `.git/info/attributes`, `annex.backend`, `core.attributesFile`, `core.excludesFile` or `core.fsync` (§3.3–3.4), or an `include` in `.git/config`, also suspends capture |
| 4 | resumes only when a fresh read matches; status shows "checking the group's settings" meanwhile (P6) |

## 3.6 Remotes

Every other device of the roster is a remote:

```ini
[remote "pigeon-3f9a0c1d2e4b"]          ; "pigeon-" + first 12 hex digits of the device ID [no ADR]
    url = p2p-annex::iroh:<device-id>   ; git-annex's generic P2P transport (ADR 0006, 09 §9.2)
    annex-uuid = <the device's git-annex UUID, from the roster>
    fetch = +refs/heads/main:refs/pigeon/peers/<device-id>/main
    fetch = +refs/heads/git-annex:refs/remotes/pigeon-3f9a0c1d2e4b/git-annex
    fetch = +refs/pigeon/proposals/*:refs/pigeon/peers/<device-id>/proposals/*
    fetch = +refs/pigeon/status/*:refs/pigeon/peers/<device-id>/status/*
```

- **Pull only.** No device pushes history or accepts a push: the filter
  refuses `CONNECT git-receive-pack`, a `pre-receive` hook every push (E12,
  [09 §9.4](09-network-and-joining.md)). Content moves both ways by
  git-annex's P2P protocol (`get`, `copy --to`).
- `pigeond` keeps the remotes in step with the roster. A peer's proposal
  refs land in its own namespace, and accept copies each valid one into
  `refs/pigeon/proposals/` ([04 §4.5](04-roster-and-authority.md)): two
  peers relaying one proposal never race on a ref, and nothing unchecked is
  relayed **[no ADR]**.
- One `git annex remotedaemon --foreground` per repository (ADR 0011), stdin
  held open, tells peers of changes; on a notice it only fetches by these
  refspecs, never moving `main`, `HEAD`, the index or the tree (E12).

## 3.7 Driving git and git-annex

**Environment.** No inherited `GIT_*` variable, `GIT_CONFIG_NOSYSTEM=1`,
`GIT_CONFIG_GLOBAL=/dev/null`, `GIT_ATTR_NOSYSTEM=1`, `GIT_TERMINAL_PROMPT=0`:
git reads only the `.git/config` pigeon writes (E20; contract test
`capture_ignores_the_persons_git_configuration`).

**Processes [no ADR: driver].** Per repository, one `git cat-file --batch`
and long-lived `git annex … --batch --json` processes for `add`, `whereis`,
`matchexpression`, `get`, `copy`, `drop`, parsed with serde (ADR 0004). Each
call has a timeout (default 120 s); a failure keeps the command, exit status
and last 4 KiB of stderr (ADR 0023). A blank answer is never success; a batch
past its deadline is killed and restarted at the next call
(`a_batch_past_its_deadline_restarts`); it is stopped by closing its input,
so git-annex stages what it queued, and killed after a grace period.

**Allowed commands.** `pigeon_git::allow::check` checks every argument vector
against this table before a process starts (`refused_commands_never_start`),
and refuses `get`, `copy` and `drop` without explicit keys, `copy` without
`--to`, `drop` without `--numcopies` and `--mincopies` except a purge's
`--force` (`drop_refuses_without_k_verified_copies`). It guards against
pigeon's mistakes; it is not a sandbox.

| Allowed | Used for |
|---|---|
| `git cat-file --batch`, `rev-list`, `for-each-ref`, `status --porcelain=v2 -z -uall --ignored=traditional` | reading commits, trees, refs and changes |
| `commit-tree -S`, `update-ref`, `merge-tree --write-tree`, `read-tree`, `update-index`, `write-tree` | commits and merges without touching the working tree |
| `git fetch <remote>`, `sparse-checkout` | history; kept folders |
| `git annex add --no-check-gitignore`, `calckey`, `reinject`, `setkey` | capture of regular files only ([07 §7.9](07-capture-and-install.md)); `setkey` imports an immutable snapshot of a kept aside, never the person's live file |
| `git annex get --key`, `copy --to <custodian> --key` | transfers, in pigeon's order (ADR 0037, [08 §8.6](08-custody-loops.md)) |
| `git annex drop --key --numcopies K --mincopies K` (local, verified; explicit counts outweigh `.gitattributes` and the branch), `drop --key --force` (purge and Drop only) | freeing space, rebalancing, purge |
| `git annex whereis`, `find`, `info`, `matchexpression --key`, `fsck --incremental --time-limit` | observations, wanted keys, scrubs |
| `git annex numcopies`, `mincopies`, `trust`, `untrust`, `semitrust`, `group`, `wanted`, `maxsize`, `dead` | settings (§3.5) |
| `git annex merge`, `smudge --update`, `restage`, `remotedaemon`, `version` | branch merge, sparse folders (E15), notices, checks |
| `git init`, `config`, `remote`, `git annex init`, `describe` | `create` and `join` (§3.6) |
| `git rev-parse`, `ls-files`, `ls-tree`, `diff-tree`, `show-ref`, `hash-object`, `mktree`, `git annex lookupkey`, `examinekey`, `contentlocation` | reading the index, trees and keys; objects without the working tree |

Before the subcommand, only `-c annex.thin=false` (capture never hard-links,
[07 §7.8](07-capture-and-install.md)) and `-c remote.<name>.annex-ignore=true`
are accepted **[no ADR]**. `git annex p2p` runs only as `--enable iroh`
([09 §9.2](09-network-and-joining.md)) and `--gen-addresses`. `remotedaemon`
leads its own process group, which `pigeond` stops with it, since the P2P
fetches it starts can hang past its end ([16 §16.4](16-open-questions.md)).

**Never run:** `git annex sync`, `assist`, `assistant` (they commit and merge,
E9); `move`, `drop --from`, `satisfy`, `--rebalance` against a remote,
`dropunused` (ADR 0014, [08 §8.6](08-custody-loops.md)); `get --auto`,
`drop --auto`; `git merge`, `checkout`, `pull`, `reset --hard` on the working
tree (install is pigeon's, [07](07-capture-and-install.md)); `git push`.

Every output pigeon parses has a contract test against the minimum versions
(`crates/pigeon-git/tests/contract.rs`, ADRs 0004, 0020); a new git-annex is
supported once they pass ([13 §13.6](13-verification.md)).

## 3.8 What may change a repository

| Change | Only by | Never by |
|---|---|---|
| `refs/heads/main` | this device's `pigeond`, `update-ref` after validation ([08 §8.2](08-custody-loops.md)) | a push, a fetch, `git annex` |
| peer, proposal and status refs | `git fetch` by the refspecs of §3.6 | a push |
| `refs/heads/git-annex` | git-annex; §3.5 watches the settings | — |
| the index, `HEAD`, the working tree | `pigeond`: capture, install ([07](07-capture-and-install.md)), sparse folders ([08 §8.5](08-custody-loops.md)) | `remotedaemon` (E12), git porcelain |
| annexed content | `pigeond` driving git-annex; peers through the filter: reads, stores, and removals on laptops only ([09 §9.4](09-network-and-joining.md)) | a remote `REMOVE` on a custodian |

Hooks: pigeon never fires git-annex's laptop hooks (it uses `commit-tree`,
runs `smudge --update` itself, accepts no push), and installs `pre-receive`,
refusing every push, everywhere (a bare custodian has no other hook). The
only background processes are `pigeond`, one `remotedaemon` per repository,
the batch processes and git-annex's network helper
([09 §9.2](09-network-and-joining.md)); never an assistant, `sync` or `watch`.
