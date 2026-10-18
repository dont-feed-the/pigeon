# M7: complete native object inventories

> Mission: refuse incomplete Git object replies before they become smaller authoritative inventories.

## Reproduced original behavior

Review finds that the one-shot `cat-file --batch` decoder stops at an
incomplete header, substitutes empty data for a short payload and treats
malformed headers as missing objects. Current key discovery then filters
missing objects out. Retained-history size selection checks individual
headers but does not require one complete ordered header for every request.

Five cases against the extracted original decoder fail: a missing reply,
a short payload, a malformed header, a wrong delimiter and an extra reply.
Three native pipe-fault tests run the original production readers with a
controlled child that exits successfully. All fail their required refusal:
a short object reply succeeds with fewer objects; a short size reading
succeeds with a smaller retained key set; a missing current pointer succeeds
with fewer current paths. Their complete RED logs are retained before fixes.
The native RED log SHA-256 is
`b99dd8f4482f37612920007c20bf7e89f81181484533333fc1e194ebc5a3e799`.

These faults establish parser and inventory behavior. They do not claim a
spontaneous native Git truncation, an external exploit or observed data loss.
Git also legitimately returns an explicit missing-object record with status
zero; the positive native fixture below exercises that case without a stub.

## Complete ordered readings

The shared driver decoder requires exactly one reply per request, valid
header encoding, a known object kind, a parseable size, every payload byte
and its exact newline delimiter, and no trailing replies or bytes. Full
SHA-1/SHA-256 object IDs must match their request position. Explicit missing
or ambiguous replies must name that same request and preserve its position;
malformed metadata is an error. A size that overflows its frame is refused
before slicing or allocating a payload. Empty or newline/NUL-containing
object requests are refused before starting a child.

Retained-history size selection uses the same complete ordered metadata
checks before selecting small blobs. Current and retained pointer inventories
refuse missing objects or a wrong object kind. Existing retained-reference,
quorum, signature, hash and guarded-drop checks remain responsible for their
usual proofs. A read error cannot authorize an omitted requested pointer.

## Validation

Four laws run 1000 generated cases each: arbitrary binary payloads round
trip; strict frame prefixes fail; arbitrary trailing bytes fail; complete
metadata preserves sizes and order while prefixes and reordered IDs fail.
Focused cases cover 40/64-digit identities, explicit missing entries, invalid
sizes, frame-size overflow, malformed headers and wrong delimiters.

All 40 driver library/native contract checks pass. All 207 daemon library
checks pass, including the three corrected pipe-fault cases. A native Git
fixture writes binary bytes and a pointer, reads its complete current and
historical inventories, then removes that fixture's loose pointer object.
Git returns an explicit missing record with status zero; current discovery
now refuses the read and historical discovery also fails. No other
repository or original observation is edited by the fixture.

The full law/trace and security audits, platform/runtime evidence, resource
budgets, elapsed gates and final-RC qualification remain separate work.

Nine additional native history, backup-rejoin, health and purge-format checks
pass in 35.353 seconds. They preserve normal inventory behavior, historical
versions and refusal of unreadable protection evidence. All-target workspace
Clippy and repository rules run before committing; exact-head CI and fresh
publication gates remain required before a release.

Main `dde2472b1c2e1ccd20216943ec2931aca1b0e256` is integrated before
committing and PR validation, including the v0.6.43 version-only release
changes. The one local pre-commit Clippy miss is an unnecessary clone in
the native fixture; it is replaced by a borrowed one-element slice.
The original failed check remains retained. No CI retry is used.
