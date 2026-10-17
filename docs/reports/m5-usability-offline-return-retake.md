# Focused retake: restore original version with existing erasure pending

Date: 2026-10-16.
Participant: returning source-blind browser usability participant, acting through Nora’s visible UI.
Build: normal 2291757, as supplied by the coordinator; no source or build inspection performed.
Fixture: `/tmp/pgn-m5offline-pending-b`.

This is a fresh-fixture retake by the same participant as `/tmp/pigeon-m5-offline-usability-participant.md`, not a new participant’s first use. I remembered the navigation and the prior uncertainty about restored bytes under a pending erasure.

I used only visible browser controls for product actions. I did not inspect source, repository documentation, private fixtures/manifests, daemon or direct Git state, root/private data, downloaded bytes, CLI, APIs or constructed URLs. I made no filesystem writes for product actions. This report is outside the repository.

## Task and first-attempt outcome

The task was to restore ORIGINAL version 1 of Archive/notes.txt while keeping its existing pending erasure untouched. Bob’s laptop was initially offline according to the test setup.

The first-attempt restoration succeeded with no errors or retries:

1. Home → Files → Erasure requests: I read the scope wording before any restore.
2. Files → Deleted files → individual Restore: the file was restored so I could reach its version history.
3. Files → Archive → notes.txt → Restore this version under version 1: the original version restoration succeeded immediately.
4. Back showed version 3 as current, with versions 2 and 1 retained.
5. Files → Erasure requests showed the same pending request, with unchanged identifier, earliest date and Family backup waiting.

I did not cancel, refound, request another erasure, accept/decline suggestions or change other product state. No product hints or assistance were required. External coordination for Bob’s later return is recorded separately below.

## Before restoring: what I understood from the visible wording

The pending request page said:

> Waiting for the retention period and all required backup seals. This request covers the versions reviewed when it was made. Later saves and restores are kept; restoring does not cancel this request.

Before acting, I recorded this interpretation: the request’s scope is fixed to the versions reviewed at the time it was made. A later restoration of version 1 should create a kept current version, while the earlier request continues for its original reviewed versions. Restoring does not cancel or replace that request.

The existing request was:

- Identifier: `2e5872482a611d0819b0592a55a08378f00257d9fb68c94de9febc39a213616f`.
- File: Archive/notes.txt.
- Declared earliest completion: `2026-11-15T14:46:13Z`; each receiver’s own waiting period also applies.
- Waiting for the retention period, all required backup seals and Family backup.

The statement about later saves and restores directly answered my previous 3/7-confidence question. I no longer needed to infer whether restoring under a pending request was allowed to keep a current version.

## Interpretations recorded before each restore

At Deleted files, no version selector or history route was offered. Before choosing the individual Restore, I said I expected it to bring back the latest state so I could reach version history. Based on the pending-page wording, I expected that later restore to be kept and the request to remain active.

The first restore succeeded and the message said:

> Archive/notes.txt is restored on this computer. Restoring older contents saves a new current version and keeps the existing history. Earlier erasure requests stay pending but do not cover this new version. Check Home for sharing and protection, and Suggestions if approval is needed.

Before selecting version 1, I recorded that this explicit confirmation resolved my earlier concern. Version 1 was the original by number; versions 2 and 1 had the same rounded age. I expected version 1’s old contents to become a new current version that the earlier request does not cover, while the old history and request remain.

The version-1 restore succeeded on its first attempt and repeated that same message.

## After restoring: what I understood

The history showed:

- Version 3, just now: Shared, not safe yet.
- Version 2: Safe.
- Version 1: Shared, not safe yet.

The pending request remained and repeated the same scope statement. From these screens, I understood that the restored original contents had become new current version 3, which the old request does not cover. The old request can remain pending for its reviewed versions. I read “Shared, not safe yet” as protection still catching up, rather than as a warning that the old erasure would erase version 3.

The wording gave a clear rule about version scope. It did not give a per-version list of the original reviewed targets or display the contents of version 3. I therefore treated exact bytes and the request’s technical implementation as outside my direct observations.

## Comprehension, confidence and remaining confusion

The main improvement was the same rule appearing both before restoration on the pending page and immediately after the action in its success message. The two messages reinforced each other: later restores are kept, the earlier request remains, and the new version is outside its scope.

Remaining minor issues:

- Deleted files still required an intermediate restoration before I could choose the original version. It had no direct version-history route.
- Version number was the useful original-version cue because both saved ages were rounded to the same value.
- The pending page still used a long identifier and “backup seals,” but the scope clarification was easy to understand in plain language.
- Protection labels changed between versions after restore. The new scope explanation helped separate protection progress from erasure scope, so those changes did not recreate my earlier concern about version 3 being included in the request.
- The request page still did not enumerate target version numbers. I understood the general scope rule confidently, with exact target membership left unspecified by that page.

No action errors, mistakes or task hints occurred before the later reload.

## Ratings before Bob’s later return

Scale: 1 = very difficult / low confidence; 7 = very easy / high confidence.

| Measure | Previous focused session | This retake | Reason |
| --- | --- | --- | --- |
| UI confidence about eventual retention of restored bytes while the old erasure remains pending | 3/7 | 6/7 | The pending page and restore success now explicitly state that later restored versions are kept and excluded from earlier requests. Exact bytes and implementation remain outside my observation. |
| Comprehension of what the pending request covers | Not separately rated | 7/7 | It covers the versions reviewed when made; later saves/restores are outside that scope. |
| Confidence the original version was selected | 6/7 | 6/7 | Version 1 was clear by number; I did not read byte contents. |
| Confidence the request stayed pending | 7/7 | 7/7 | Same identifier, file, time and waiting state after both restores. |
| Ease of restoring original version | 6/7 | 6/7 | First-attempt success, with the same extra restore step to reach history. |
| Overall subjective ease | 6/7 | 6/7 | Navigation unchanged; scope comprehension substantially improved. |

I communicated the restoration completion to the coordinator and kept the browser open for the requested Bob-return reload.

## Evidence before Bob’s later return

Screenshots are under `/tmp/pgn-m5offline-pending-b/browser/`:

- 1: starting Home.
- 3: pending request and new scope clarification before any restoration.
- 5: Deleted files and individual Restore.
- 6: first restore success with explicit earlier-request exclusion.
- 9: versions 2 and 1 before original-version restore.
- 10: version-1 restoration success, first attempt.
- 11: new current version 3 with original history retained.
- 13: same pending request and scope clarification after restoration.

## Reload after Bob’s return

The coordinator informed me that Bob had restarted and asked for one reload. I reloaded the pending-request page once, then used visible links to check Files, Archive, the file history, Home and Suggestions. I made no further product changes.

UI observations:

- The pending request still had identifier `2e5872482a611d0819b0592a55a08378f00257d9fb68c94de9febc39a213616f`, earliest completion `2026-11-15T14:46:13Z`, and Family backup waiting.
- The same explicit scope sentence remained: it covers the versions reviewed when made; later saves and restores are kept; restoring does not cancel the request.
- Archive and notes.txt were now Safe.
- History still had version 3 current and versions 2 and 1 retained. All three were Safe.
- Home said “Everything is safe,” with 2 current files Safe and 2 other versions Safe. Missing and Erased forever counts were zero.
- Home showed 1 suggestion awaiting a decision. Suggestions said “Bob suggests a change of Archive/notes.txt, saved just now.” I left its Review, Accept, Decline and erasure controls untouched.

There were no action errors or unexpected changes during the reload checks. The new wording allowed me to understand the pending request coexisting with Safe restored contents: the restored current version is a later version outside that earlier review. Bob’s suggestion appeared separately, with no need to overwrite the restored file to acknowledge it.

Final subjective ratings remained:

- Scope comprehension: **7/7**.
- UI confidence in retention of later restored bytes while the request remains pending: **6/7**, increased from **3/7** in the previous focused session.
- Confidence the original request remained pending: **7/7**.
- Overall ease: **6/7**.

My remaining confidence limit is that visible messages and Safe labels are not independent byte verification. I understand the promised behavior clearly; I did not inspect exact contents or technical enforcement.

Additional screenshots:

- 14: the single post-return reload, request unchanged with scope wording.
- 15–16: Safe Archive and current file.
- 17: versions 3, 2 and 1 all Safe.
- 18: Home Safe counts and one suggestion.
- 19: Bob’s suggestion, read only and left pending.

## Independent coordinator verification

The root coordinator separately reported that verification passed before and after Bob’s restart: the restoring computer retained exact original bytes, Bob retained the exact offline private edit, the pending erasure stayed unchanged, and the return check passed. I did not perform, inspect or derive those checks from the browser. This evidence is separate from my subjective UI observations and ratings.

## Closure

No mistakes, action errors, retries or task hints occurred in this retake. Bob-return coordination and root byte verification were supplied after I had completed restoration. No downloads were requested, and no byte contents were read. The browser was closed after the final observations; daemon shutdown was left to the coordinator.

