# 0009. The roster lives in the tree; proposals are git refs

- **Status:** Accepted, 13 October 2026, by Guillaume (tooling interview, Q4).

## Context

P4 says the accepted catalogue is the set of commits valid under the roster
in force at their parents. The roster, the proposals and the decisions on
them must replicate with the catalogue and be signed. A git commit signature
covers the commit's tree, so anything stored in the tree is signed without a
canonical serialization of our own. E7 kept an unauthorized commit as a ref
(`refs/proposals/<peer>/<commit>`) and merged the authorized prefix.

## Options

1. `.pigeon/roster.toml` in the main tree; proposals as git refs.
2. A separate `pigeon` branch, like the `git-annex` branch.
3. Signed JSON documents (JCS canonical form) outside git.
4. A CRDT document (Automerge).

## Decision

Option 1.

- `.pigeon/roster.toml` holds persons and their device IDs (ADR 0007),
  roles (admin, owner, steward), folder rules, K and the retention period.
  The genesis commit is signed by the founders.
- The roster in force for a commit is the file in its parents' trees. A
  commit that touches `.pigeon/` is valid only if its signer is an admin under
  that roster.
- `.gitattributes` sets `.pigeon/** annex.largefiles=nothing`, so these files
  are stored in git and never annexed.
- A rejected commit is kept as `refs/pigeon/proposals/<device-id>/<commit>`,
  replicated by a refspec on every remote. Decisions are signed commits: an
  accept merges the proposal; a reject adds
  `.pigeon/decisions/<commit>.toml`, after which the ref may be deleted.

## Consequences

- Authority is a pure function of history: two nodes with the same commits
  accept the same catalogue.
- No second ordering problem: with a separate branch, which roster applies to
  a catalogue commit would need cross-references.
- Concurrent roster changes produce a git merge of `roster.toml`; the merge
  is itself a `.pigeon/` change and needs an admin signature valid under
  both parents' rosters.
- `.pigeon/` is visible as a hidden folder on laptops; pigeon's watcher
  treats edits to it as roster proposals.
