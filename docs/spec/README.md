# pigeon v2 — specification

> **Status:** version 1 of the specification, 13 October 2026. It assembles the
> decisions recorded in [ADRs 0001–0042](../adr/README.md), the guarantees P1–P6 of the
> [proposal](../design/proposal/proposal.pdf), the [UX study](../design/ux.md), the
> [TLA+ models](../../models/tla/) and the [git-annex experiments](../../lab/git-annex/).
> It supersedes [spec draft 0](../design/spec-draft-0.md) and [draft 1](../design/spec-draft-1.md),
> which described a custom engine (ADR 0001).

## What pigeon is

pigeon keeps the files of a small group (a family, a lab, a club) on everyone's
computers, without a central server. Each person works in a normal folder.
pigeon shares it with the group, keeps *safe copies* on the group's *backup
machines*, and always says, in plain words, which files are safe.

pigeon v2 is a **policy and user-experience layer over git-annex** (ADR 0001).
git-annex stores versions, tracks where every copy is, moves content and refuses
unsafe deletions. pigeon adds what git-annex lacks:

- a signed list of members, computers and folder rules, and the authority rule
  that every computer applies to every change (P4);
- capture of what people save, and installation of what others saved, without
  ever overwriting a person's file (P2, ADR 0034);
- suggestions for changes a person may not make directly, and decisions on them;
- three version states and one honest headline (P6, ADR 0027);
- joining by invitation, a peer-to-peer transport, a CLI and a local web page.

## How to read this specification

Each file has one mission. Read them in order the first time.

| File | Mission |
|---|---|
| [01-concepts.md](01-concepts.md) | the vocabulary: group, persons, devices, roles, folders, versions, states, removals, escapes |
| [02-guarantees.md](02-guarantees.md) | what pigeon promises (P1–P6), under which assumptions, and the invariants that carry each promise |
| [03-repository-and-roles.md](03-repository-and-roles.md) | the git and git-annex layout of each device, and the configuration of each role |
| [04-roster-and-authority.md](04-roster-and-authority.md) | the roster file, the authority rule, suggestions, decisions, Force |
| [05-catalogue-and-names.md](05-catalogue-and-names.md) | the logical catalogue, merges, conflict names and portable names (P5) |
| [06-status.md](06-status.md) | version states, observations, the headline and its reasons (P6) |
| [07-capture-and-install.md](07-capture-and-install.md) | the disk door: capture, install, divergences, the mass-deletion guard (P2, P3) |
| [08-custody-loops.md](08-custody-loops.md) | exchanging history, placement and repair, freeing space, purge (P1, P3) |
| [09-network-and-joining.md](09-network-and-joining.md) | device keys, the iroh transport, the daemon socket, invitations, removing a computer |
| [10-interfaces.md](10-interfaces.md) | the request API, the CLI, the web interface, notifications, messages and errors |
| [11-local-state.md](11-local-state.md) | the files `pigeond` keeps on each device, their formats and how they are written |
| [12-observability.md](12-observability.md) | logs, telemetry and trace events |
| [13-verification.md](13-verification.md) | TLA+ models, trace validation, property-based tests, the lab, CI, the usability test |
| [14-platforms-and-packaging.md](14-platforms-and-packaging.md) | platforms, delivery of git and git-annex, the Rust workspace, dependencies |
| [15-resource-use.md](15-resource-use.md) | how background work stays out of the way: priorities, adaptive limits, settings, budgets |
| [16-open-questions.md](16-open-questions.md) | what is not decided, and the details this specification adds that still need an ADR |

## Conventions

- **MUST**, **MUST NOT**, **SHOULD** and **MAY** have their RFC 2119 meaning. Text
  without these words explains.
- `(ADR NNNN)` cites the decision a rule comes from. A rule that no ADR decides yet is
  marked **[no ADR]** and listed in [16-open-questions.md](16-open-questions.md), so
  that Guillaume's rule "an ADR for every choice" stays checkable. If this
  specification and an accepted ADR disagree, the ADR wins and this file is the bug.
- Names in code, in `--json` and in this specification are the internal ones
  (*local*, *published*, *protected*, *custodian*, *proposal*). People see the plain
  words of the vocabulary table ([ux.md §2.2](../design/ux.md)), as
  [01-concepts.md](01-concepts.md) recalls.
- Experiments are cited as E1–E15 ([lab/git-annex/](../../lab/git-annex/)); models by
  configuration name (`T_n42o4`, see [models/tla/expected.tsv](../../models/tla/expected.tsv)).
- Thresholds written as "default *x*" are settings with tested defaults; the
  usability test may change them (ux.md §11).

## Precedence of documents

When two documents disagree, the one higher in this list wins, and the lower one
is the bug:

1. **Accepted ADRs** ([docs/adr/](../adr/README.md)): the decisions. They are
   never rewritten; a change of mind is a new ADR.
2. **This specification**: the current, normative contract that assembles them.
   A detail it adds is marked **[no ADR]** until an ADR records it.
3. **Proposed ADRs**: pending decisions. Where this specification assumes one, it
   says so (for example ADR 0040, ADR 0041).
4. **The UX study** ([ux.md](../design/ux.md)) and the mockups: words, journeys and
   screens. This specification follows them, except where
   [16 §16.2](16-open-questions.md) says why not.
5. **The proposal** ([proposal.pdf](../design/proposal/proposal.pdf)): the origin
   of P1–P6, restated normatively in [02](02-guarantees.md). Its mechanisms that
   later decisions replaced (custody tokens, a Windows week in the feasibility
   study, restoring unauthorised edits automatically) are historical.
6. **Superseded drafts** ([spec draft 0](../design/spec-draft-0.md),
   [draft 1](../design/spec-draft-1.md)) and the [review](../design/REVIEW.md) of them:
   history only.

Known gaps are tracked as GitHub issues; [16 §16.5](16-open-questions.md) maps
each to the parts of this specification it concerns and to its state.

## Scope of v1

In scope: Linux and macOS for people's computers, Linux for backup machines
(ADRs 0005, 0032); groups of up to about 20 devices and a few hundred thousand
files; the CLI and the local web interface (ADR 0021); notifications and a
launcher (ADR 0029); French and English (ADR 0028).

Out of scope for v1, designed so that it can come later: native Windows (v1.1,
ADR 0005), the status icon (v1.1, ADR 0029), phones, per-file freeing of space
(ADR 0031), a placement planner (ADR 0003), cloud special remotes, migration
from pigeon 0.9 (ADR 0015), short invitation codes with a PAKE (ADR 0017).

pigeon does not fit projects that several people edit at the same time (use git
or a collaborative editor), nor real-time sync of hot files (proposal §1).

## Traceability

| ADR | Where this specification applies it |
|---|---|
| 0001 engine | everywhere; [03](03-repository-and-roles.md) |
| 0002 copy classes, 0003 partial custodians | [03](03-repository-and-roles.md), [06](06-status.md), [08](08-custody-loops.md) |
| 0004 Rust over the git-annex CLI | [03](03-repository-and-roles.md), [14](14-platforms-and-packaging.md) |
| 0005 platforms, 0032 backup machines on Linux | [14](14-platforms-and-packaging.md) |
| 0006 iroh helper, 0016 relays, 0017 joining | [09](09-network-and-joining.md) |
| 0007 device keys | [04](04-roster-and-authority.md), [09](09-network-and-joining.md) |
| 0008 trace validation, 0012 laws, 0026 CI | [13](13-verification.md), [12](12-observability.md) |
| 0009 roster and proposals | [04](04-roster-and-authority.md) |
| 0010 git CLI only, 0011 one daemon | [03](03-repository-and-roles.md), [09](09-network-and-joining.md) |
| 0013 change detection | [07](07-capture-and-install.md) |
| 0014 append-only serving, 0018 purge | [08](08-custody-loops.md), [09](09-network-and-joining.md) |
| 0015 ignore 0.9, 0019 licence | this file, [14](14-platforms-and-packaging.md) |
| 0020 git-annex delivery | [14](14-platforms-and-packaging.md) |
| 0021 interfaces, 0024 CLI, 0029 notifications | [10](10-interfaces.md) |
| 0022 logging, 0025 telemetry | [12](12-observability.md) |
| 0023 errors, 0027 words and headline, 0028 Fluent | [06](06-status.md), [10](10-interfaces.md) |
| 0030 (Rejected), 0033 (Superseded), 0034, 0035 | [07](07-capture-and-install.md), [10](10-interfaces.md) |
| 0031 free per folder | [08](08-custody-loops.md) |
| 0036 local state | [11](11-local-state.md) |
| 0037 background work | [15](15-resource-use.md) |
| 0038 decisions on suggestions (Proposed) | [04](04-roster-and-authority.md) |
| 0039 pointer-free freeing and keeping (Proposed) | [08](08-custody-loops.md) |
| 0040 install proves its own file, racing saves win (Proposed) | [07](07-capture-and-install.md), [11](11-local-state.md) |
| 0041 captured versions immutable until protected (Proposed) | [07](07-capture-and-install.md), [03](03-repository-and-roles.md), [06](06-status.md) |
| 0042 purge by references and seals (Proposed) | [08](08-custody-loops.md), [05](05-catalogue-and-names.md), [06](06-status.md), [03](03-repository-and-roles.md) |
