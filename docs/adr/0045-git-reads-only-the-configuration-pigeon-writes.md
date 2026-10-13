# 0045. git and git-annex read only the configuration pigeon writes

- **Status:** Proposed, 14 October 2026, by the agent, for issue #14
  (Guillaume's review: "Define the supported filesystem object and metadata
  model"), on the evidence of experiment E20.

## Context

`pigeond` runs git and git-annex as the person, so they read the person's
configuration. They also read files that the shared folder itself may hold.
Each of these changes what capture produces:

- **The person's global configuration** (`~/.gitconfig`,
  `$XDG_CONFIG_HOME/git/config`) and the system one (`/etc/gitconfig`).
  E20, with a `~/.gitconfig` setting `annex.backend = WORM`: capture made
  `WORM-s5-m…--t.txt` keys, which carry no checksum, so no custodian could
  verify them. With `annex.largefiles = nothing`, which git-annex ranks above
  `.gitattributes`, files became plain git blobs. With `core.excludesFile`,
  matching files were neither listed by `git status` nor added: `git annex
  add --batch` answered an empty line.
- **In-tree `.gitattributes` and `.gitignore`** in any subfolder, for
  example from a project copied into the folder. A nested `.gitattributes`
  can set `annex.largefiles`, `annex.backend`, `filter` or
  `working-tree-encoding` for its paths. A nested `.gitignore` hides files
  from `git status`, and `git annex add` skips ignored files unless given
  `--no-check-gitignore`.
- **Variables** such as `GIT_DIR`, `GIT_INDEX_FILE` or
  `GIT_CONFIG_PARAMETERS` in the environment `pigeond` inherits.

The settings check of ADR 0043 watches git-annex's shared settings, not
these. Options were to refuse to run when the person has a global
configuration, which most developers have; to check a list of dangerous
keys, which grows with every git release; or to give git a configuration of
pigeon's own.

## Decision

- **A clean environment.** `pigeond` starts every git and git-annex process
  without any inherited `GIT_*` variable. It sets `GIT_CONFIG_NOSYSTEM=1`,
  `GIT_CONFIG_GLOBAL=/dev/null` (git ≥ 2.32; pigeon's minimum is 2.38, ADR
  0010), `GIT_ATTR_NOSYSTEM=1` and `GIT_TERMINAL_PROMPT=0`. git then reads only
  the repository's `.git/config`.
- **pigeon writes that configuration**, and nothing else does (spec 03 §3.8).
  Besides the settings of spec 03 §3.4, it sets:
  - `core.attributesFile` and `core.excludesFile` to an empty file of
    pigeon's own, since git reads `$XDG_CONFIG_HOME/git/attributes` and
    `ignore` by default;
  - `annex.backend = SHA256E`;
  - no `annex.largefiles`, so that the attributes below decide.

  The file has no `include` or `includeIf` sections.
- **`.git/info/attributes`**, which git ranks above every in-tree
  `.gitattributes` (gitattributes(5)), is written by pigeon on every device:

  ```gitattributes
  * filter=annex annex.largefiles=anything annex.backend=SHA256E -text -eol -ident -working-tree-encoding
  /.gitattributes annex.largefiles=nothing
  /.pigeon/** annex.largefiles=nothing
  ```

  A nested `.gitattributes` is then an ordinary shared file with no effect.
  `git annex init` writes `* filter=annex` there itself, and pigeon keeps it.
- **In-tree ignore files have no effect.** Capture lists changes with
  `git status --porcelain=v2 -z -uall --ignored=traditional`, and adds with
  `--no-check-gitignore`. Only pigeon's own list decides what is ignored
  (ADR 0044). `--ignored=matching` would list an ignored folder without its
  files (E20). A nested `.gitignore` is shared like any file.
- **The settings check** (spec 03 §3.5) also compares `.git/info/attributes`
  and the keys above with the expected ones. A difference suspends drops and
  capture until pigeon has rewritten them.

## Consequences

- Capture produces the same keys and the same tree on every device, whatever
  each person's git setup; rule 6 (ADR 0044) is then never broken by an
  honest device.
- The person's own git use elsewhere is untouched. In the shared folder,
  their aliases, hooks, editor and signing settings do not apply: pigeon
  alone commits there.
- A project copied into the folder is shared whole, including what its
  `.gitignore` names (build outputs, `node_modules`). Its `.git`, if any,
  makes it a `repository` that is not shared (ADR 0044).
- The JSON contract tests (spec 13 §13.6) run in this environment, so they
  test what `pigeond` runs.
- **Evidence:** E20 shows the global settings changing the keys, the tree and
  the listing, and the isolated environment restoring them. It also shows
  `--ignored=traditional` listing files inside ignored folders, and
  `.git/info/attributes` overriding nested `.gitattributes`.
