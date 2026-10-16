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

## Fixed in the follow-up

Every finding left open above was fixed after this report, in the work
the changelog lists under the release after 0.5.8:

| Finding | Seen by | Fix |
|---|---|---|
| *Free up space* acted at once, and freed files still read "Safe" | P1, P5 | a confirmation naming the files and the backup machines that keep them; a second one when no backup machine has been in touch for an hour; "Safe, not on this computer" |
| Removing the only backup machine had no warning; a removed computer kept saying "Everything is safe" | P5 | its own question, with "your files will not be safe until you add one"; the removed computer says it was removed |
| *Sending…* outlasted ten minutes because the grace counted from `main`'s commit | P5 | each version's wait counts from when this computer first saw it; silent backup machines are named |
| A differing file read "waiting to be captured" forever, with no conflicts view | P5 | its own reason, and a conflicts section on Home and the file page with the three choices of `pigeon resolve` |
| The pause showed only in Home's headline | P3, P5 | a banner on every page, kept when something needs you; Settings offers only what applies |
| Raw UTC times and byte counts | P2, P3, P5 | local times ("until 14:38") and sizes ("29.3 GB"), in English and French |
| "Less than 25% spare" whenever backups = K | P1, P5 | a warning only for a limit below one copy, and a "no spare backup machine" note |
| An abandoned recovery sheet stayed in the roster | P5 | a backup machine is checked first, and the work finishes or is undone even when the browser leaves |
| An admin named "Me" | P2, P3, P5 | "Your name" on Create and `pigeon create --person`; never "Me" |
| A failed join left a half-joined group | P2 | the join is added only once welcomed, and a failed one is cleaned up |
| English download names | P3 | named in the person's language |
| Suggestions decided only whole, with no difference shown | P1 | per-file decisions with bulk actions, each file described, and the outcome shown to the suggester (ADR 0078) |
| No skip link; repeated button names | P4 | a skip link, and each repeated button names what it acts on |
