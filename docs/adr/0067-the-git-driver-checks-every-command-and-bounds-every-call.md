# 0067. The git driver checks every command against the allow-list and bounds every call with a deadline

- **Status:** Proposed, 14 October 2026, by the agent, for issue #17
  (Guillaume, in chat: "work on a first version of the implementation even if
  not all details are figured out yet"). Settles the per-platform minimum
  versions of 14 §14.3, marked **[no ADR: per platform]**.

## Context

03 §3.7 lists the git and git-annex commands pigeon may run, the environment
they run in (ADR 0045), and the long-lived `--batch --json` processes
`pigeond` keeps (ADR 0004). Each call has a timeout, and a failure keeps the
command, exit status and end of stderr (ADR 0023). Contract tests pin the
output pigeon parses (ADRs 0004, 0020). Writing `pigeon-git` left details
open. How is the list enforced? Which setup and read commands does it lack?
What does a batch process answer for an input it cannot use? What happens
past a deadline, and how does a batch process end? How long is a timeout,
and which minimum versions apply on each platform?

## Decision

1. **One checked entry point.** Every argument vector passes
   `pigeon_git::allow::check` before a process starts, whether one-shot or
   batch. The check refuses:
   - a subcommand not in the table of 03 §3.7;
   - `--auto`, `--rebalance`, `--from`, `--all` and `--unused` on every
     git-annex subcommand;
   - `get`, `copy` and `drop` without keys (`--key` or `--batch-keys`);
   - `copy` without `--to`;
   - `drop` without both `--numcopies` and `--mincopies`, unless it is a
     purge's `--force`;
   - `add` without `--no-check-gitignore`;
   - `cat-file` outside batch mode, and `status` outside porcelain form.

   The check guards against pigeon's own mistakes. It is not a sandbox,
   since pigeon builds every argument itself.
2. **Setup and read commands join the table.** For `create` and `join`
   (03 §3.6): git `init`, `config` and `remote`, and git-annex `init` and
   `describe`. For reading: git `rev-parse`, `ls-files`, `ls-tree` and
   `show-ref`, and git-annex `lookupkey` and `examinekey`. For writing
   objects without the working tree: git `hash-object` and `mktree`.
3. **Deadlines.** A one-shot call has a deadline, 120 s unless the caller
   gives another. Past it, the process is killed, and the error keeps the
   arguments and the deadline. A failed command keeps its arguments, exit
   status and the last 4 KiB of stderr.
4. **Batch processes** run with `--json --json-error-messages`, and their
   stderr is discarded, since errors arrive in the JSON. One line goes in
   and one line comes out.
   - A blank line means git-annex had nothing to do for that input
     (`batchBadInput`, `CmdLine/Batch.hs`), for example a missing file
     given to `add`. The driver returns it as no answer, never as success.
   - A deadline, an I/O error or the end of the process kills the process.
     The next call starts a new one, and the caller decides whether to
     retry the key, so one stuck transfer cannot block the queue (ADR 0056).
   - Stopping closes stdin and waits for the process to end within a grace
     period, and kills it only after that. git-annex stages its queued
     index updates only as it ends (`Annex/Queue.hs`), so a batch `add` is
     in the index only once its process has ended. Killing it at once lost
     the staging on macOS.
5. **Minimum versions, per platform.** git 2.38.0 everywhere, the first
   with `merge-tree --write-tree` (ADR 0010). git-annex 10.20261006 on
   Linux, the pinned standalone build. git-annex 10.20260901 on macOS, the
   version Homebrew provided when E20 ran. Below these, `pigeond` refuses
   to start with `git-annex-too-old`.
6. **Contract tests are required in CI.** `crates/pigeon-git/tests/contract.rs`
   drives the real git-annex. It pins the blank-line rule, `whereis` of an
   unknown key, a drop refused as `unsafe` when copies are missing, `info`
   taking tree-ish input, `matchexpression` answering by exit status, and
   a restart after a deadline. Without git-annex the tests skip, unless
   `PIGEON_REQUIRE_GIT_ANNEX=1` is set. The `rust` job of `ci.yml` sets it
   on both systems, after installing git-annex as `lab.yml` does, so a
   missing git-annex fails CI rather than skipping.

## Consequences

- A new subcommand or option needs an edit of `allow.rs` and of 03 §3.7
  together. A refused command fails before any process starts, with the
  rule it broke.
- A blank answer leaves the caller to decide about the input. A capture
  pass re-reads the file on its next turn.
- Killing a batch process during a transfer leaves a partial file in
  `.git/annex/tmp`, which git-annex resumes or cleans up later. A killed
  `add` loses its queued staging, so capture adds the file again.
- On macOS, CI tests whatever Homebrew provides on the day it runs. The
  minimum stays at the E20 version until a job pins an older one, so a
  person with an older Homebrew git-annex is refused, never left untested.
