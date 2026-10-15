# 0071. A save within ten seconds of an install goes beside it

- **Status:** Accepted, 15 October 2026, by Mario (chat, answering the
  question of [16 §16.4](../spec/16-open-questions.md): "yes, within
  10 s: a conflict, both versions kept").

## Context

An application that holds a file's old content in memory may save it just
after pigeon installed a newer version from another computer
([07 §7.5](../spec/07-capture-and-install.md)). Capture then records the
old content as the newest version: the other person's change leaves the
folder of everyone and remains only in history, and nobody is told. The
question was whether a save within seconds of an install should count as
concurrent with it.

The alternatives were to keep the rule as it is (simple, and the change is
still in history), or to compare the saved content with the old version
only, which misses an application that saves a further edit of the old
content.

## Decision

- A save of a path whose file was modified within 10 seconds after install
  wrote another version of it (or up to 2 seconds before install recorded
  it) is put **beside** the installed version: capture commits it at the
  path's first free conflict name (05 §5.3), with the saver's name and the
  time of the save, and leaves the installed version at the path.
- The laptop's own file is not overwritten: it is a `concurrent`
  divergence (07 §7.6), which `pigeon resolve` ends.
- The copy is an addition, judged as one (04 §4.3); where the person may
  not add it, the save is captured as any other.
- The time of each install is kept in memory; a restart forgets it.

## Consequences

- An application's stale save no longer hides another person's change:
  both versions show, as a conflict does, and the person chooses.
- A person who really edits a file within 10 seconds of receiving it gets
  a copy rather than a new version. The lab's people wait 12 seconds
  before such a save (13 §13.12).
- The copy is a file of its own in the catalogue, not a slot with two
  versions: the merge of 05 §5.2 cannot express the save as concurrent,
  since its content may be exactly the version the install replaced.
