# pigeon v2 — specification

> **Status:** version 2 of the specification, 14 October 2026: a high-level
> contract that cites the laws and models that check it. It assembles the
> accepted [ADRs](../adr/README.md), the guarantees P1–P6 of the
> [proposal](../design/proposal/proposal.pdf), the [UX study](../design/ux.md),
> the [TLA+ models](../../models/tla/README.md), the
> [catalogue model](../../models/catalogue/catalogue.py), the laws of
> `pigeon-core` and the [git-annex experiments](../../lab/git-annex/README.md).

## What pigeon is

pigeon keeps the files of a small group (a family, a lab, a club) on
everyone's computers, without a central server. Each person works in a
normal folder; pigeon shares it, keeps *safe copies* on the group's *backup
machines*, and always says in plain words which files are safe. It is a
**policy and user-experience layer over git-annex** (ADR 0001), which stores
versions, tracks copies, moves content and refuses unsafe deletions. pigeon
adds a signed roster and the authority rule (P4), capture and install that
never overwrite a person's file (P2), suggestions and decisions, three
states and one honest headline (P6), joining by invitation, a peer-to-peer
transport, a CLI and a local web page.

## Reading order and line budget

Read the files in order the first time. Each has one mission and a budget;
`scripts/check-file-length-and-header` caps every file at 1000 lines.

| File | Mission | Budget |
|---|---|---|
| [01-concepts.md](01-concepts.md) | the vocabulary | 110 |
| [02-guarantees.md](02-guarantees.md) | P1–P6, their assumptions, and the invariants that carry them | 130 |
| [03-repository-and-roles.md](03-repository-and-roles.md) | the git and git-annex layout and the configuration of each role | 210 |
| [04-roster-and-authority.md](04-roster-and-authority.md) | the roster, the authority rule, suggestions, decisions, Force | 210 |
| [05-catalogue-and-names.md](05-catalogue-and-names.md) | the catalogue, merges, conflict names, portable names (P5) | 130 |
| [06-status.md](06-status.md) | states, observations, the headline and its reasons (P6) | 170 |
| [07-capture-and-install.md](07-capture-and-install.md) | capture, install, divergences, the mass-deletion guard (P2, P3) | 210 |
| [08-custody-loops.md](08-custody-loops.md) | accept, placement, repair, freeing, purge, capacity (P1, P3) | 230 |
| [09-network-and-joining.md](09-network-and-joining.md) | keys, transport, the filter, invitations, removal, recovery | 240 |
| [10-interfaces.md](10-interfaces.md) | the request API, CLI, web, notifications, messages, Problems | 170 |
| [11-local-state.md](11-local-state.md) | the files `pigeond` keeps and how they are written | 170 |
| [12-observability.md](12-observability.md) | logs, telemetry, trace events | 90 |
| [13-verification.md](13-verification.md) | the evidence layers, the slice, the laws in code | 340 |
| [14-platforms-and-packaging.md](14-platforms-and-packaging.md) | platforms, git-annex delivery, supervision, the workspace | 150 |
| [15-resource-use.md](15-resource-use.md) | priorities, the work queue, adaptive limits, budgets | 150 |
| [16-open-questions.md](16-open-questions.md) | conflicts left with ADRs, details decided without one, open questions | 170 |

With this README (120) the budget is 3,000 lines. A file that outgrows its
budget moves detail to the law, model or ADR that holds it, rather than
growing.

## Precedence

When two documents disagree, the higher one wins and the lower one is the
bug:

1. **Accepted ADRs** ([docs/adr/](../adr/README.md)): the decisions, never
   rewritten; a change of mind is a new ADR.
   [16 §16.2](16-open-questions.md) lists where this specification still
   departs from one.
2. **This specification, with the laws and TLA+ modules it cites**: the
   normative contract. A cited `pigeon-core` law (module documentation and
   hegeltest property, 13 §13.13) or TLA+ invariant (13 §13.2) is normative
   for its point. A conflict between a sentence here and a law it cites is
   a bug in the sentence: fix the sentence.
3. **The UX study** ([ux.md](../design/ux.md)) and the mockups: words,
   journeys and screens.
4. **The proposal** ([proposal.pdf](../design/proposal/proposal.pdf)): the
   origin of P1–P6, restated in [02](02-guarantees.md); mechanisms that later
   decisions replaced are history.

Superseded drafts ([draft 0](../design/spec-draft-0.md),
[draft 1](../design/spec-draft-1.md)) and the [review](../design/REVIEW.md)
are history only.

## Conventions

- **MUST**, **MUST NOT**, **SHOULD** and **MAY** have their RFC 2119
  meaning; other text explains.
- `(ADR NNNN)` cites the decision a rule comes from. A rule no ADR decides
  is marked **[no ADR]** and listed in [16 §16.3](16-open-questions.md).
- `NN §N.M` cites a section; `scripts/check-spec-refs` checks in CI that
  every cited section exists, so sections keep their numbers.
- Laws are cited by module and test (`journal`:
  `nothing_the_person_wrote_is_lost`), invariants by model and name
  (`CaptureInstall.tla` `NoLoss`), configurations by name (`T_n42o4`, in
  [models/tla/expected.tsv](../../models/tla/expected.tsv)), experiments as
  E1–E25.
- Names in code, `--json` and this specification are internal (*local*,
  *published*, *protected*, *custodian*, *proposal*); people see the words of
  ux.md §2.2. "Default *x*" is a setting with a tested default.

## Scope of v1

In scope: Linux and macOS for people's computers, Linux for backup machines
(ADRs 0005, 0032); groups of up to about 20 devices and a few hundred
thousand files; the CLI and the local web interface (ADR 0021);
notifications and a launcher (ADR 0029); French and English (ADR 0028).
Later: native Windows and the status icon (v1.1), phones, per-file freeing
(ADR 0031), a placement planner (ADR 0003), cloud special remotes,
migration from pigeon 0.9 (ADR 0015), PAKE invitation codes (ADR 0017).
pigeon does not fit projects several people edit at once, nor real-time
sync of hot files (proposal §1).

## Where the accepted ADRs apply

| ADRs | Files |
|---|---|
| 0001 engine; 0002, 0003 copy classes and partial custodians; 0004, 0010 the git and git-annex CLIs | [03](03-repository-and-roles.md), [06](06-status.md), [08](08-custody-loops.md), [14](14-platforms-and-packaging.md) |
| 0007 keys; 0009, 0051 roster, proposals, decisions | [04](04-roster-and-authority.md), [09](09-network-and-joining.md) |
| 0013 change detection; 0034, 0035 no overwrite, the group's version | [07](07-capture-and-install.md) |
| 0014 append-only serving; 0018 purge; 0031 free per folder | [08](08-custody-loops.md), [09](09-network-and-joining.md) |
| 0006 helper, 0011 one daemon, 0016 relays, 0017 joining, 0047 recovery | [09](09-network-and-joining.md) |
| 0021, 0023, 0024, 0027, 0028, 0029 interfaces, errors, words, messages | [06](06-status.md), [10](10-interfaces.md) |
| 0036 local state; 0022, 0025 logs and telemetry; 0008, 0012, 0026 evidence | [11](11-local-state.md), [12](12-observability.md), [13](13-verification.md) |
| 0005, 0032 platforms; 0019 licence; 0069 git-annex delivery; 0037 resources | [14](14-platforms-and-packaging.md), [15](15-resource-use.md) |
| 0046 protected; 0048 versioned formats | [02](02-guarantees.md), [04](04-roster-and-authority.md), [06](06-status.md), [08](08-custody-loops.md), [09](09-network-and-joining.md), [13](13-verification.md) |

0030 is Rejected, 0033 Superseded; the 27 withdrawn ADRs are rules or
details of this specification, listed in [16 §16.3](16-open-questions.md).
