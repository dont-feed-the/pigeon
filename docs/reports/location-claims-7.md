# Location-query cost checkpoint

M6 remains incomplete. This work targets location-log reads, not transfers
or physical-copy verification. The full reference and HDD budgets still
need measured results.

## Native boundary

In git-annex 10.20261006, `Command/Whereis.hs`'s `perform` calls
`keyLocations`, then `getUUIDUrls`, then `trustPartition`. URL collection
reads web and claimed-URL logs even when the caller consumes only UUIDs.
Pigeon uses the UUIDs in `whereis` and `untrusted`; it does not use these
URLs. `Remote/Git.hs`'s standard Git remote has no `whereisKey` callback,
but claimed-URL lookup still happens. `--fast` does not select a different
path in this command.

The optional patched `--no-urls` skips URL lookup. It retains an empty URL
list for each recorded UUID, because the upstream JSON formatter derives
its location entries from those associations. Returning an empty
association list would incorrectly hide all locations. Normal queries keep
the upstream behavior. The new build is `10.20261006-pigeon2`; the two P2P
fixes from pigeon1 remain applied.

Pigeon's custody and capacity reads share capability detection: one bounded
`whereis --help` probe per repository view family, including concurrent
clones and download views. Only a successful probe advertising the exact
option enables it. Stock builds and failed probes retain ordinary queries.
Query exit status, input identities, existing completeness checks, trust
classes and physical-presence checks retain their previous meaning.
Custody batches remain bounded to 256 keys and 64 KiB of input.

## Evidence and limits

All four patches apply with zero fuzz to the checksum-verified upstream
tarball. The new native contract uses synthetic web and ordinary UUID
claims plus an absent key, checking both trusted and untrusted web states,
complete ordered replies, equal UUID sets and success bits, omitted URLs,
and zero content objects. It fails against pigeon1 because the option is
absent. The Linux producer must pass that contract and the existing P2P
and installer checks. Both platform archives are prepared before the pin
change is integrated; macOS runtime checks remain periodic background work.

Eight focused caller/batching checks pass locally with pigeon1, including
the real 301-key present/absent fixture and concurrent capability discovery
with supported, absent, similarly named and failed-help outcomes. Workspace
all-target Clippy passes. Native pigeon2 results and timing comparisons
remain pending; no speedup is inferred from source inspection.

The documented lab-feature profile passes all 235 daemon-library checks
in 24.478 seconds, followed by Clippy and repository rules. An earlier run
omitted that feature: 234 passed, while the explicit-destination fixture
correctly encountered production's unsupported-overlay refusal before
creating its repository. That original is retained; the lab profile does
not waive release-build filesystem admission.

A prior 100,000-key synthetic fixture used the reserved web UUID by
mistake. Its complete 256/4096-key trials and original partial follow-up
remain retained, but they are not ordinary-repository measurements. A
corrected ordinary-UUID fixture has a separate original setup receipt.
Its single 4,096-key preflight pair took 3.070 seconds at 256 keys and
2.415 seconds at 4,096 keys per process. That pair does not establish a
full-size speedup or justify increasing production bounds. Both fixtures
contain only synthetic claims and no content objects.

## Positive-answer lookup reuse

The network sync adapter previously started a separate native
`contentlocation` query before each durability/hash barrier. A controlled
257-object regression reaches all 257 successful hashes and then fails its
startup bound: 257 children instead of two. That original is retained.

Each incoming stream now owns its own lookup adapter. It reuses the existing
worker's bounded 256-query/64-KiB session, serializing only metadata lookup,
and retains the sync adapter's 60-second native-call deadline, including the
encoded-key fallback. No path, presence, durability or hash result is cached.
Every answer still opens and syncs the actual object and directories, then
hashes the object when required. A repository directory identity change
invalidates the session, and a changed identity during lookup refuses it.
Stream cancellation drops its owned process; a weak idle timer closes a
remaining lookup after two idle seconds. The timer cannot retain an idle
stream's cache. Hashing and syncing are outside the lookup mutex.

Four adapter checks pass: the 257-object bound plus later corruption/removal,
independent stream helpers and idle/drop cleanup, same-path repository
replacement, and cancellation before a reply. Private Unix datagram sockets
observe actual helper lifetime without signaling potentially reused PIDs.
The real annex fixture also checks 300 independent object hashes across the
session boundary, then refuses later corruption, removal and an absent key.
Its first corruption attempt correctly hit annex's read-only permissions;
the corrected private fixture explicitly changes only its synthetic object's
permissions before damaging it. An earlier fixture compile error is also
retained. Neither is counted as a product failure or a passing regression.

All 239 daemon-library checks pass in the documented lab profile in
25.853 seconds, followed by all-target workspace Clippy and repository rules.
The startup-count comparison uses an instrumented helper, not a native
throughput claim. Network integration and native pigeon2 validation remain
separate requirements before integration.

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
