# Pigeon 0.5.0 current-main focused usability retake

The requested real-group workflow succeeded on the first attempt at every product step: create, non-admin Bob join, Bob-owned backup invitation/join, initial protection, later-edit protection, and one further browser join with exact current and old history bytes. No product failure, retry, source inspection, or hidden repair occurred. One participant is represented here: an experienced returning agent participant, explicitly authorized by the user, not five new first-use sessions.

## Candidate and method

- Frozen normal package: `/workspace/pigeon-m4-current-main-bin`; `pigeon --version` reports `pigeon 0.5.0`. All three actual binary hashes matched `/tmp/pigeon-m4-current-main-bin.sha256`. Recorded source-tree identity: `b8498168e46a23191b8856dd143dedddbb3b8064`, from the supplied `.tree` file. No rebuilds or binary changes.
- Current README and guide read from `/workspace/pigeon-m4-completion`, plus ordinary CLI help. An older guide was initially read before the parent identified the current docs, then the current guide was read before starting fixtures. No implementation files were read.
- Fresh private fixture `/tmp/pigeon-usability-050-current-main`, with Alice admin laptop, genuine ordinary-member Bob laptop, Bob-owned backup, and Camille laptop. K=1. Every membership came from a real invitation. No prior fixture or failed group was reused or modified.
- Complete lifecycle under `/workspace/pigeon-debug-tools/reap-run`; patched git-annex first on PATH, then system Git; TOKIO_WORKER_THREADS=2; each pigeon machine's HOME and all XDG paths isolated. Actual Chromium browser with separate contexts at 1280×960. Runtime network readiness checked with the cloud runtime skill before connected fixtures.
- Session: 2026-10-16 08:29:26–08:33:55 UTC. Timings below include agent orchestration, waiting, and evidence capture after each prompt. They are not human reaction benchmarks. Preparing isolated homes and confirming the binary manifest were facilitator setup. Actual create/invite/join/edit/browser actions were participant tasks.

## Outcomes and timing

| Task | Result / assistance | Elapsed | Ease (1 hard–7 easy) |
|---|---|---:|---:|
| A. Create admin group and file | Success, unassisted | 1.492s | 7 |
| B. Invite non-admin Bob and join laptop | Success, unassisted | 2.593s | 7 |
| C. Bob invites/owns backup; original becomes safe | Success, unassisted | 27.308s | 7 |
| D. Later edit becomes safe; original retained | Success, unassisted | 33.653s | 7 |
| E. Further real laptop joins via browser; bytes/history | Success, unassisted | 4.675s | 7 |

All required content/state checks succeeded. The generic harness recorded 38 checks: 37 true, plus one deliberately observed point-in-time probe that was false because Camille's file was absent at the membership response. That probe is evidence of asynchronous arrival, not a failed requirement; it is preserved unchanged with an explicit measurement clarification in the log. There were no CLI errors, timeout failures, assisted successes, or incorrect safety beliefs.

## Actual steps, messages, and bytes

Alice ran `pigeon open`, then the documented command:

```sh
pigeon create 'Current-main returning session' /PRIVATE/home-alice/Family --k 1 --name alice-laptop
```

`people` identified the creator as `agent (admin): alice-laptop`. I saved `plan.txt`. Home immediately said **“Checking 1 changed file; it is not safe yet.”**, with Safe: 0 and Only on this computer: 1. The CLI also explained that 0 of the required 1 backups were configured. I did not interpret saving or creating membership as protection.

Alice ran `pigeon invite Bob` with no `--admin`; Bob ran the supplied `pigeon join INVITATION /PRIVATE/home-bob/Family --name bob-laptop` once. `people` showed **“Bob (member): bob-laptop”**. Join took 0.670s and said:

> This computer is now in “Current-main returning session”. The group's files arrive in its folder.

Exact original bytes were confirmed 0.512s after join returned. Bob's Home showed **“Not safe yet: add a backup machine.”** and **Shared, not safe yet: 1**; his CLI state was `published`. This was genuine multi-laptop sharing without protection.

While still an ordinary member, Bob ran:

```sh
pigeon invite --backup-machine bob-backup
```

The real backup ran `pigeon open`, then `pigeon join --backup-machine INVITATION` once. The join exited 0 after 0.615s and said:

> This backup machine now keeps copies for “Current-main returning session”.

On that backup, `people` showed **“Bob (member): bob-backup, bob-laptop”** and `machines --json` marked `bob-backup` as `custodian: true`. Bob was never promoted or impersonated. Its exact original object was already present by the first physical check **0.764s after join returned**; that is an observation upper bound, not an exact transport timestamp. Immediately after backup join, Bob's Home showed **“Sending 1 file version to the backup machines…”**, **0 of 1 safe copies**. Alice still showed add-a-backup wording while the membership update propagated. Alice confirmed `protected` **25.766s after backup join returned**, followed by Bob. Home then showed **“Everything is safe. Checked just now.”** I did not count the backup's membership response as proof of safety.

Alice then saved a byte-distinct later accepted edit. Home immediately returned to **“Checking 1 changed file; it is not safe yet.”**, Safe: 0 and Only on this computer: 1. Exact edited content was found physically on Bob's backup **1.803s after saving**. Bob's laptop subsequently held the exact edit. Alice confirmed the new version protected **32.100s after saving**; Bob also confirmed protected. Both Homes then showed Current files Safe: 1 and Other versions being kept Safe: 1. Exact original and edited objects remained in the backup. Documented backup `view --version 1` and `view --version 2` returned separate cache copies outside the shared folder, with no write bits, matching the exact respective versions.

The byte fixtures included UTF-8, CRLF, NUL, and a distinct non-UTF8 marker so line-ending conversions or merely equal visible text would not pass:

| Version | Exact Python byte expression | Size | SHA-256 |
|---|---|---:|---|
| Original | `'Original café plan.'.encode() + b'\r\nExact bytes: \x00\xff\n'` | 38 | `a9f2884f4e5129a95071d7d047287bee45531ae46ea334fec10b12414fcacda3` |
| Later edit | `'Revised café plan.'.encode() + b'\r\nExact bytes: \x00\xfe\n'` | 37 | `4ec2b3c25102d20a8a46fe9a7b623896d84c99b135926545485912abe44df896` |

For the further laptop, Alice issued `pigeon invite Camille`. On a new isolated HOME, I ran `pigeon open`, chose **Join or create a group**, filled the invitation, absolute local folder, and computer name `camille-laptop`, then clicked **Join the group** once. The browser used the same explicit membership/files-arrive wording. The exact current file was **not present at that response**. The next normal content wait found it, and it was confirmed **0.776s after the browser join returned**, including the intervening screenshot/check overhead.

Camille's `people` identified her as a member and still showed Bob owning the backup. The first file status showed `published`/Sending while safety evidence arrived. Her history query shortly afterward listed exactly two accepted protected versions: number 2 saved at 08:31:27 UTC and number 1 saved at 08:29:51 UTC. Current `view --group`, old `view --version 1`, and edited `view --version 2` each returned exact expected bytes as separate read-only copies. Her final Home showed Everything is safe, Current files Safe: 1, and Other versions being kept Safe: 1. All three working laptops held exact edited bytes at final verification, and the Bob-owned backup retained both exact versions.

## My safety explanation

“Joined” tells me this computer has membership. It does not tell me the files are already on its disk. I saw that directly on Camille: membership succeeded while the file was absent, then the bytes arrived. I would check the actual file or Files before assuming I can work on it.

Joining a backup makes it a backup participant, but files become safe only when that backup actually receives/verifies the required version and the laptop receives recent safety evidence. Copying and confirmed protection are distinct: initial bytes were present within the first second, while Alice's protected result took about 26 seconds. A later edit needed its own copy and confirmation even though the old version was already safe. Bob's ownership and ordinary-member role did not prevent his real backup from protecting the group's files.

No incorrect safety belief occurred. This retake actually observed local, shared-without-backup, Sending, and protected states; it did not use messages alone as byte evidence.

## Friction and wording

No blocking usability failure occurred. Ordinary help/guide commands and the browser form were sufficient, without parent coaching or alternate commands after failure.

- Minor docs mismatch: the current guide's opening status still says **alpha (v0.3)** while this candidate reports 0.5.0. Update that label when publishing these docs.
- Transient membership propagation was visible: Alice briefly kept asking to add a backup while Bob already showed Sending after the successful backup join. It resolved through normal waiting; I did not add a duplicate backup. A clear reminder in backup join guidance that other computers need to receive its membership/copy evidence would reduce uncertainty.
- Camille's final Home also retained **“Not checked yet; pigeon may be busy.”** beneath the otherwise safe inventories. I did not treat this as lost or unsafe file content because the file counts/history and exact bytes were explicit, but identifying the pending check would make that residual line clearer.

These are wording observations from one returning participant, not invented failure or new first-use measurements.

## Private evidence and verified cleanup

Evidence is preserved under `/tmp/pigeon-usability-050-current-main` (0700); private transcript, snapshots, screenshots and raw commands in `private/` are 0600. `results.json` contains task timing and recorded observations; `file-manifest.json` records exact hashes. Useful snapshots: `A-created-no-backup`, `B-bob-member-no-backup`, `C-immediately-after-backup-join`, `C-initial-protected`, `D-immediately-after-edit`, `D-edit-and-retained-protected`, `E-further-join-form`, `E-further-join-returned`, `E-further-current-and-history`. Runtime logs remain in each HOME's `.local/state/pigeon/`. No invitation, login token, recovery key or private signing key is reproduced here.

All browser contexts closed normally. Before stopping each daemon, the harness verified its exact frozen `pigeond` executable and isolated HOME: Alice PID 3003938, Bob 3004728, Bob backup 3013887, Camille 3044392. All stopped with SIGTERM, without forced kill. The entire reap-run lifecycle exited **0**. A subsequent `/proc` check found **zero fixture processes**; `cleanup-verification.json` records the empty scan. Group repositories, logs and every previous failed fixture were preserved; other participants' processes were untouched.
