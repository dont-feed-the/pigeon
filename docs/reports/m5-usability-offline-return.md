# Focused usability session: restore original version while erasure stays pending

Date: 2026-10-16.
Participant: source-blind browser participant, acting through Nora’s visible UI.
Normal build: 0.6.0, as supplied by the coordinator; I did not inspect build or repository state.
Fixture: `/tmp/pgn-m5offline-pending-a`.

This was a fresh fixture and focused session. I am a returning participant from earlier browser usability tasks, so I already knew the general Files / Deleted files / version history navigation. This was not a new participant’s first use.

I used visible browser controls only for all product actions. I did not inspect repository source, fixture code, private manifests, daemons, direct Git state, root/private files, downloaded bytes, or any other fixture files. I did not use the product CLI, API, constructed URLs, or filesystem writes for product actions. This report is the only file I wrote.

## Task and first-attempt result

The task was to restore the ORIGINAL first version of Archive/notes.txt while its existing erasure request remained pending and Bob’s laptop was offline. I was explicitly not to cancel, refound, erase again, or change the pending request.

I completed the browser restoration without an error or retry:

1. From Home, I opened Files and read the existing Erasure requests page.
2. I left cancellation alone and navigated Files → Deleted files.
3. The individual notes.txt entry offered Restore but no version selector. I restored it once to bring it back into the file list.
4. I navigated Files → Archive → notes.txt and selected “Restore this version” under version 1.
5. That first version-1 restore succeeded immediately. Back showed new current version 3 and the retained versions 2 and 1.
6. I checked Erasure requests again. The same request remained pending with the same identifier, earliest completion and waiting state.

I made no cancellation, refounding, new erasure, acceptance, decline, or other destructive action. No navigation mistakes, product errors, or task hints were needed. The coordinator’s later Bob restart instruction was test coordination, not assistance in finding or performing the restore.

## Interpretations recorded before consequential actions

Before restoring from Deleted files, I recorded that there was no version choice. I expected Restore to bring back the latest saved state so I could reach version history and choose version 1. I also recorded that this screen did not say how restoration affects a pending erasure, and that I expected the request to remain because I was not canceling it.

Before restoring version 1, I recorded that version 1 was the original by number: both displayed versions had the same rounded age. Version 2 was current and “Shared, not safe yet”; version 1 was Safe and offered “Restore this version.” I expected version 1’s contents to become a new current version while the existing history and pending request remained.

The restore success message said:

> Archive/notes.txt is restored on this computer. Restoring older contents saves a new current version and keeps the existing history. Check Home for sharing and protection, and Suggestions if approval is needed.

This was clear about a new current version and retained history. It did not explicitly connect restoration with an existing pending erasure request.

## UI observations before the later reload

- Starting Home said “Pigeon cannot check every saved version right now.” It listed one current file Safe and saved history as not checked. No application error was displayed.
- Files initially contained Elsewhere but no Archive, consistent with notes.txt being deleted from current files.
- The existing pending request was:
  - Identifier: `404998617963e473f2108431c0580a309bc23d5a07ee3635fc23a2ca3aaf5746`.
  - File: Archive/notes.txt.
  - Declared earliest completion: `2026-11-15T14:37:43Z`, with each receiver’s own waiting period also applying.
  - “Waiting for the retention period and all required backup seals.”
  - “Waiting for: Family backup.”
- The Deleted files page described restoring folders from their latest saved state and retaining newer files/history. The individual notes.txt entry had Restore but no history link or version chooser.
- The initial file restore succeeded. Files then called Archive Safe, but on the next folder page notes.txt was “Shared, not safe yet.” That transient change made protection status feel unsettled.
- In the history before the oldest-version restore, version 2 was “Shared, not safe yet” and version 1 was Safe.
- After restoring version 1, history showed:
  - Version 3, just now: Shared, not safe yet.
  - Version 2: Safe.
  - Version 1: Shared, not safe yet.
- The same pending erasure request remained unchanged after that restore.
- I briefly read People & machines. Bob laptop was listed as a computer, “last heard just now,” rather than carrying a clear offline label. I cannot establish the exact point of Bob’s external restart from that UI observation; I did not inspect process or network state.

## What the UI led me to believe about bytes and the pending request

My UI-only interpretation was that the first version’s contents had become current in a new version 3. The “Shared, not safe yet” state made me think this restoration still needed protection/sharing work to finish. The existing request was still pending because its same identifier and waiting state remained visible.

The restore success text’s promise to keep existing history led me to expect the restored contents to remain available. However, neither the restore screen nor the pending request page explicitly said that restoring these contents removes them from the erasure’s effective scope or prevents their shared bytes being removed later. The request continued to say Archive/notes.txt with no target-version list or note about the restored current version. That created uncertainty: the file looked restored, but a pending erasure for that same path still looked active.

I was therefore much more confident that the restore action completed and the request remained than I was about the restored bytes’ eventual treatment. I did not interpret the UI alone as proof of exact binary contents or long-term exemption from the pending request.

## Later reload after the coordinator restarted Bob

I reported that the version-1 restore had finished and kept the browser open. After the coordinator said Bob had restarted, I reloaded the visible UI and navigated normally to check the file, request and Home.

Observed changes:

- People & machines still listed Bob laptop “last heard just now.” It did not display a special return/reconciliation event.
- Files and the Archive folder now called notes.txt Safe.
- File history now showed versions 3, 2 and 1 all Safe. No restore failure or extra current version appeared during my checks.
- The same pending request still had the same identifier, earliest completion and Family backup waiting.
- Home said “Everything is safe,” with 2 current files Safe and 2 other versions Safe. No files were Missing or Erased forever.
- Home now showed one suggestion awaiting a decision. I read the visible Suggestions page, which said “Bob suggests a change of Archive/notes.txt, saved 1 minute ago.” I did not review its bytes or click Accept, Decline, or the erasure control. This gave a visible indication of Bob’s returned change, separate from the restored file’s normal history.

After reload, I felt more confident about protection because the file and all displayed versions were Safe. The relation between the pending erasure and restored bytes was still unexplained on the screens I used. “Everything is safe” and a pending erasure are compatible in principle, but the UI did not explain that distinction for this restored file.

## Independent coordinator verification — separate from participant observations

After the browser restoration and before my later reload, the root coordinator reported independent verification. I did not perform or inspect those checks. The coordinator’s message stated that:

- Exact original binary bytes had been restored.
- Bob was restarted.
- Bob’s exact offline private edit survived his return.
- The restoring computer’s first-version bytes stayed exact after that return.
- The original erasure remained pending.

This is coordinator-supplied evidence, not an inference from Safe labels or something I verified through the browser. It supports byte correctness and survival on return; it does not change my UI-only confusion about how the pending request communicates the restored bytes’ treatment.

## Ratings and confidence

Scale: 1 = very difficult / very low confidence; 7 = very easy / very high confidence.

| Measure | Rating | Reason |
| --- | --- | --- |
| Ease of restoring the original first version | 6/7 | Both restore actions worked first try. Deleted files required an intermediate latest-state restore before I could choose version 1. |
| Ease of keeping the pending request in place | 7/7 | I simply avoided cancellation, and could confirm the same request afterward. |
| Confidence that I selected the original version | 6/7 | Version 1 and the matching restore button were clear. The actual file contents were outside this participant’s permitted observations. |
| Confidence from UI that restore completed | 6/7 | Explicit success and new current version 3. |
| Confidence from UI that request remained pending | 7/7 | Same request identifier, path, date and waiting state before and after restore/reload. |
| Confidence from UI about eventual retention of restored bytes while erasure remains pending | 3/7 | Restoration retained history, but the pending page gave no version scope or explicit restore exemption. |
| Confidence in protection immediately after restore | 4/7 | Shared/not-safe states moved between versions and one folder summary differed on successive pages. |
| Confidence in protection after reload | 6/7 | All displayed versions and Home counts became Safe. Exact bytes still required independent verification. |
| Overall subjective ease | 6/7 | No errors, retries or help; extra navigation and unresolved scope wording cost confidence. |

## Evidence and closure

Screenshots are under `/tmp/pgn-m5offline-pending-a/browser/`:

- 1: initial Home.
- 3: existing pending request before restore.
- 5: Deleted files Restore option without version selection.
- 6: first restore success.
- 8–9: shared-not-safe current file and versions before original-version restore.
- 10: first-attempt version-1 restore success.
- 11: new current version 3 and varying protection states.
- 13: same pending request after restoration.
- 14–15: People & machines around the externally coordinated restart/reload.
- 18: all three versions Safe after reload.
- 20: same pending request after reload.
- 21: Home Safe counts and new suggestion count.
- 22: Bob’s suggested change; left untouched.

No downloads were requested and no byte contents were read. The browser was closed after the post-return observations. Daemons were left running for the coordinator’s verification.
