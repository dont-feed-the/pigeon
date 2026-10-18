# Recovering a lost native listener

> Mission: restore incoming metadata service after actual helper loss while
> retaining group-lifetime guards, admission and the native durability barrier.

A living git-annex remotedaemon can outlive its transport's listener helper.
The ordinary round checks whether the parent is alive, so it does not restart
that parent when the helper's local registration has ended. The transport
then continues refusing incoming peers as not-listening. A peer fetch can
also keep the ordinary round waiting before it next checks native children.

The native regression kills only the disposable backup's actual listener
helper after a real admitted connection and initial protected seed. The first
run fails in 35.177 seconds waiting for a replacement. An instrumented
unchanged-source run fails again in 38.282 seconds; its retained observation
records pigeon PID 3805890, terminated helper 3806479 and native parent
3806467, still alive after helper termination. Fixture .tmpm6oWz6 and its
listener-loss.json are retained locally. Neither observation terminates a
person's service or introduces a production fault-injection switch.

The registry now retains one generation-bound loss until recovery claims it.
A replacement registration clears the loss; an obsolete helper's teardown
cannot withdraw a newer registration. A small owned supervisor checks only
actual losses, outside slow peer-fetch rounds. It skips a busy group rather
than delaying other groups, rechecks identity/leaving under the group lock,
then stops and starts the group's owned native backend. Initial absence is
not a failure. Retired loss entries are pruned. The worker is aborted and
joined before intentional daemon shutdown, preventing recovery from racing
that teardown. Failed spawning retains the ordinary round's existing retry;
each confirmed loss is claimed once, and no content authority is added.

The first retake passes its 1000-case registration-order law and the actual
native case in 6.488 seconds total, including setup and initial protection.
The test requires a different helper PID and a real admitted connection,
while the same pigeon process stays alive. Group leave/refounding, the
broader native checks and final Clippy remain separate validation below.

The retained v0.6.19 fixtures include not-listening responses and stalled
metadata/transfer sessions. They do not distinguish missing registration
from an unreachable registered Unix socket, or establish this precise helper
loss interleaving. This independently reproduced recovery gap is not claimed
as the exact cause of that candidate's failures. It does not establish HDD
budgets, full global scheduling or actual elapsed M7 gates.

## Broader validation and integration

All 157 daemon library checks pass, including the 1000-case registration
law and existing transport/priority contracts. Seven native leave,
refounding/restart and held-deletion checks pass in 42.874 seconds. These
exercise group removal and repository replacement alongside the new owned
worker. Formatting, all-target workspace Clippy and file/header, ADR-index
and spec-reference checks pass.

Current v0.6.20 main `2d670e88ae1b5402472d659da33c51612b7f59ad` is
integrated before this branch's PR validation. Its differences from the
local native-test parent are release version/lockfile and report/changelog
metadata; Rust sources compare equal. Final all-target Clippy and structural
checks run again against this integrated tree before committing.

The process-list fixture explicitly requests unlimited argument width on
BSD/macOS as well as Linux, so a long helper path cannot hide its listener
arguments. A local retake checks that fixture adjustment; a hosted Mac
runtime verdict remains separate periodic evidence.
