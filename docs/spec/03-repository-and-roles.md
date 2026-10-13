# 3. Repositories and roles

> Mission: say what git and git-annex hold on each device, how each role is
> configured, and which git and git-annex commands pigeon may and may not run.
> git-annex is the engine (ADR 0001); pigeon reaches it through its CLI
> (ADR 0004) and reaches git through the git CLI only (ADR 0010).

## 3.1 One repository per group and device

A device that belongs to a group holds one git repository with git-annex for it.

| Role | Repository | Where (default) |
|---|---|---|
| laptop | non-bare, the working tree *is* the shared folder | chosen at join, for example `~/Famille Geiger` |
| custodian | bare: objects only, no working tree, nothing exposed to applications **[no ADR]** | `/var/lib/pigeon/<group-id>.git` |

ADR 0002 asks custodians for "locked, read-only objects, never exposed to
applications"; a bare repository is the simplest way to meet it. E9 and E10 ran
custodians as non-bare repositories with locked files; the bare layout must
pass the same experiments before release ([16](16-open-questions.md)).

## 3.2 Refs

| Ref | Holds | Written by |
|---|---|---|
| `refs/heads/main` | the accepted catalogue: a signed commit history | this device's accept and capture loops only |
| `refs/heads/git-annex` | git-annex's branch: location logs, trust, groups, preferred content, metadata | git-annex |
| `refs/pigeon/proposals/<device-id>/<commit>` | proposals, one ref per proposal commit (ADR 0009) | the proposing device; replicated by fetch |
| `refs/pigeon/peers/<device-id>/main` | the last `main` fetched from each peer, before validation **[no ADR]** | `git fetch` |
| `refs/remotes/<remote>/git-annex` | the peer's git-annex branch, merged by `git annex merge` | `git fetch` |
| `refs/pigeon/archive/<group-id>/main` | the history of the group this one was refounded from, read-only (ADR 0047) | the refounding device; replicated by fetch |
| `refs/pigeon/status/<device-id>` | the latest signed status report of each device ([06 §6.3](06-status.md)) **[no ADR]** | its device; replicated by fetch |

`main` only moves forward: every update is an `update-ref` with the expected old
value, to a descendant of it.

## 3.3 What is annexed

`.gitattributes`, written in the genesis commit:

```gitattributes
* annex.largefiles=anything
.gitattributes annex.largefiles=nothing
.pigeon/** annex.largefiles=nothing
```

Every file of the shared folder is annexed (key backend `SHA256E`), dotfiles
included (`annex.dotfiles=true`); pigeon's own files are plain git blobs, so the
commit signature covers them (ADR 0009). Only regular files and their
execute bit are shared; links, nested repositories, empty folders, other
permissions and extended attributes are not, and pigeon lists what it leaves
out ([07 §7.9](07-capture-and-install.md), ADR 0044).

`.git/info/attributes`, which git ranks above every in-tree `.gitattributes`,
is written by pigeon on every device, so that a nested `.gitattributes` copied
into the folder changes nothing (ADR 0045):

```gitattributes
* filter=annex annex.largefiles=anything annex.backend=SHA256E -text -eol -ident -working-tree-encoding
/.gitattributes annex.largefiles=nothing
/.pigeon/** annex.largefiles=nothing
```

## 3.4 Configuration by role

### Laptop (ADR 0002)

| Setting | Value | Why |
|---|---|---|
| `annex.addunlocked` | `true` | files are normal, editable files |
| `annex.thin` | `true`; capture adds with `-c annex.thin=false` | received files cost one copy (E8); a captured version stays an immutable copy until protected, then is re-thinned (E18, ADR 0041) |
| `annex.autocommit` | `false` | pigeon alone commits (E9 pitfall) |
| `annex.synccontent` | `false` | pigeon alone moves content |
| `annex.resolvemerge` | `false` | pigeon merges, with its own conflict names ([05](05-catalogue-and-names.md)) |
| `annex.dotfiles` | `true` | every file is annexed |
| `annex.backend` | `SHA256E` | verifiable keys only (rule 6, ADR 0044) |
| `core.attributesFile`, `core.excludesFile` | an empty file of pigeon's | the person's git attributes and excludes do not apply (ADR 0045) |
| `annex.jobs` | `1` | background work stays light (ADR 0037) |
| trust | `untrust` | its copies never count toward K |
| groups | `laptop` | for diagnostics |
| `wanted` | `include=<folder>/* or …` over the top-level folders kept on this device | selective sync ([08 §8.5](08-custody-loops.md)) |
| sparse checkout | the kept top-level folders and `/.pigeon/` (E15) | folders not kept are absent from disk (ADR 0031) |
| `core.fsmonitor` | `true` on macOS | faster scans (ADR 0013) |
| `core.precomposeunicode` | `true` on macOS | NFC names ([05 §5.4](05-catalogue-and-names.md)) |
| `gpg.format`, `user.signingkey` | `ssh`, the device key file | signed commits (ADR 0007) |
| `core.fsync`, `core.fsyncMethod` | `objects,reference`, `fsync` | commits and refs reach the disk; git's default skips loose objects and refs (ADR 0046) |

### Custodian (ADRs 0002, 0003, 0014, 0018, 0042)

| Setting | Value | Why |
|---|---|---|
| repository | bare ([§3.1](#31-one-repository-per-group-and-device)) | never exposed to applications |
| trust | `semitrust` (git-annex's default), never `trust` | every drop verifies it |
| groups | `custodian` | `sizebalanced` and status count this group |
| `wanted` | `sizebalanced=custodian:K` | partial custodians balanced by capacity; `pigeond` applies it key by key to `keys()`, so to the whole history (E9) and never to released keys (ADR 0042, [08 §8.3](08-custody-loops.md)) |
| `maxsize` | from the roster entry of the device | capacity for `sizebalanced` (ADR 0003) |
| `annex.jobs` | `cpus` | a dedicated machine |
| `core.fsync`, `core.fsyncMethod` | `objects,reference`, `fsync`; objects synced by `pigeond` before any positive answer ([09 §9.4](09-network-and-joining.md)) | what counts toward K is on disk; git-annex itself never syncs (ADR 0046) |
| serving | append-only: no remote `REMOVE` (ADR 0014) | [09 §9.4](09-network-and-joining.md) |

### Group-wide (git-annex branch)

`numcopies = mincopies = K` (ADR 0002). Every device removed from the roster is
marked `dead`, so its copies stop counting and repair starts ([08 §8.4](08-custody-loops.md)).

## 3.5 Settings follow the roster, or drops stop

git-annex's shared settings live in its branch, which any device can change.
They are a *function of the roster*: for each device ID of the roster, its
trust, groups, `wanted` and `maxsize`; for the group, `numcopies` and
`mincopies`. The roster maps each device ID to its git-annex repository UUID
([04 §4.2](04-roster-and-authority.md)).

After every merge of the git-annex branch and every change of `main`, `pigeond`:

1. computes the expected settings from the roster of its `main`;
2. reads the actual ones (`git annex info --json`, `wanted`, `group`, `trust`);
3. if they differ, **suspends every drop on this device** (evict, purge,
   rebalancing) and re-applies the expected settings **[no ADR: re-apply]**.
   The same holds if pigeon's `pre-receive` hook is missing or `core.hooksPath`
   is set (ADR 0043), and then serving peers is suspended too; and if
   `.git/info/attributes`, `annex.backend`, `core.attributesFile`,
   `core.excludesFile` or `core.fsync` differ from §3.3–3.4, or `.git/config` has an
   `include` (ADR 0045), and then capture is suspended too;
4. resumes drops only when a fresh read matches.

Drops stay suspended while two devices with different rosters disagree; this
ends when their `main` branches converge. The status shows "checking the
group's settings" while drops are suspended (P6).

## 3.6 Remotes

Every other device of the roster is a git remote of this repository:

```ini
[remote "pigeon-3f9a0c1d2e4b"]          ; "pigeon-" + first 12 hex digits of the device ID [no ADR]
    url = p2p-annex::iroh+<device-id>   ; git-annex's generic P2P transport (ADR 0006)
    annex-uuid = <the device's git-annex UUID, from the roster>
    fetch = +refs/heads/main:refs/pigeon/peers/<device-id>/main
    fetch = +refs/heads/git-annex:refs/remotes/pigeon-3f9a0c1d2e4b/git-annex
    fetch = refs/pigeon/proposals/*:refs/pigeon/proposals/*
    fetch = +refs/pigeon/status/*:refs/pigeon/peers/<device-id>/status/*
```

- **Pull only.** A device never pushes `main` or proposals into a peer; each
  device fetches and validates for itself (P4). A device also accepts no push:
  pigeond's filter refuses `CONNECT git-receive-pack`, and a `pre-receive` hook
  refuses every push (E12, ADR 0043). Content moves both ways
  through git-annex's P2P protocol (`get` downloads, `copy --to` stores).
- Remotes are created and removed by `pigeond` when the roster changes.
- Proposal refs are fetched without `+`: their names are unique and their
  targets never change.
- `git annex remotedaemon --foreground` runs per repository (ADR 0011), with
  its stdin held open by `pigeond` (it exits when stdin closes). It tells
  peers when this device changed, so they fetch at once. On a change notice
  it only fetches, by the refspecs above, and never moves `main`, `HEAD`, the
  index or the working tree (E12); `pigeond` then validates as after any fetch.

## 3.7 Driving git and git-annex

**Environment** (ADR 0045). Every git and git-annex process starts without any
inherited `GIT_*` variable, with `GIT_CONFIG_NOSYSTEM=1`,
`GIT_CONFIG_GLOBAL=/dev/null`, `GIT_ATTR_NOSYSTEM=1` and
`GIT_TERMINAL_PROMPT=0`. git then reads only the repository's `.git/config`,
which pigeon writes. E20 showed a person's `~/.gitconfig` changing what
capture makes: `WORM` keys, plain blobs, silently excluded files.

`pigeond` keeps, per repository, one `git cat-file --batch` process and
long-lived `git annex … --batch --json` processes for the commands it issues
often (`add`, `whereis`, `matchexpression`, `get`, `copy`, `drop`); it parses their
JSON with serde (ADR 0004). Each call has a timeout; a failure keeps the
command, exit status and end of stderr (ADR 0023).

| Allowed | Used for |
|---|---|
| `git cat-file --batch`, `rev-list`, `for-each-ref`, `status --porcelain=v2 -z -uall --ignored=traditional` | reading commits, trees, refs and changes |
| `commit-tree -S`, `update-ref`, `merge-tree --write-tree`, `read-tree`, `update-index`, `write-tree` | writing commits and merges without touching the working tree |
| `git fetch <remote>`, `sparse-checkout` | exchanging history; folders kept |
| `git annex add --no-check-gitignore`, `calckey`, `reinject` | capture (only regular files, checked with `lstat`, ADR 0044) |
| `git annex get --key`, `copy --to <custodian> --key` | transfers, key by key in pigeon's priority order (ADRs 0037, 0042) |
| `git annex drop --key --numcopies K --mincopies K` (local only, verified; the explicit counts outweigh `.gitattributes` and the branch, ADR 0043), `drop --key --force` (purge and Drop only) | freeing space, rebalancing, purge |
| `git annex whereis`, `find`, `info`, `matchexpression --key`, `fsck --incremental --time-limit` | observations, wanted keys, scrubs |
| `git annex numcopies`, `mincopies`, `trust`, `untrust`, `semitrust`, `group`, `wanted`, `maxsize`, `dead` | settings ([§3.5](#35-settings-follow-the-roster-or-drops-stop)) |
| `git annex merge`, `smudge --update`, `restage`, `remotedaemon`, `version` | branch merge, sparse folders (E15), notifications, checks |

**Never run:** `git annex sync`, `assist` and `assistant` (they commit and
merge on their own, E9); `git annex move`, `drop --from`, `satisfy`,
`--rebalance` run against a remote, `dropunused` (they drop from peers or
outside a purge, ADRs 0014, 0042); `get --auto`, `drop --auto` (refused with
`--all` and `--key`, so pigeon chooses keys itself); `git merge`, `checkout`, `pull`, `reset --hard` on the working tree
(install is pigeon's, [07](07-capture-and-install.md)); `git push`.

The output of every command pigeon parses is covered by **JSON contract
tests** run against the minimum git-annex version (ADRs 0004, 0020); a new
git-annex version is supported only once they pass ([13](13-verification.md)).

## 3.8 What may change a repository (ADR 0043)

| Change | Only by | Never by |
|---|---|---|
| `refs/heads/main` | this device's `pigeond`, with `update-ref` after validation ([08 §8.2](08-custody-loops.md)) | a push (refused), a fetch, `git annex` |
| `refs/pigeon/peers/*`, proposals, status refs | `git fetch` by the refspecs of §3.6, from `pigeond` or `remotedaemon` | a push (refused) |
| `refs/heads/git-annex` | git-annex, merging location logs and settings; the settings check watches the settings (§3.5) | — |
| the index, `HEAD`, the working tree | `pigeond`: capture and install ([07](07-capture-and-install.md)), sparse folders ([08 §8.5](08-custody-loops.md)) | `remotedaemon` (E12), any git porcelain |
| annexed content | `pigeond` driving git-annex; peers through the filter: reads and stores, and removals on laptops only ([09 §9.4](09-network-and-joining.md)) | a remote `REMOVE` on a custodian |

| Hook | Installed by | Fires when | Effect for pigeon |
|---|---|---|---|
| `pre-commit` | git-annex (laptops) | `git commit` | never: pigeon commits with `commit-tree` |
| `post-checkout`, `post-merge` | git-annex (laptops) | `git checkout`, `git merge`, possibly sparse changes | `git annex smudge --update`, which pigeon runs itself (E15) |
| `post-receive` | git-annex (laptops) | a push is received | never: no push is accepted |
| `pre-receive` | pigeon (every repository) | a push is attempted | refuses it |

Background processes are `pigeond` and its children: per repository one
`remotedaemon`, the `cat-file --batch` and git-annex batch processes (§3.7),
and the network helper that git-annex starts ([09 §9.2](09-network-and-joining.md)).
Nothing else runs: no assistant, no `sync`, no `watch`. A bare custodian
repository has no hooks from git-annex.
