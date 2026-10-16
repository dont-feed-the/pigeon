# pigeon v2 — user experience study

> Status: decided on 13 October 2026; the usability test (§9) may still change it. Guillaume
> asked for this study, so that the command line and the web interface suit people who are not
> technical, and answered its six questions. Decisions: ADRs 0027–0031 and 0033–0035 (see
> their status in the ADR index). Mockups: [`mockups/`](mockups/) (static HTML, open
> `mockups/index.html`).

## 1. Who uses pigeon

pigeon serves a small group: a family, a lab, a club. In each group, one or two people are
comfortable with computers and set things up; the others are not, and should never need to be.

| Persona | Who | Uses | Needs |
|---|---|---|---|
| **Martine**, 68 | retired, family photos and papers | her laptop, Finder, a browser | "Are my photos safe?" Find an old version. Get back what she deleted. |
| **Paul**, 41 | the family's "computer person" | terminal, a NAS at home | Set up the group and the backup machines. Help others from a distance. |
| **Inès**, 27 | PhD student in a lab | laptop, terminal sometimes | Share data with her supervisor; free up space on a small laptop. |
| **Prof. Roy**, 55 | lab head, admin of the group | browser, rarely a terminal | Decide on suggested changes to the lab's reference data. |

Martine is the design target. If she can answer the three questions below without help,
everyone can. Paul and the CLI matter too, but the CLI is never the only way to do something
Martine needs.

**The three questions every screen answers, in this order:**

1. *Are my files safe?*
2. *If not, why, and what will happen?*
3. *Is there something I must do?* (Usually: no.)

## 2. The mental model we teach

One sentence, repeated in the onboarding, the help and the status:

> **Your files live in a normal folder on your computer. pigeon shares them with your group and
> keeps safe copies on the group's backup machines. A file is *safe* once enough backup
> machines hold it.**

Everything else is detail to disclose on demand. In particular, people never need to know
that git, git-annex, commits, keys or remotes exist.

### 2.1 The three states, in plain words

The proposal names three states of a version: local, published, protected. These remain the
names in the code, the spec and `--json`. People see plain phrases instead (ADR 0027):

| Internal | Shown in English | Shown in French | Icon | Colour |
|---|---|---|---|---|
| local | **Only on this computer** | **Seulement sur cet ordinateur** | laptop | grey |
| published | **Shared, not safe yet** | **Partagé, pas encore en sécurité** | clock | amber |
| protected | **Safe** | **En sécurité** | shield with check | green |
| uncertain (P6) | **Not checked since …** | **Pas vérifié depuis …** | question mark | grey outline |

Why not the original words: in everyday English and French, *published* suggests "made
public", and *protected* suggests a password. *Safe* answers question 1 directly. Guillaume
chose the plain words (ADR 0027); the usability test (section 9) still compares both.

The state is always shown with an icon **and** a word **and** a colour, never by colour alone.

### 2.2 Vocabulary

One table, shared by the CLI, the web interface, notifications and documentation. A word not
in the right-hand columns does not appear in the interface.

| Concept (spec) | English | French |
|---|---|---|
| custodian | backup machine | machine de sauvegarde |
| K | number of safe copies | nombre de copies de sauvegarde |
| device, node | computer (or phone, later) | ordinateur |
| roster | members | membres |
| admin | admin | administrateur·rice |
| owned folder | *Paul's folder* — only Paul can change it | *Dossier de Paul* — seul Paul peut le modifier |
| add-only folder, steward | everyone can add; *Roy* decides changes | tout le monde ajoute ; *Roy* décide des modifications |
| proposal | suggestion | suggestion |
| accept / reject | accept / decline | accepter / refuser |
| evict (laptop unsync) | free up space on this computer | libérer de l'espace sur cet ordinateur |
| selective sync on | keep on this computer | garder sur cet ordinateur |
| unlink | delete (can be undone) | supprimer (annulable) |
| release, purge | erase forever | effacer définitivement |
| retention period | waiting period | délai d'attente |
| conflict | edited at the same time; two versions kept | modifié en même temps ; deux versions gardées |
| invite ticket | invitation | invitation |
| scrub, fsck | check | vérification |
| repair epoch | repairing | réparation en cours |
| partition, offline peer | can't reach *Paul's NAS* since … | impossible de joindre *le NAS de Paul* depuis … |
| Force | admin override | dérogation d'administrateur |
| Drop | remove without checking (CLI only) | retirer sans vérifier (CLI seulement) |

Never shown outside diagnostics (`pigeon doctor --details`, logs): git, annex, commit, merge,
branch, remote, key, hash, numcopies, trust, wanted, sync, pointer file, ref.

## 3. Principles

1. **Answer "are my files safe?" first, everywhere.** One headline sentence on the home page,
   in `pigeon status`, in every notification, and in the status icon from v1.1.
2. **Honest, with a time.** "Safe" means observed: K verified copies at the last check, and the
   check time is shown ("checked 4 min ago"). When pigeon cannot know, it says so and says
   since when (P6). It never rounds "probably" up to "safe".
3. **Why, then what next.** Every state that is not "Safe" says why, and what happens next or
   what the person can do. Most of the time the answer is "nothing: it finishes on its own".
4. **Files stay files.** People work in their usual folder with their usual applications.
   pigeon is a helper in the background; its interface is for checking, restoring and
   deciding, not for working on files.
5. **Undo instead of "are you sure?".** Deleting, freeing space, accepting a suggestion: all
   reversible, so no confirmation dialog; the result says how to undo it. Only irreversible
   actions (erase forever, admin override, removing a member) ask, and they say exactly what
   will be lost.
6. **Calm.** No notification for success. Notify only when a person must act, or when a file
   stays not safe for longer than a threshold (24 h by default). Grouped, never one per file.
7. **People and machines by name.** "Paul's NAS", "Martine's laptop", never an identifier.
   Names are chosen at setup and shown everywhere.
8. **Progressive disclosure.** The first level is a sentence; "Details" shows which machines
   hold which copy and when they were checked; diagnostics show the internals.
9. **Same words, same operations in the CLI and the web.** Both render the same messages from
   one catalogue (ADR 0028); no operation exists in only one (ADR 0021).
10. **Accessible.** WCAG 2.2 level AA for the web interface; the CLI never conveys meaning by
    colour alone and works with `NO_COLOR` and screen readers.
11. **Never overwrite what a person saved** (ADR 0034). When their file differs from the
    group's, pigeon shows the conflict and lets them choose.

## 4. The status model

pigeon reduces everything it knows to one **headline** with one of five levels. The worst
level wins; within a level, the sentence names the most important case and counts the rest.

| Level | When | Example headline |
|---|---|---|
| 1. **Needs you** (red) | only a person can fix it: disk full, a backup machine missing for more than 7 days, a mass deletion to confirm, a suggestion waiting for you for more than 3 days, this computer removed from the group | "Your disk is full. pigeon can't save new changes. Free up space or empty the trash." |
| 2. **Not safe yet** (amber) | some versions have been *only on this computer* or *shared, not safe yet* for longer than 10 minutes | "3 files are not safe yet. Paul's NAS has been offline since Monday 18:02; they will be safe when it is back." |
| 3. **Working** (blue) | changes are being saved, sent or received, all younger than 10 minutes | "Sending 12 photos to the group… (2 min left)" |
| 4. **Safe** (green) | every version is safe, checked recently | "Everything is safe. Checked 4 minutes ago." |
| 5. **Not checked recently** (grey) | pigeon can't reach the group, so it can't confirm | "Last confirmed safe yesterday at 18:00. pigeon can't reach the group right now; changes since then are only on this computer." |

Rules:

- The 10-minute grace period avoids alarming people for normal delays: a photo saved one
  minute ago is "Working", not "Not safe yet".
- A headline is at most two sentences: *what*, then *why and what next*.
- Times are relative under a day ("4 minutes ago"), absolute beyond ("Monday 18:02"), always
  in local time, with the exact time on hover or in `--json`.
- Folder rows aggregate their files: "All safe", "3 not safe yet".

## 5. Where people meet pigeon

| Surface | For whom | Role |
|---|---|---|
| **Status icon** in the menu bar or tray, from v1.1 (ADR 0029) | everyone | the headline colour at a glance; menu with the headline, "Open pigeon", "Open my folder", "Pause" |
| **Notifications** | everyone | only level 1, a long level 2, and suggestions waiting for you |
| **Web interface** on this computer (the "pigeon" launcher, a notification, or `pigeon open`) | everyone | status, files and versions, deleted files, suggestions, people and machines, settings |
| **The folder** in Finder or the file manager | everyone | the actual work; edits to folders you can't change become suggestions (ADR 0033) |
| **CLI** `pigeon` | Paul, Inès, scripts | everything, with the same words; `--json` for scripts |

## 6. The web interface

Server-rendered pages (axum + maud, ADRs 0021 and 0077), five sections in a sidebar, a bar at
the top on narrow screens. Each page starts with its own one-line answer.

1. **Home.** The headline card; "Needs you" (actions, if any); "Recent activity" in sentences
   ("Paul added 12 photos to *Photos/Vacances*", "You restored *budget.xlsx*"); the space used
   on this computer.
2. **Files.** The group's folders as a list, including folders not kept on this computer.
   Each row: name, state, size, and a switch *Keep on this computer*. A folder shows its
   owner rule ("Paul's folder" or "Everyone can add; Roy decides changes"). Search by name.
   **Deleted files** is a tab of this page.
   - **A file's page:** its versions as a timeline (who, when, state), *Restore this
     version* on each, and *Details* (which backup machines hold each version, checked when).
     Restoring creates a new version; nothing is lost.
3. **Suggestions.** Two tabs: *For you to decide* and *Yours*. A suggestion shows who, what,
   and a comparison (text: line differences; images: side by side; other files: size and
   date), with *Accept* and *Decline* (optional note). Declined suggestions stay visible to
   their author with the note.
4. **People & machines.** Members and their computers (last seen), backup machines with a
   space bar (used, free, needed) and the last check, *Invite someone* (admins), and *I lost
   a computer*, which removes it from the group from any other computer of the same person.
5. **Settings.** Language, notifications, pause, this computer's name and folder, and
   *Advanced*: diagnostics, logs, the telemetry dashboard (ADR 0025).

Live updates: the headline and activity refresh through server-sent events; without
JavaScript the pages still work, with a refresh link.

## 7. The command line

Built with clap (ADR 0024). Verbs follow the person's intent, not git-annex's operations.
Output follows the same headline-then-detail pattern; colour only on a terminal, never alone;
`NO_COLOR` respected; progress on stderr; results on stdout; `--json` everywhere.

| Command | Does | Web equivalent |
|---|---|---|
| `pigeon status [PATH]` | the headline, then what is not safe and why | Home |
| `pigeon open` | opens the web interface | — |
| `pigeon create NAME` | creates a group with this computer as first member | Welcome → Create |
| `pigeon invite [--admin] PERSON`, `pigeon invite --backup-machine [NAME]` | prints a one-time invitation, valid 24 h | People → Invite |
| `pigeon join INVITATION [FOLDER]` | joins a group | Welcome → Join |
| `pigeon keep PATH` · `pigeon free PATH` | keep a folder on this computer · free up its space | Files → switch |
| `pigeon history PATH` | versions of a file | file page |
| `pigeon view PATH [--version N \| --group]` | opens a read-only copy of a version, outside the folder | file page → Open a copy |
| `pigeon restore PATH [--version N \| --at TIME]` | restores a version, or a deleted file | file page, Deleted files |
| `pigeon deleted [PATH]` | lists deleted files that can be restored | Files → Deleted files |
| `pigeon suggestions` · `pigeon accept ID` · `pigeon decline ID [--note TEXT]` | the suggestion inbox | Suggestions |
| `pigeon conflicts` · `pigeon resolve PATH --take-group \| --keep-copy DEST \| --suggest-again` | your files that differ from the group's, and their resolution | Suggestions → Yours |
| `pigeon people` · `pigeon machines` | members, computers, backup machines | People & machines |
| `pigeon remove-computer NAME` | removes a lost or retired computer | People → I lost a computer |
| `pigeon erase PATH` | asks an admin decision to erase forever, after the waiting period | file page → Erase forever |
| `pigeon pause [DURATION]` · `pigeon resume` | stops and restarts sharing | Settings |
| `pigeon doctor [--details]` | checks the installation and explains what is wrong | Settings → Advanced |

Examples of output (English; the French catalogue says the same):

```text
$ pigeon status
✓ Everything is safe. Checked 4 minutes ago.
  1,204 files in 3 folders · 2 safe copies each · 2 backup machines online
```

```text
$ pigeon status
! 3 files are not safe yet.
  They are shared, but only Mario's desktop holds them; the group keeps 2 safe copies.
  Paul's NAS has been offline since Monday 18:02.

  Photos/2026-10-12/IMG_2201.jpg   shared, not safe yet   1 of 2 safe copies
  Photos/2026-10-12/IMG_2202.jpg   shared, not safe yet   1 of 2 safe copies
  Photos/2026-10-12/IMG_2203.jpg   shared, not safe yet   1 of 2 safe copies

  Nothing to do: they will be safe when Paul's NAS is back. Paul has been told.
```

```text
$ pigeon free Photos/2019
Freeing 14.2 GB on this computer…
  2,310 of 2,316 files are safe and their space is freed.
  6 files are not safe yet; pigeon will free them once they are (usually within a day).
Undo: pigeon keep Photos/2019
```

```text
$ pigeon erase Photos/old-scans
Erasing forever needs an admin's decision and a waiting period of 30 days.
  412 files, 3.1 GB, all versions, on every computer and backup machine.
  Until 12 November 2026 anyone can still restore them.
Ask Roy (admin) to approve? [y/N]
```

Errors are `Problem`s (ADR 0023): a sentence, a hint, and a code for asking for help.

```text
$ pigeon join pgn1-...
✗ This invitation has expired (it was valid until yesterday 14:10).
  Ask Paul for a new one: pigeon invite Martine
  Code: P-INVITE-EXPIRED
```

## 8. Key journeys

**8.1 Create a group (Paul).** `pigeon create "Famille Geiger"` asks for the folder, the
number of safe copies (default 2) and this computer's name. It then explains that the group
needs that many backup machines, and how to add one: `pigeon invite --backup-machine NAS`
on this computer, `pigeon join …` on the NAS.

**8.2 Join (Martine).** Paul sends the invitation by message. Martine opens pigeon (installed
by Paul), clicks *Join a group* and pastes it. pigeon shows "You are joining **Famille Geiger**,
invited by **Paul**" with three short words that Paul's screen shows too, so that both can
check over the phone. She picks her folder and which shared folders to keep on this computer
(the size of each is shown against her free space). Done in four steps; the first files
appear while she reads a one-screen tour of the three states.

**8.3 Daily life.** Nothing to do, and pigeon stays silent. Opening it from the launcher shows
"Everything is safe". (From v1.1, the status icon is green; saving a file turns it blue for a
moment.)

**8.4 Not safe yet.** The headline turns amber after 10 minutes; a notification comes only
after 24 h. Clicking it shows the headline and the list; the sentence says what pigeon waits for
("Paul's NAS offline since Monday") and that Paul has been told. Admins and the owner of the
missing backup machine get the "Needs you" version after 7 days.

**8.5 Free up space (Inès).** In Files, she switches off *Keep on this computer* for
*Raw data 2024*. The folder disappears from her disk once its files are safe, and stays
listed in pigeon's Files page with the size freed. Files not safe yet keep their space until
they are; the row says so. Switching it back on downloads the folder and makes it reappear
complete, never as empty placeholders. In v1, space is freed per folder, not per file
(ADR 0031; experiment E15: `git sparse-checkout` hides the folder, content is dropped and fetched by
key, `git annex smudge --update` fills the files back).

**8.6 Delete and restore.** Deleting a file in Finder deletes it for everyone, as in any
shared folder, and it moves to *Deleted files*, where *Restore* brings it back with its
history. Deleting many files at once (more than 100, or a whole top folder) is held back:
"You deleted 1,204 files in *Photos*. Delete them for everyone, or bring them back?" Until
she answers, the deletion stays on her computer only.

**8.7 An old version.** Files → *budget.xlsx* → the timeline shows "Today 14:05, you", "Today
09:12, Paul", … → *Restore this version*. The restored version becomes the current one, and
the newer one remains in the timeline.

**8.8 Someone else's folder (ADRs 0033, 0034).** Files in a folder she may not change stay
editable, and the Files page says so beforehand: "Paul's folder: your changes become
suggestions to Paul". When Martine saves *tarte.docx* there, her file stays as she saved it;
pigeon sends her version to Paul as a suggestion and the file reads "Your suggestion, waiting
for Paul". If Paul accepts, her version becomes the group's. If he declines, or changes the
file meanwhile, pigeon tells her once that her version differs from the group's, and she
chooses: *Take the group's version*, *Keep mine as a copy*, or *Suggest again*. *Open Paul's
version* shows the group's version as a read-only copy outside her folder (ADR 0035). pigeon
never overwrites her file by itself.

**8.9 Edited at the same time.** Both versions are kept. The other one is named
`budget (Paul, 13 Oct 14:02).xlsx`, so that it still opens in the same application. Home
says "Paul and you edited *budget.xlsx* at the same time; both versions are kept" with
*Keep mine*, *Keep Paul's*, *Keep both*.

**8.10 Lost laptop.** From any other computer of hers, or by asking an admin: People & machines
→ *I lost a computer* → it is removed from the group and can no longer receive changes. Her
files are safe on the backup machines; on a new laptop she joins again with a new invitation.

**8.11 A backup machine dies (Paul).** The headline turns amber for files that drop below 2
safe copies, then "Needs you" for Paul after 7 days: "Paul's NAS has been missing for 7 days.
1,204 files have only 1 safe copy. Replace it or add another backup machine." Adding one
starts repairing, with progress in files and GB.

## 9. Usability test plan

Before code, with the clickable mockups; then again with the real system in week 2 of the
feasibility study (proposal §7).

- **Participants:** 5 per round (finds most problems at low cost), not technical, who use a
  computer for photos or documents; plus 2 "Pauls" for the setup journeys.
- **Method:** think-aloud, moderated, 30 minutes, the facilitator never explains the words.
- **Tasks:** (1) Is *IMG_2201.jpg* safe? (2) Why not, and what will happen? (3) Free up space
  for *Photos/2019*. (4) Get back yesterday's version of *budget.xlsx*. (5) You deleted
  *Recettes* by mistake: get it back. (6) Paul suggested a change to your file: look at it
  and decide. (7) Join the group with this invitation.
- **Teach-back:** "Explain to a friend the difference between *Only on this computer*,
  *Shared, not safe yet* and *Safe*." Half of the participants see the original words
  (*local*, *published*, *protected*) to compare.
- **Measures:** task success without help, errors, time, the single ease question (1–7) per
  task, and every wrong belief about safety, which counts as a critical error.
- **Pass:** at least 4 of 5 explain the three states correctly; at least 4 of 5 complete
  tasks 4 and 5 without help; **no** participant believes a file that is not safe is safe.

## 10. Libraries this study adds

| Need | Choice | Why |
|---|---|---|
| Message catalogue, French and English | `fluent-bundle` 0.16 + `fluent-langneg` 0.14 (Project Fluent, Mozilla) | plural and gender rules per language, translators edit `.ftl` files, shared by CLI and web (ADR 0028) |
| Tray icon (v1.1) | `tray-icon` 0.26 in a small `pigeon-tray` process | Linux, macOS and later Windows; a client of `pigeond` like the CLI (ADR 0029) |
| Notifications | `notify-rust` 4.18 | D-Bus on Linux, Notification Center on macOS |
| Terminal colour | `anstream` 1.0 / `anstyle` (clap's own) | honours `NO_COLOR`, `CLICOLOR`, non-terminals |
| Times and time zones | `jiff` 0.2 | local time zones and spans done right; relative times rendered by Fluent messages |
| Opening the browser | `open` 5.4 | `pigeon open` and the tray menu |

## 11. Open questions

- Phones (read-only viewer of status and files) are out of v1 but the vocabulary already
  says "computer (or phone, later)".
- Installers for people like Martine (a signed `.pkg` for macOS, packages for Linux) are not
  designed yet; in v1, Paul installs.
- Thresholds (10 minutes, 24 hours, 7 days, 100 files) are guesses to tune in the test.
