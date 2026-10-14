# Problems

Every error that reaches a person is a *Problem* ([10 §10.6](spec/10-interfaces.md), ADR 0023):
a stable `code`, shown as `P-CODE`, a Fluent message and an optional hint,
rendered at the edge, and `details` for helpers. This file lists every code
the code may raise, with its English message; `pigeon-i18n`'s test
`docs_problems_lists_every_code` checks it against `ids::PROBLEMS`. Messages
are in `crates/pigeon-i18n/i18n/en/pigeon.ftl`; `{ $x }` is an argument.

| Code | Shown | Message (en) | Hint (en) | When | Spec |
|---|---|---|---|---|---|
| `not-in-this-build` | `P-NOT-IN-THIS-BUILD` | “{ $command }” is not in this build of pigeon yet. | This early build has only the commands that keep files safe; the others come later. | a command or option this build does not have yet | [13 §13.12](spec/13-verification.md) |
| `file-system-not-in-this-build` | `P-FILE-SYSTEM-NOT-IN-THIS-BUILD` | This folder is on { $fs }, which this build of pigeon does not support yet. | Choose a folder on ext4 (Linux) or APFS (macOS). | `create` or `join` on a file system this build refuses | [14](spec/14-platforms-and-packaging.md) |
| `invite-expired` | `P-INVITE-EXPIRED` | This invitation has expired (it was valid until { $until }). | Ask { $inviter } for a new one. | `join` with an invitation past its time | [09](spec/09-network-and-joining.md) |
| `invite-used` | `P-INVITE-USED` | This invitation has already been used. | Ask { $inviter } for a new one. | `join` with an invitation already used, or `recover` into a group this device has | [09](spec/09-network-and-joining.md) |
| `invite-unknown` | `P-INVITE-UNKNOWN` | This invitation is not one the group knows. | — | `join` with an invitation its inviter does not know | [09](spec/09-network-and-joining.md) |
| `invite-not-allowed` | `P-INVITE-NOT-ALLOWED` | { $inviter } may no longer invite to this group. | — | `invite` by someone who may not, or a recovery sheet that may not add the device | [04 §4.7](spec/04-roster-and-authority.md) |
| `inviter-offline` | `P-INVITER-OFFLINE` | { $inviter }'s computer can't be reached right now. | Try again when { $inviter }'s computer is on. | `join` or `recover` could not reach the inviter or a backup machine | [09](spec/09-network-and-joining.md) |
| `not-in-group` | `P-NOT-IN-GROUP` | This computer is not a member of the group. | — | a request on a group this device is not in | [10](spec/10-interfaces.md) |
| `not-allowed` | `P-NOT-ALLOWED` | You are not allowed to do this in this group. | — | a roster change its signer may not make | [04 §4.7](spec/04-roster-and-authority.md) |
| `needs-confirmation` | `P-NEEDS-CONFIRMATION` | This needs your confirmation, and pigeon may not ask here. | Run it again in a terminal, or add --yes. | a request that needs `--yes` | [10 §10.7](spec/10-interfaces.md) |
| `backup-machine-needs-linux` | `P-BACKUP-MACHINE-NEEDS-LINUX` | A backup machine must run Linux with systemd. | — | `join --backup-machine` elsewhere than Linux with systemd | [10 §10.7](spec/10-interfaces.md) |
| `disk-full` | `P-DISK-FULL` | { $machine } is full. | Free up space on it, or add another backup machine. | `doctor`, `status` and `restore`: free space below `annex.diskreserve` | [08 §8.9](spec/08-custody-loops.md) |
| `no-capacity` | `P-NO-CAPACITY` | { $machine } has no room for { $files } files: it needs { $missing_mb } MB more free space. | Free up space on it, or add another backup machine. | `doctor` and `status` on a custodian whose admission deferred keys | [08 §8.9](spec/08-custody-loops.md) |
| `commit-unsigned` | `P-COMMIT-UNSIGNED` | A change from { $machine } is not signed, so it was refused ({ $commit }). | — | `doctor` on a custodian that refused a peer's unsigned commit | [04 §4.4](spec/04-roster-and-authority.md) |
| `commit-forged` | `P-COMMIT-FORGED` | A change from { $machine } has a signature that does not match it, so it was refused ({ $commit }). | — | … one whose signature does not match it | [04 §4.4](spec/04-roster-and-authority.md) |
| `commit-unknown-signer` | `P-COMMIT-UNKNOWN-SIGNER` | A change from { $machine } was signed by a computer outside the group, so it was refused ({ $commit }). | — | … one signed by a device outside R(p) | [04 §4.4](spec/04-roster-and-authority.md) |
| `commit-not-allowed` | `P-COMMIT-NOT-ALLOWED` | A change from { $machine } changes what its person may not change, so it was refused ({ $commit }). | — | … one changing what its person may not | [04 §4.3](spec/04-roster-and-authority.md) |
| `commit-malformed` | `P-COMMIT-MALFORMED` | A change from { $machine } is not well formed, so it was refused ({ $commit }). | — | … one with an ill-formed tree or roster change | [04 §4.4](spec/04-roster-and-authority.md) |
| `commit-waits` | `P-COMMIT-WAITS` | A change from { $machine } waits for a newer pigeon ({ $commit }). | — | … a merge or rules this release does not judge yet (ADR 0048) | [04 §4.4](spec/04-roster-and-authority.md) |
| `repository-lost` | `P-REPOSITORY-LOST` | The files of { $machine } were lost or replaced. | — | a custodian whose repository is gone | ADR 0046 |
| `daemon-not-running` | `P-DAEMON-NOT-RUNNING` | pigeon is not running on this computer. | Start it, then check with: pigeon doctor | the CLI cannot reach `pigeond` | [10](spec/10-interfaces.md) |
| `version-mismatch` | `P-VERSION-MISMATCH` | This pigeon command and the running pigeon are different versions. | Restart pigeon. | the CLI and the running `pigeond` differ | [10](spec/10-interfaces.md) |
| `git-annex-too-old` | `P-GIT-ANNEX-TOO-OLD` | git-annex { $found } is too old; pigeon needs { $minimum } or newer. | — | `doctor`: git-annex below the minimum | [14 §14.3](spec/14-platforms-and-packaging.md) |
| `git-annex-untested` | `P-GIT-ANNEX-UNTESTED` | git-annex { $found } is newer than the versions pigeon was tested with. | — | `doctor`: git-annex newer than CI tests | ADR 0048 |
| `update-needed` | `P-UPDATE-NEEDED` | The group uses a newer pigeon. Update pigeon on this computer. | — | git below the minimum, or the group runs rules this release does not know | ADR 0048 |
| `pigeon-too-old` | `P-PIGEON-TOO-OLD` | This pigeon is too old for the group. | — | local files newer than this release | ADR 0036, ADR 0048 |
| `peer-too-old` | `P-PEER-TOO-OLD` | { $peer } runs a pigeon too old for the group. | — | a peer speaks no protocol version in common, older | ADR 0048 |
| `peer-too-new` | `P-PEER-TOO-NEW` | { $peer } runs a newer pigeon that this one can't follow. | — | … newer | ADR 0048 |
| `settings-check` | `P-SETTINGS-CHECK` | The group's settings on this computer were changed outside pigeon; freeing space waits until they are restored. | — | the settings check could not put a setting back | [03 §3.5](spec/03-repository-and-roles.md) |
| `receiving` | `P-RECEIVING` | A copy of this folder is still arriving. | Try again in a moment. | `keep` while a copy arrives | [10 §10.6](spec/10-interfaces.md) |
| `unknown-computer` | `P-UNKNOWN-COMPUTER` | The group has no computer named { $name }. | See the names with: pigeon machines | `remove-computer` with a name the roster lacks | [10 §10.6](spec/10-interfaces.md) |
| `usage` | `P-USAGE` | “{ $command }” was given the wrong arguments. | See: pigeon help | the CLI's arguments do not parse (exit code 2) | [10 §10.2](spec/10-interfaces.md) |
| `not-a-pigeon-repository` | `P-NOT-A-PIGEON-REPOSITORY` | This folder is not shared with pigeon. | — | a path that is no group's folder | [10 §10.6](spec/10-interfaces.md) |
| `peer-unreachable` | `P-PEER-UNREACHABLE` | The other machine can't be reached right now. | — | the transport helper could not reach the peer | [09 §9.3](spec/09-network-and-joining.md) |
| `peer-refused` | `P-PEER-REFUSED` | The other machine refused the connection. | — | the peer refused the stream | [09 §9.3](spec/09-network-and-joining.md) |
| `internal-error` | `P-INTERNAL-ERROR` | Something went wrong inside pigeon. | Gather the details with: pigeon debug bundle | anything else; details go to the debug bundle | [10 §10.6](spec/10-interfaces.md) |
