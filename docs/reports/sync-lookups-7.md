# Positive-answer native lookup reuse

M6 remains incomplete. This change reduces metadata startup work before
positive copy answers; it does not relax durability or hashing.

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

The combined development branch passes 239 daemon-library checks and five
real network checks with pigeon1; those results precede this isolated
change and are retained separately. This branch keeps the published
pigeon1 dependency and excludes the optional whereis URL optimization. Its
current-source validation is recorded below. The startup-count comparison
uses an instrumented helper, not a native throughput or M6 budget claim.

## Isolated validation

The current branch passes all 238 daemon-library checks in 25.258 seconds
with the documented lab feature, plus workspace all-target Clippy and
repository rules. The startup-bound fixture additionally accounts for
observed intervals long enough to permit legitimate idle expiry, so a
loaded runner does not have to finish every pair of barriers within two
seconds. Its focused check passes in 0.084 seconds. Idle expiry itself is
checked separately using the helpers' actual private socket lifetimes.
Current-source end-to-end validation and exact-head CI precede merging.


## Integrated validation

PR [#119](https://github.com/dont-feed-the/pigeon/pull/119), head
`7b3d9b56255a256d0b30f7fe2c14c253d74e3b67`, integrates current main
before validation. CI 39167282406, lab 39167282534 and bridge 39167282487
pass on their first attempts. Five current-source real network checks pass
in 42.932 seconds with unchanged programs and source: capacity accounting,
honest status, a returning backup and Free/Keep. The merge has the same tree
as that validated head. The release retains pigeon1; resource/HDD budgets,
the broader law/trace audit and elapsed M7 gates remain open.
