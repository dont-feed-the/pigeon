# 0048. Replicated formats and rules are versioned; what a device does not understand, it never guesses

- **Status:** Proposed, 14 October 2026, by the agent, for issue #15
  (Guillaume's review: "Specify replicated-format compatibility and rolling
  upgrades").

## Context

ADR 0036 versions local files, and a device refuses a local file newer than
it knows. ADR 0020 checks minimum versions of git and git-annex. What devices
replicate to each other was not covered: the roster, decisions, drop records,
purge decisions, proposals, status reports, invitations and the stream
protocols. A group holds long-offline laptops and custodians upgraded at
different times. An old device that reads a newer document *successfully*,
but ignores a field that changes authority, protection or retention, would
accept what newer devices reject, drop what they keep, or count copies they
no longer count. Parsing is not understanding.

The authority rule makes this sharper. P4 says that all devices accept the
same catalogue from the same commits. That holds only among devices that run
the same rules.

## Decision

**Two numbers.**

- `format = N` in every replicated document (roster, decision files, drop
  records, status reports) is its syntax. A format's schema is **closed**:
  a reader that meets a field the schema does not define treats the document
  as not understood. No field is ignorable by default, so a newer writer cannot
  slip past an older reader a field that matters.
- `rules = N` in the roster is the version of the rule set the group runs: the
  authority rule, the folder rules, the purge rules and the protection rule.
  A record is always judged by the `rules` of the roster at its parents, as
  every rule of spec 04 §4.4 is, so signed history keeps the meaning it had
  when it was made. Nothing replicated is ever migrated in place.
- A pigeon release supports a range of formats and of `rules`, and states it
  (`pigeon doctor`, the status report's `versions`).

**Not understood means "unknown", never a guess.**

- A commit whose tree holds a `.pigeon/` document with an unknown `format` or
  field, or whose parents' roster has an unknown `rules`, gets a third
  verdict: **unknown**. It is neither accepted nor a proposal, its
  descendants are unknown too, and it is kept as fetched, so the device
  judges it again after an upgrade. The compatibility law, a property test
  (ADR 0012): for every commit, the verdict of an older release is the
  verdict of the newer one, or *unknown*.
- A status report of an unknown format is ignored: the device counts fewer
  observations, so it may show less protection, and purges wait for seals it
  cannot read. Both err towards keeping ([08 §8.6](../spec/08-custody-loops.md)).
- A device that holds an unknown commit still captures, serves content and
  keeps every copy. It performs no forced drop (purge, Drop), and it installs
  nothing beyond its last understood `main`. Its headline is *Needs you*:
  "Update pigeon on this computer: the group uses a newer version (1.2).
  Your files are kept; changes from others wait for the update."
  (`update-needed`).

**Upgrades are rolled out by writers, never by readers.**

- A device writes only formats and `rules` that its roster allows. Raising
  `rules` is an admin roster change. `rules` never decreases: a roster that
  lowers it is invalid (spec 04 §4.2).
- Before raising it, the CLI and the web list every device of the roster
  whose latest status report shows a release that does not support the new
  value ("Martine's laptop runs pigeon 1.0: it will wait for changes until
  it is updated"). They ask, or need `--yes`. Laptops long offline are named
  with their last report's date.
- A device that comes back after the group moved on behaves as above: it
  shows *Needs you* with the release it needs, and its own captures, on its
  last understood `main`, are judged later by the rules at their parents.
- **Downgrades.** A release older than the local files refuses to start
  (ADR 0036), with `pigeon-too-old` and the release those files need. A device
  downgraded below the group's `rules` is an old device as above.

**Protocols negotiate, and refuse with a reason.**

- ALPN names carry a version (`pigeon/annex/1`, spec 09 §9.3). A device offers
  every version it supports; a peer with none in common is refused with a
  reason, which becomes `peer-too-old` or `peer-too-new` on both sides and
  names the release to install. The stream header (`"v":1`, 09 §9.8) and
  invitations (`pgn1-`) do the same.
- git-annex's P2P protocol is capped by pigeond's filter at the highest
  version it knows ([09 §9.4](../spec/09-network-and-joining.md)). A newer git-annex therefore never speaks a
  protocol version the filter has not been tested with.

**git and git-annex outside the tested range.**

- The minimum is the oldest version CI tests on each platform (spec 14
  §14.3). A version newer than the newest tested one is **untested**.
- When `pigeond` first starts with a git or git-annex version it has not run
  before, it runs the contract self-test: a subset of the contract tests of
  spec 13 §13.6, on a scratch repository, in under 30 seconds and at idle
  priority (ADR 0037). If a check fails, `pigeond` keeps capturing and
  serving, but suspends every drop until a version passes. It shows *Needs
  you*: "The installed git-annex 10.2027… behaves differently from what
  pigeon was tested with; pigeon keeps every copy until this is resolved."
  `pigeon doctor` names the check.

## Consequences

- P4 holds within one `rules` value. Across releases it weakens to the
  compatibility law: an older device never accepts what a newer one rejects,
  and never rejects what a newer one accepts; it waits.
- Every new field that matters ships as a new `format` or `rules` value, which
  the group enables explicitly. Display-only additions pay the same price.
  This is the cost of closed schemas, accepted because no reader can tell a
  harmless field from a harmful one.
- The core keeps the validators of every supported `rules` value side by side,
  so that history is judged as it was written.
- **Fixtures:** `fixtures/formats/` holds golden documents and small histories
  for every supported (format, rules) pair, each with its expected verdicts.
  Every release runs them, together with fixtures from the next format that
  the previous release must judge *unknown*. Mixed-version peers are a
  prototype test (#17): two releases built from two commits, one group.
- Problem codes gain `update-needed`, `pigeon-too-old`, `peer-too-old`,
  `peer-too-new` and `git-annex-untested`.
