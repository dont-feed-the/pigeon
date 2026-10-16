# Printable recovery sheets for M4

> Mission: record the paper workflow of ADR 0047 and its verification.

## Behavior

Admins can prepare a sheet in People & machines and print the page with
their browser. It contains localized instructions, the complete checksummed
text code, and an SVG QR carrying exactly that code. The terminal renders
the same instructions and code with a text QR. If the payload exceeds one
QR's capacity, the complete text remains usable and the page explains the
fallback. The wire format and recovery authority exchange are unchanged.

The page recommends a paper key or another computer for an admin with only
one signing computer and no sheet. It recommends a replacement when no
active sheet names the current backup devices. The latter comes from the
key's first appearance in accepted roster history; no paper secret is kept.
Paper names use a date and public ID, so preparing multiple sheets on the
same day works and the role label follows the reader's language.

A local authenticated POST with the session's form token creates the key.
Names are escaped, the QR embeds geometry only, and the response is
`Cache-Control: no-store`. Preparing a sheet before any backup has joined
explains the missing step without adding a key. If no backup receives the
roster commit before the deadline, the unprinted key is revoked.

## Verification

- The real disaster-recovery scenario passed in 98.10 seconds: an admin
  recovers through another member's backup, obtains current and historical
  bytes, revokes the used sheet and refuses its reuse. It also verifies
  advice before and after preparing the sheet and refusal before a backup
  joins.
- A real offline-backup test passed in 141.43 seconds, including the full
  two-minute receipt deadline: the unprinted key is revoked and advice
  remains for the sole admin computer. Its first fixture incorrectly used
  the admin's own backup, a second signing device; it was corrected to a
  member-owned backup before the passing run.
- A signed-roster-history test changes backup devices, checks the refresh
  advice, then prepares a current sheet and checks that advice clears.
- Rendering tests preserve the exact code in French, exercise excessive QR
  capacity and preserve its text fallback. The 1000-case recovery codec law
  includes a misleading `pgr1-` word in the labels; another regression
  covers a complete different code embedded in a user-supplied label.
- Router tests prove forged forms never dispatch sheet creation, names
  cannot inject markup, the French preparation action is discoverable,
  and the resulting page is not cacheable.

## Independent normal-build usability

A returning agent who did not implement the changes ran 52 checks against
normal 0.4.5 binaries, using a real laptop and backup, browser forms and
printing. All passed. This was a task test, not a blinded novice study.
The sole-admin recommendation and preparation action were discoverable;
refusal before adding a backup took 0.092 seconds and added no paper key.
After the backup joined, English and French sheets worked, advice cleared,
and two preparations on the same date had distinct public names.

An independent zxing-cpp decoder recovered the exact respective
541-character `pgr1-` codes from the browser QR and all four A4/Letter PDF
rasters. Normal sheets were one page with complete text and no clipping.
A deliberately oversized valid group label produced a 6,949-character code
with the explicit QR fallback and its complete text over three A4 pages.
Private QR, key and PDF evidence stays outside the repository. All owned
processes were stopped.

These changes do not complete the other M4 gates or claim sustained nightly
or soak evidence.
