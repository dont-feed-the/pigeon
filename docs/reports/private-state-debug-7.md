# Private state diagnostic checkpoint

The first [secret formatting checkpoint](secret-debug-7.md) corrects network
configuration, served repositories and invitations. The subsequent audit
found additional derived Debug boundaries: a roster's invitation, the
filter's annex token and incomplete buffered authentication line, protocol
actions with raw stream/refused bytes, cached group authentication, recovery
sheets, join proofs and local API requests/responses/messages. Erasure
confirmation/cancellation tokens also appeared in their direct typed values.

Fourteen focused fixtures fail on the original implementations: four core
contracts and ten protocol/daemon contracts. They check real formatting,
including alternate and nested cases, without printing synthetic authority.
The corrected implementations pass all fourteen. Useful peer/group/kind,
state and byte-count metadata remains; sensitive fields print `[redacted]`
or are omitted. API enum matches are exhaustive, so adding a variant needs
an explicit diagnostic choice. Arguments and payloads are not serialized
merely to decide what a diagnostic may print.

The actual fields, framing, signing, authorization and wire derives stay
unchanged. The fixtures independently require intended invitation, recovery,
join, API and erasure data to survive their wire codecs/serialization.
This tests privacy alongside delivery, rather than silently destroying the
credential needed by the recipient. The existing web interface already has
a custom Debug that omits session and login-token stores; bridge password
forms and portal configuration do not derive Debug.

This is a typed-boundary checkpoint, not completion of the broader M7
security review. Generic storage/commit buffers, private helper frames,
external-library diagnostics and every instrumentation/error path still
need review. Real elapsed nights, soak and hardware budgets remain separate.
Broader validation and current-main integration are recorded after running.

Broader local validation passed all 245 core/protocol checks (one slow),
all 135 daemon-library checks, all 16 codec/format checks and the real
annex copy/get/refused-remove integration (66.066 seconds). The fourteen
focused privacy regressions are included in those broader selections;
they are not additional unique checks. All-target workspace Clippy with
lab, trace and console features and the repository header/length, ADR and
spec checks pass. Current-main integration and exact-head PR CI remain
separate before release.

Current v0.6.13 main `a9f8447d09af85a9c5938554038e65346c6e62c6` is integrated before PR validation.
All core/protocol and daemon-library selections, codec/format fixtures,
real annex transfer and the corrected backup-rejoin preservation check
pass again, with all-target Clippy and structural checks. The old repair
changelog items stay in their actual release; only these diagnostic and removal wording corrections
remain Unreleased. Exact-head PR CI and the wider M7 review are separate.

## Release validation

PR #82, head `e28cb6b8a07008668b44e2e126c230a3f37933a0`, passed exact-head CI
39075272648, lab 39075272692 and bridge
39075272709. Current main was integrated before validation.
The fourteen original privacy regressions, four later buffered-input
regressions, both language wording retakes and affected native/codec/library
checks remain distinct evidence above and in their linked reports. Bridge
and core slice caches saved; active main producers remain protected from
cancellation. v0.6.13 was published and independently verified before this
candidate. The wider security audit and actual elapsed M7 gates remain open.
