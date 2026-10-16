# Pigeon erasure workflow — English focused retake

Participant: returning spawned usability participant, acting as Nora (admin).
Build: corrected normal build 242146a, as identified by the test coordinator; I did not inspect source or build metadata.
Fixture: `/tmp/pgn-m5u-english-retake`.
Date: 2026-10-16.

This is a retake by the same participant, not an independent first-use session. I remembered the earlier workflow but did not assume the earlier problems were resolved. The initial report at `/tmp/pigeon-m5-usability-english.md` was left unchanged.

I started at Home and used only visible browser navigation and controls. I did not inspect application source, repository documentation, private fixture files, application APIs, constructed URLs, or the Pigeon CLI. I did not read the downloaded file using a shell or filesystem tool. The browser was closed at the end and the fixture was left running for independent verification.

## Outcome

I successfully requested erasure of Archive/notes.txt and both observed versions, checked the receipt and waiting state, canceled the pending request, restored the deleted file, and restored its oldest saved version (version 1). Every requested action succeeded on its first attempt. No assistance was needed.

I also tried to view the restored file through the browser using the visible current version’s “Download a copy” button. The browser reported a completed, error-free download of `notes (version 3).txt`. The application stayed on the version history and did not display the file’s text. Thus obtaining the restored copy succeeded; inline viewing of its contents was not provided by the visible flow, and I did not independently verify its bytes.

At the end, Erasure requests said “No accepted erasure requests.” Home showed both current files Safe, but said it could not check every saved version right now. I therefore confirmed current-file protection from the UI, not complete older-history protection.

## What I understood

The request removes the current notes.txt and requests erasure of both versions Pigeon knows about. Its preview covered one current file, two observed versions and 79 distinct bytes. Preparation alone does not erase anything or schedule erasure; computers first acknowledge preparation, then I review and confirm the actual request.

One selected version shared bytes with Elsewhere/duplicate.txt. Those bytes stay because the other file still needs them, so 79 bytes is not a promised reduction in disk use.

The pending request cannot complete until the required backups agree and each backup’s waiting period of at least 30 days after receiving it has elapsed. The pending page declared an earliest completion of 2026-11-15T12:43:29Z and warned that each receiver’s own waiting period also applies. It was still waiting for Family backup and all required backup seals. I read this date as a lower bound rather than a guaranteed completion time.

Permanent means the selected saved versions can become unrecoverable through Pigeon after their bytes are actually removed. It does not mean secure disk wiping or erasing copies outside Pigeon. Shared bytes can remain. Canceling cannot reconstruct bytes already removed and does not automatically restore the current file. Restoring a version before completion keeps it. Restoring old contents saves a new current version and keeps the existing history.

## Actions, expectations and results

The interpretations below were recorded in commentary before the corresponding consequential steps.

| Step | Interpretation, expectation and uncertainty before acting | Observed result |
| --- | --- | --- |
| Home → Files → Archive → notes.txt | I expected visible file navigation to lead to a history and erasure review. | Versions 2 and 1 were both Safe. The copy action was explicitly labeled “Download a copy.” |
| Open “Erase observed versions” | I expected a scope review before changes, covering the current file and history Pigeon knows about. | Review showed one current file, two observed versions, 79 bytes, shared-byte warning, waiting periods and permanence limits. |
| “Prepare erasing only” | I expected preparation without erasure and a separate later confirmation. | It said “Nothing was erased or scheduled for erasure,” with Bob laptop and Family backup still waiting. |
| Back to the review | I needed to see whether preparation was complete. | The page explicitly said “Every current computer has acknowledged preparation. Review this scope before requesting erasure.” The original scope and warnings remained visible. |
| “Request this erasure” | I expected notes.txt to disappear from current files and a pending request to be created. | Success on the first attempt. Receipt page provided a receipt identifier and direct Erasure requests and Files links. |
| Receipt → “Erasure requests” | I expected the link to show waiting backups and completion status. | It showed the notes.txt request, the earliest date, waiting-period caveat and Family backup waiting. |
| Cancellation review and confirmation | I expected cancellation to stop pending erasure, preserve remaining versions, and leave restoring the current file as a separate step. | The warning matched that interpretation. Cancellation succeeded and repeated the instruction to restore current files separately. |
| Files → Deleted files → individual “Restore” | No version choice was offered. I expected to restore the latest saved state and then reach its version history. | Restore succeeded. Archive returned to Files and the message explained that restoring older contents creates a new current version while keeping history. |
| File history → “Restore this version” under version 1 | Version 1 was the oldest by number; both ages were rounded to “2 minutes ago.” I expected old contents to become a new current version with history retained. | Success on the first attempt. No receiving error. History then showed version 3 “just now,” plus retained versions 2 and 1. |
| “Download a copy” under current version 3 | I was trying to view the restored file. The label set an expectation of downloading, not necessarily displaying its text. | Browser reported `notes (version 3).txt`, pending false, error null. The page stayed on Versions; no note text was displayed. |
| Home and final Erasure requests | I wanted to confirm protection and cancellation without changing anything. | Home showed 2 current files Safe but could not check every saved version. Erasure requests showed no accepted requests. |

## Clarity and errors

- **Preparation was clear in this run.** “Nothing was erased or scheduled” distinguished preparation from the actual request. After returning, the affirmative acknowledgement sentence explained why the action had changed to “Request this erasure.” I did not encounter a stale-history error.
- **Receipt navigation worked directly.** The success page told me to check Erasure requests and provided that link immediately. I did not need a command or remember a separate navigation route.
- **Receipt and request identifiers still differed.** The success page labeled its identifier as a receipt, while the pending request had a different long identifier. That labeling helped, but I matched the pending request using Archive/notes.txt. The long identifier in the cancellation link remained visually cumbersome.
- **Waiting language remained technical in places.** “Required backup seals” was less familiar than the earlier explanation that every required backup must agree. The timestamp and per-receiver caveat required some thought, but I understood why the earliest date was not a promise.
- **Restoring old contents had a clear history explanation.** The new version 3 matched the message that old contents are saved as a new current version with existing history kept. I no longer had to infer that behavior from the list alone.
- **Oldest-version restoration remained indirect.** Deleted files did not offer version selection. I restored the file first, then opened its history and selected version 1. Both old versions had the same rounded age, so version number was the useful oldest-version cue.
- **The restore succeeded without a retry.** No P-RECEIVING error appeared in this run. This reports what happened once; it does not establish that such an error cannot occur.
- **Downloading was described accurately.** “Download a copy” matched the real browser result. I received a completed download rather than a visible error or an unexpected page. There was no visible inline text view in this flow, so I did not actually see the restored contents in the page.
- **Home did not overstate saved-history certainty at the end.** It said “Pigeon cannot check every saved version right now,” showed current files Safe, and showed older history as not checked. That was more interpretable than assuming all history was protected, although the two repeated “Not checked yet; pigeon may be busy” lines did not tell me how long to wait or which other check they referred to.

No action errors, tooling errors, or requests for assistance occurred in this retake.

## Ease ratings

Scale: 1 = very difficult; 7 = very easy. These ratings are influenced by having completed the earlier session.

| Task | Rating | Reason |
| --- | --- | --- |
| Find the erasure action | 6/7 | Clear file navigation; “observed” still takes interpretation. |
| Understand scope, permanence, bytes and timing | 6/7 | Concrete warnings and counts were helpful. Backup seals and receiver-specific timing add reading effort. |
| Prepare and request erasure | 7/7 | Explicit preparation completion, same scope review, then first-attempt success. |
| Use receipt navigation and understand pending state | 6/7 | Direct link worked. Waiting state was clear overall, with technical terminology and long identifiers. |
| Cancel the pending request | 7/7 | Clear warning and explicit success. |
| Restore the oldest saved notes and understand history | 6/7 | First-attempt success and clear new-version explanation; restoring the deleted file first added a step. |
| Obtain a restored copy through the browser | 7/7 | Accurate Download label and completed download without errors. |
| View restored contents in the browser page | 3/7 | No text view was exposed; the visible action downloaded a file. I did not read that file outside this browser-only task. |
| Overall workflow | 6/7 | Smooth completion with no retries or help; minor navigation and terminology effort remained. |

## Evidence and final state

Screenshot directory: `/tmp/pgn-m5u-english-retake/browser/`.

- Screenshot 1: starting Home.
- Screenshot 4: initial two-version file history and Download labels.
- Screenshot 5: full erasure preview and preparation requirement.
- Screenshot 6: preparation only; nothing erased or scheduled.
- Screenshot 7: every current computer has acknowledged preparation.
- Screenshot 8: successful receipt with direct navigation.
- Screenshot 9: pending status, earliest completion and Family backup waiting.
- Screenshot 10: cancellation warning.
- Screenshot 11: cancellation success with navigation links.
- Screenshot 13: Deleted files restore options.
- Screenshot 14: first file restoration message explaining new current versions.
- Screenshot 17: versions 2 and 1 before oldest-version restore.
- Screenshot 18: successful version 1 restoration.
- Screenshot 19: current version 3 and retained old history.
- Screenshot 20: current-version download action; browser transport reports a completed download.
- Screenshot 21: Home: current files Safe, saved history not fully checked.
- Screenshot 23: final page, “No accepted erasure requests.”

The browser reported this download, preserved for independent verification:

- Name: `notes (version 3).txt`
- Path: `/tmp/pgn-m5u-english-retake/browser/download-1-notes__version_3_.txt`
- Pending: false
- Error: null

The browser transport printed visible page text and labels throughout. I did not need to inspect screenshots to select controls, and did not inspect the downloaded file contents. The application and download outcomes above are UI/browser-confirmed; byte correctness is left to the requested independent verification.
