# File manager usability checkpoint

On 16 October 2026, an independent agent participant used the file manager
from PR #64, combined with the replication fix at `0dcd400`. The user accepts
agent participants for usability evaluation. This participant had used earlier
pigeon interfaces but had not seen this layout, and did not inspect its
implementation. The build included lab support without injected faults.

Using three isolated local peers, the participant found a file from Home,
explained its protection and current version, filtered and sorted the table,
opened details, downloaded version 1, restored it, read the folder README and
followed its internal link. They also freed another folder and kept it again,
correctly distinguishing local storage choices from deletion and erasure.
Every task succeeded on the first attempt without product assistance. Ease
was rated 6/7; this is one returning participant's evaluation, not a population
measurement or evidence about network partitions.

An independent byte comparison confirmed that the restored file, the downloaded
version and the re-kept folder's file matched the exact original binary bytes,
including its NUL, non-UTF-8 byte and line endings. The browser and all fixture
peers were stopped and reaped.

The session found that “Open a copy” downloads a file; the rollup now says
“Download a copy” in English and French. A README link to a text file still
opens its version page rather than a text reader. Immediate browser-helper snapshots sometimes preceded asynchronous
updates; a short wait showed the result without another click. This session
does not isolate browser-helper timing from product latency.
