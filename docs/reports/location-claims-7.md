# Location-query cost checkpoint

M6 remains incomplete. These measurements cover synthetic location-log
claims, not transfers or physical-copy verification. The reference-tree and
foreground HDD budgets remain unproved.

## Native boundary

In git-annex 10.20261006, `Command/Whereis.hs`'s `perform` calls
`keyLocations`, then `getUUIDUrls`, then `trustPartition`. `getUUIDUrls`
queries URLs only for UUIDs found in its remote map. Unknown UUIDs skip
that branch. The web remote reads its URL logs; a configured standard Git
remote has no `whereisKey` callback (`Remote/Git.hs`) but still reads the
claimed-URL log. Pigeon consumes UUIDs from `whereis` and `untrusted`, not
these download URLs. `--fast` does not bypass this code.

The optional patched `--no-urls` omits URL lookup. It retains an empty URL
list for each recorded UUID, because the upstream JSON formatter derives
its location entries from those associations. Normal queries retain their
upstream behavior. The pinned build becomes `10.20261006-pigeon2`, keeping
both P2P fixes from pigeon1.

Custody and capacity reads share one five-second capability probe per
repository view family, including concurrent clones and download views.
Only a successful help reply with the option as the first token of an
option line enables it. A prose-only mention, similarly named option or
failed help reply selects the ordinary query, as do stock builds without
the feature. An initial prototype incorrectly recognized a prose mention;
its failing fixture is retained before correction. Query statuses, input
identities, completeness checks and trust handling retain their meanings.
Custody batches remain bounded to 256 keys and 64 KiB of input.

## Native build and contract evidence

All four patches apply with zero fuzz to the checksum-verified upstream
source. [Producer 39165272575](https://github.com/dont-feed-the/pigeon/actions/runs/39165272575),
source `9bb6b8313b3d1964dda5cda284aea91bb9330fd6`, passes on its first
attempt: Linux and Apple Silicon archives, the consumer's PATH/version
checks, and installation with the actual daemon children. Both archive ZIP
digests, native archive hashes and ELF64 x86_64/Mach-O64 arm64 headers are
independently checked. Completed build/installer logs and archives remain
retained. Native inputs remain identical after integrating current main.

The Linux producer and the isolated local Linux artifact pass the native
contract: complete ordered web/ordinary/absent-key replies, equal UUID sets
and success bits, trusted/untrusted web states, omitted URLs and zero
content objects. The option is absent in pigeon1, whose original negative
control is retained. Linux also passes the existing P2P E23/E25 checks.
Mac runtime checks remain periodic background evidence; this producer proves
its archive build and version, not a new passing Mac runtime suite.

## Controlled metadata measurements

One Linux host queries 100,000 unique synthetic keys per fixture, with zero
content objects. Each mode uses the same pigeon2 binary and fixed production
bounds: 391 processes, at most 256 keys and 64 KiB per process. Three paired
trials alternate mode order. Every reply is bound to its requested key;
UUIDs, trust lists and success bits must have equal semantic digests between
modes. Metadata tips stay unchanged. Workers serialize with other local
validation; background soak and publication verification remain separate.

Medians in seconds:

| Fixture | Normal wall | No-URLs wall | Normal child CPU | No-URLs child CPU |
|---|---:|---:|---:|---:|
| Unregistered UUID, negative control | 71.613 | 71.523 | 68.673 | 68.389 |
| Reserved web UUID, URL-lookup control | 226.786 | 68.940 | 219.344 | 66.323 |
| Configured standard Git peer | 250.200 | 74.693 | 242.720 | 71.910 |

The configured-peer median is about 3.35 times faster for this metadata
sweep. Both modes retain the same bounded input and process counts; this is
not a daemon throughput, complete backup, memory-budget or HDD result.
The absence of a gain for unknown UUIDs agrees with the upstream branch.
A separate 4,096-key configured-peer preflight took 10.363 versus 3.073
seconds; it is distinct from the full-size three-pair result.

The first earlier fixture intended for ordinary UUIDs accidentally used the
reserved web UUID. Its original complete and partial larger-batch trials
remain retained; they earn no ordinary-peer tuning credit. That fixture is
now an explicit web control. The unregistered UUID and configured Git peer
have independent roots and setup receipts. A prior single 4,096-key pair
comparing 256/4,096-key batches does not justify increasing production bounds.

## Caller validation and adjacent work

All 239 daemon-library checks pass with the actual pigeon2 artifact,
followed by workspace all-target Clippy and repository rules. The real
301-key fixture additionally requires that the currently pinned native
build was discovered, then checks every requested UUID and 300 independent
object hashes. Later corruption, removal and an absent key still refuse.
Mock contracts retain old-build, unsupported-prose and failed-probe fallback.
All five exact-head real network checks pass in 41.768 seconds with pigeon2: capacity, both slice cases and both status-truth cases. Source and program hashes stay unchanged throughout. PR #120 integrates main `e8e0a213709af3e90a77b2bcce701bfa509aadbb` before validation; CI 39172556840, lab 39172556808, bridge 39172556719 and native 39172556620 pass. Fresh v0.6.51 tag gates and independent publication verification remain required.

Stream-owned native lookup reuse before durability/hash barriers shipped
separately in [v0.6.50](https://github.com/dont-feed-the/pigeon/releases/tag/v0.6.50).
Its [report](sync-lookups-7.md) retains the unpublished v0.6.49 health-fixture
failure and the corrected wait; this native URL change does not alter those
barriers or supply any missing physical-storage evidence.

## Full reference diagnostic

[Run 39157311302](https://github.com/dont-feed-the/pigeon/actions/runs/39157311302),
source `c2e27aee280c89f44db9f4c3fbc097e3c738fa9f`, uses exactly the v0.6.47
production runtime with a workflow-only diagnostic change. It fails the
5,400-second Fast preparation deadline before any Gentle measurement
window. Both accepted trees contain 100,000 entries. The last backup
observation counts 6,982 size-matching object files; the failure snapshot
finds 6,996 object files. Neither count is an independent final hash check.
The ingestion phase takes 2,346.435 seconds; the first laptop location read
takes 161.323 seconds and the backup's 186.723 seconds. A later backup
location sweep takes 368.803 seconds. These phases do not isolate one cause
or establish an overall improvement across different shared CI hosts.

The original finished job log and artifact are retained before another
experiment. Artifact 12149997206 is 13,056 bytes, SHA-256
`c6e4e6c0b13887a195b9a91da444bd215a5d9a302e1f6fe8d975263e065cfedc`.
It records zero measurement samples and `not-measured`. No reference, HDD,
storage-safety, elapsed-night or release-candidate qualification follows.
