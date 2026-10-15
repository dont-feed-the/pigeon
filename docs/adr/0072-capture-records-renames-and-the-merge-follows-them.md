# 0072. Capture records renames, and the merge follows them

- **Status:** Accepted, 15 October 2026, by Mario (chat, answering the
  question of [16 §16.4](../spec/16-open-questions.md): "yes, with git's
  similarity").

## Context

A rename was a deletion and an add ([05 §5.2](../spec/05-catalogue-and-names.md)):
an edit made on another computer while the file was being renamed stayed
at the old name, so the group saw the renamed file without the edit and
the edit under the name nobody uses any more. Folders reorganised while
someone works lose that person's changes from view, and nobody is told.

git detects renames by similarity when it compares two trees, but only
as a guess at display time; the catalogue, a pure function of accepted
commits, needs the rename recorded in history, and a rule that every
device applies alike.

## Decision

- **Capture** pairs, in one round, each file deleted from `main` with a
  file `main` lacks: first by content (the hash the git-annex keys name,
  whatever the extension), then, for files of at most 1 MiB whose two
  contents are here, by git's similarity estimate (chunks of a line or 64
  bytes, as `diffcore-delta.c`), when at least half of the larger file is
  in both. Each pair is a `Pigeon-Renamed: "old" "new"` trailer of the
  capture commit. Files of a package and pigeon's own files are never
  paired; a rename without a trailer stays a deletion and an add.
- **The merge** follows a rename it did not see. A write at *x*
  concurrent with a rename *x* → *y* goes to *y* when both build on one
  version of *x* (an event both saw, or a rename the write followed,
  wrote it), the writer saw no rename of *x*, nor writes or saw a write
  of *y*; the earliest such rename wins, and chains go on from *y*.
- **A version people saw never comes back:** an event replaces a write it
  saw where, in its own view, renames had taken it.
- **A mere move yields:** the rename's own copy at *y* gives way to a
  version that came there through it and saw every version it moved, so
  a concurrent edit wins over a pure rename and nothing is lost.
- A conflict event sees the renames that moved its heads, and a version
  a rename moved is allowed where its author changed it (04 §4.3).
- The oracle (`models/catalogue`), `pigeon-core::catalogue` and
  `Catalogue.tla` apply the same rules; the trailer is a hint, judged by
  no rule of 04 §4.4: the deletion and the add are judged as before.

## Consequences

- An edit concurrent with a rename lands in the renamed file; two
  concurrent renames of one file keep both names, and a concurrent edit
  follows the earlier.
- Two identical renames made concurrently can leave an extra copy; a
  version moved into an add-only folder makes its author a writer there.
- A rename with a large edit, or of a file whose old content is gone, or
  of more than 16 unpaired files at once by similarity, is still a
  deletion and an add.
- Every device computes the same catalogue: the rule depends only on the
  commits and who saw which (law `order`, 05 §5.2).
