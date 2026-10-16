# Web interface usability study, five browser participants (0.5.5)

Five agent participants each drove pigeon's web interface in headless
Chromium (Playwright) against their own `scripts/try-pigeon` group of real
daemons on 127.0.0.1: Alice's laptop (admin), Bob's laptop (member) and a
backup machine, K = 1, plus extra computers where a task needed one. Each
used only the pages and `docs/guide.md` for the tasks, then located causes
in the source. They are agents, not people: their timings say nothing of
human reaction, and five sessions on one loaded machine (load ≈ 26) made
pages slow.

| Participant | Persona and tasks |
|---|---|
| P1 | Non-technical admin: is it safe, files and versions, restore, deleted files, suggestions |
| P2 | Joining from the browser, the Join and Create forms' error paths, inviting from the web |
| P3 | French speaker: every page and error in French, plurals, layout at 390 px |
| P4 | Keyboard and screen reader, 320–390 px screens, dark mode, JavaScript off, axe-core on live pages |
| P5 | The family's administrator: safety while the backup is down, removal, pause, diagnostics, held names, leave, web security |

## What worked

The security checks held: a used or forged login link, a missing cookie,
a stale form token, a foreign `Origin` or `Host` were all refused with a
readable page. axe-core found no violation on any live page. Every action
works by keyboard alone and without JavaScript. Restoring a version and
bringing back a deleted file did what the page said, checked on disk.
Leave and Remove ask first. Names with HTML in them are escaped. Nothing
was ever called safe that was not on the backup machine.

## Fixed with this report

| Finding | Seen by | Fix |
|---|---|---|
| One file with an accent in its name (`café é.txt`, `noël.txt`) stopped every change from being captured, silently, when `pigeond` had no UTF-8 locale: git-annex skipped the name and reported success | P1, P3 | children get a UTF-8 `LC_CTYPE` when the inherited locale is not UTF-8 (`pigeon-git/src/env.rs`) |
| Home's suggestion count read "Not checked yet; pigeon may be busy" most of the time, so an admin never saw a waiting suggestion there | P1–P5 | the last count read stands while the group is busy; an unread count is labelled as the suggestions' |
| A relative folder (`Family`, `~/Family`) on Join or Create was made relative to `pigeond`'s working directory, e.g. `/~/Family` | P2, P3 | `~` is expanded on the web; `pigeond` refuses a relative folder (`folder-not-absolute`) |
| Create with an empty name, K = 0 or a bad number gave "Something went wrong inside pigeon", raw 422 text, or left a half-made repository | P2, P3 | refused in words before anything is written (`name-empty`, `copies-zero`) |
| A refused join lost what was typed and "Back" led to Home | P2 | the forms come back under the problem, filled in |
| An invitation wrapped over lines, or pasted as the `pigeon join …` line, was "not one the group knows" | P2 | the code is found in the pasted text |
| A member inviting someone saw "{ $inviter } may no longer invite to this group." | P2, P3 | `invite-admin-only` and `device-name-taken` |
| "backup has been offline since ." with no date, right after joining | P2 | "has not been heard from yet" |
| Every page was titled "pigeon"; unknown paths gave an empty page; long names widened the page on phones; the headline changed silently for screen readers; nothing said to reload without JavaScript; the not-safe list sat below the footer | P1, P3, P4, P5 | a title per page, a 404 page, wrapping, `aria-live`, a `noscript` line, the list under the headline |
| *Check installation* took its first check as its heading; the diagnostics consent was printed twice | P1, P4, P5 | own heading; printed once |
| French: "1 fichiers sur 1", "1 copies", "0 éléments", "État du groupe:", a hint naming a "Voir" button that does not exist | P3 | plural selectors, a localized separator, the hint names "Ouvrir une copie" |
| Leaving deleted a changed file whose committed version was absent here, against "Its files stay in the folder" | P5 (once, not reproduced in a clean group) | only pointer files are removed |

## Still open

| Finding | Seen by | Where to look |
|---|---|---|
| *Free up space* acts at once, empties the folder on disk, and the file still reads "Safe" with no "not on this computer" mark | P1, P5 | `pigeon-web/src/files.rs` |
| Removing the only backup machine gets the same confirmation as a laptop, with no "files stop being safe" warning; the removed computer is not told and keeps saying "Everything is safe" | P5 | `machines.rs`, `web-remove-confirm`; roster handling on the removed device |
| *Sending…* lasted over ten minutes with the backup down: the grace counts from `main`'s commit time, which other commits reset | P5 | `pigeond/src/status.rs` |
| A file that differs from the group's version shows "Only on this computer — waiting to be captured" forever, and its page says "Safe"; the web has no conflicts view | P5 | `status.rs` (why-capturing), no route in `pigeon-web` |
| The pause shows only in Home's headline, not when something needs you, and not on other pages; *Share again* is always offered | P3, P5 | `pigeond/src/health.rs`, `api.rs`, `settings.rs` |
| Times are raw UTC (`2026-10-16T12:38:36Z`) for pauses, invitations and the Space page; sizes are raw bytes there | P2, P3, P5 | `pigeon-i18n/src/render.rs`, `capacity.rs` |
| The Space page warns of "less than 25% spare" whenever backups = K, suggesting a limit that is not set | P1, P5 | `pigeond/src/capacity_view.rs` |
| A recovery sheet whose request is abandoned stays in the roster unprinted | P5 | `pigeond/src/recovery.rs` |
| A group created where `$USER` is empty names its admin "Me", which others then see | P2, P3, P5 | `pigeond/src/api.rs` `person_id` |
| A join that hangs can leave a half-joined group; a retry is told the invitation was used | P2 (once) | `pigeond/src/join.rs`, `invite.rs` |
| Download names say "(version 2)" and "(suggested version)" in English | P3 | `pigeond/src/view.rs` |
| A suggestion of several files can only be accepted or declined whole, with no difference shown | P1 | `decide.rs`, Suggestions page |
| No skip link; repeated button names ("Restore this version" ×3) without context | P4 | `pages.rs`, `files.rs` |
