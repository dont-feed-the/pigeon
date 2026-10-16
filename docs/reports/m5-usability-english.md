# Pigeon erasure workflow usability report — English

Participant: spawned end-user usability participant, acting as Nora (admin).
Fixture: `/tmp/pgn-m5u-english`.
Date: 2026-10-16.

I began at Home and used only visible browser navigation and controls. I did not inspect source, repository documentation, private fixtures, application APIs, or use the Pigeon CLI. The browser was closed at the end; the fixture was left running.

## Outcome

I successfully requested erasure of Archive/notes.txt and both observed saved versions, canceled the pending request, and restored the oldest saved version using its version 1 restore button. No assistance or hints were needed. Two attempts failed with recoverable errors before succeeding.

Evidence visible in the application:

- The erasure preview covered 1 current file, 2 observed versions, and 79 distinct bytes.
- After the successful request, Archive disappeared from Files and the pending request listed Archive/notes.txt.
- Cancellation explicitly succeeded. At the end, Erasure requests said “No accepted erasure requests.”
- I first restored the deleted file so I could access its version history, then selected the restore button under version 1. The second attempt succeeded with “Archive/notes.txt is restored on this computer; the newer version stays in its history.”
- After restoring version 1, history showed a new current version 3, “just now,” while versions 2 and 1 remained available. I interpret version 3 as the restored old contents saved as a new version. I did not inspect file contents outside the browser, so this is UI-confirmed success rather than independent byte-level verification.
- Home said “Everything is safe” and showed 2 current files Safe, 0 Missing, and 0 Erased forever.

## What I understood would happen

Requesting erasure removes the current notes.txt from the normal file listing and asks Pigeon to erase the two versions it currently knows about. Merely preparing erasure does not remove or schedule anything. Preparing needs current computers to acknowledge the update, and then I must review and confirm the actual request.

The two selected versions covered 79 distinct bytes, but one selected version shared bytes with Elsewhere/duplicate.txt. Those shared bytes stay because the other file still needs them. The size is therefore not a promise of freed disk space.

Erasure can finish only after the required backup machines agree and each backup has waited at least 30 days after receiving the request. The pending page gave a declared earliest completion of 2026-11-15T12:28:30Z, said each receiver’s own waiting period also applies, and showed Family backup still waiting. I understood the date to be a lower bound, not a guaranteed completion time.

“Permanent” means the selected versions can become unrecoverable through Pigeon once their bytes have actually been removed. It does not promise secure wiping of storage devices or removal of copies outside Pigeon. Shared bytes can remain. Canceling cannot reconstruct bytes already removed. Restoring before completion keeps the restored version. Canceling a pending request does not automatically put the current file back; I needed to restore it separately.

## Consequential steps: interpretation recorded before acting

These interpretations and expectations were recorded in commentary before the respective actions.

| Step | My interpretation and uncertainty before acting | Expected result | What happened |
| --- | --- | --- | --- |
| Open “Erase observed versions” from notes.txt | It sounded like the action for both saved versions. “Observed” left me unsure whether it meant every copy. | A scope preview and confirmation before changes. | A preview explained 1 current file, 2 observed versions, 79 bytes, shared bytes, waiting periods and permanence limits. |
| “Prepare erasing only” | It should prepare the request and wait for computers without completing erasure. I was unsure whether preparation immediately removed the current file. | Preparation, followed by a later confirmation. | The result explicitly said nothing was erased or scheduled and that I must review and confirm again. It waited for Family backup and Bob laptop. |
| First “Request this erasure” | Preparation had completed enough for the request button to appear. | Remove current file and create a pending erasure request for both versions, with time to cancel. | It failed with P-ERASE-STALE: “The history changed after review.” |
| Retry after refreshing preview | The scope still showed the same 1 file, 2 versions, 79 bytes and shared-byte warning. I could not see what history had changed. | The refreshed request should become pending. | It succeeded. |
| Open cancellation review | The request was pending, had a lower-bound completion date and was waiting for Family backup. | Review a cancellation, with restoration separate. | The confirmation said cancellation cannot reconstruct removed bytes and current files are not restored automatically. |
| “Cancel this pending erasure” | Pending cancellation should preserve the versions that remain. It would not put the file back. | Stop the pending erasure, then restore separately. | Cancellation succeeded and repeated that files must be restored separately. |
| “Restore” in Deleted files | There was no version selector here. I expected the latest saved state first so I could reach version history. | Bring notes.txt back and expose its history. | File restoration succeeded; Archive returned to Files. |
| First “Restore this version” under version 1 | Version 1 was the oldest by number; both versions had identical relative ages. | Make the older notes current while retaining newer history. | It failed with P-RECEIVING: “A copy of this folder is still arriving. Try again in a moment.” |
| Retry version 1 restoration after waiting briefly and returning | I expected the temporary receiving state to clear and the same version restore to work. | Restore the oldest notes as current. | It succeeded. History then showed version 3 from just now, plus versions 2 and 1. |

## Errors and confusion

1. **P-ERASE-STALE after preparation:** I followed the visible prepare → Back → request sequence and was told the history had changed. Back showed the same scope; reloading still showed the same scope. A retry then worked. I could recover using the error text, but I had no explanation of what changed or whether I had made a mistake. Screenshots 6–11 preserve this sequence.
2. **P-RECEIVING during oldest-version restoration:** The first version 1 restore failed despite the folder, file and versions being labeled Safe. The text said to try again, and the retry worked. “Safe” did not prepare me for the inability to restore. Screenshots 21–25 preserve this sequence.
3. **Command-style success instruction:** The successful request said “Check pigeon erase for waiting backup machines and completion.” In a browser workflow that read like I was expected to know a terminal command. I used Files → Erasure requests instead, based on a link I had already seen. Screenshot 11.
4. **“Observed” and “seals”:** I inferred that “observed” limited the request to the versions Pigeon knows about, and that all required backup seals meant the backups must agree. These terms were less clear than the concrete counts and waiting-period explanation.
5. **Long identifiers:** The request success showed one long identifier, while the pending/cancellation screens used another long identifier. I matched by Archive/notes.txt, not by those IDs. The cancellation link’s very long ID made scanning harder. Screenshots 11, 13–15.
6. **Restoring the oldest version required an indirect route:** Deleted files offered Restore without a version choice. I restored the current file first and then used the file history. This was discoverable but added a step. Versions 1 and 2 had the same rounded age, so version number was the useful cue.
7. **Post-restore history:** A new version 3 appeared rather than version 1 being marked current. This fits restoring old contents as a new version, but I inferred that behavior; the page did not explicitly connect version 3 to restored version 1.
8. **Home had mixed status language:** Home said “Everything is safe” and also “Not checked yet; pigeon may be busy.” I treated the current-file Safe counts as the protection result but did not know what that unchecked line referred to. Screenshot 27.

## Ease ratings

Scale: 1 = very difficult; 7 = very easy.

| Task | Rating | Reason |
| --- | --- | --- |
| Find the appropriate erasure action | 6/7 | Files → Archive → notes.txt was clear; “observed” needed interpretation. |
| Understand scope, permanence, shared bytes and timing | 6/7 | The preview gave concrete counts and useful limits. “Seals,” receiver timing, and a long timestamp added effort. |
| Make the erasure request | 4/7 | Preparation and explicit confirmation were understandable, but the stale-history error and unchanged preview interrupted confidence. |
| Find and cancel the pending request | 6/7 | The Erasure requests link and cancellation warning were clear; the long ID was distracting. |
| Get the oldest saved notes back | 4/7 | I needed to restore from Deleted files before selecting version 1, then recover from P-RECEIVING despite Safe labels. |
| Overall workflow | 5/7 | I completed all goals without help, but two retries and unclear status details increased uncertainty. |

## Final browser state and evidence

Final browser page: Erasure requests, showing “No accepted erasure requests” (screenshot 29).

Relevant screenshot files are in `/tmp/pgn-m5u-english/browser/`:

- `screenshot-1.png`: starting Home.
- `screenshot-5.png`: complete erasure scope and warnings before preparing.
- `screenshot-6.png`: preparation only, nothing erased or scheduled.
- `screenshot-8.png`: stale preview error.
- `screenshot-11.png`: successful request.
- `screenshot-13.png`: pending status and earliest completion.
- `screenshot-14.png`: cancellation warning.
- `screenshot-15.png`: cancellation success.
- `screenshot-17.png`: Deleted files restore options.
- `screenshot-21.png`: two-version history before oldest-version restore.
- `screenshot-22.png`: receiving error.
- `screenshot-25.png`: successful oldest-version restore.
- `screenshot-26.png`: new version 3 and retained versions 2 and 1.
- `screenshot-27.png`: Home Safe status after restoration.
- `screenshot-29.png`: no accepted erasure requests.

The browser transport printed each page’s visible body text and control labels. I did not need to inspect screenshots to select controls; the screenshot references above are available for review.
