# 0077. Refounding keeps completed erasure in its signed archive

- **Status:** Proposed, 16 October 2026, for M5.
- Supersedes ADR 0047 only where preserving every old metadata entry or
  protecting every historical key would revive completed erasure. Its
  signed history, explicit new authority and invitation rules remain.

## Context

An erasure decision and its custody preparation refer to the original
genesis, introduction and roster. Copying those documents into a parentless
new genesis invents origins and makes its membership unusable. Removing
them without reading the archived proofs can fetch previously erased bytes
again. Old signed history must remain verifiable in its own authority domain.

## Decision

Refounding selects an independently accepted old snapshot and retains its
signed archive link. Before selecting it, root completion certificates for
any locally complete erasure epochs. A new genesis keeps the current file
tree and settings, replaces the roster, and omits old custody-admission,
purge-decision and completion documents. It begins at rules 1; its own new
participants must explicitly prepare a fresh erasure quorum. Old immutable
votes and receipt fences are never edited to simulate agreement.

For erasure evaluation, follow at most 31 signed canonical archive links.
Validate each old tip and its complete epoch reports with that old group's
isolated history judge. Completed scopes retain their exact original
identity, signer quorum, causal closing head and immutable holds. The new
accepted head observes these anchored histories without giving its genesis
Git parents. Current files therefore have fresh births, and restored or
copied references remain uncovered by old erasure decisions.

Missing or substituted archives and unavailable or unsupported proof objects
pause content and erasure. A cached proof is usable only while every signed
anchor still matches the current retained-ref reading. Never infer that a
missing proof means there was no completed erasure.

Pending requests in the chosen snapshot remain readable in the old archive
and are not reissued in the new authority domain. Refounding confirmation
states this explicitly. Old computers can still finish those requests in
the old group; the new group cannot cancel their actions. Keeping or erasing
the new group's retained versions is a new decision under its new admin.
The chosen snapshot cannot assert knowledge of later old-group decisions.

## Consequences

Refounding no longer carries unusable old capability votes into a new
genesis. Verified completed erasure survives restart and subsequent
refounding, while fresh current references still keep their bytes. Signed
archive/proof tests, real refound/rejoin scenarios and model conformance
remain required before this decision or M5 is declared complete.
