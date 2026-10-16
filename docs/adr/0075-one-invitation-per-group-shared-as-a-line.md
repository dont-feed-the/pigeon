# 0075. One invitation per group, shared as a line that installs and joins

- **Status:** Accepted, 16 October 2026, by Mario (design interview in
  chat). Supersedes 0017.

## Context

ADR 0017 made each invitation a one-time ticket for 24 hours, written by
the inviter for a named person or a backup machine. Adding a family meant
one ticket per person and per computer, each sent separately, and each
redeemed while its inviter was online. Mario wants one link to share with
everyone, and that link should also install pigeon. pigeon has no server:
a link can only be a line of text that someone pastes into a terminal.

## Decision

- **The roster holds the group's invitation**: 256 random bits in hex, the
  field `invitation` of `.pigeon/roster.toml`. `pigeon create` writes it in
  the genesis. It never changes: a roster change cannot express a new one,
  so a commit that changes it is invalid. A refounded group (ADR 0047) gets
  a new one.
- **The invitation is a line to share**: `curl -fsSL <installer> | sh -s --
  --join pgn2-…`. `pigeon create` prints it, and `pigeon invite` prints it
  again on any member. The `pgn2-` code carries the group ID and name, K,
  the relay, the secret, and its **admitters**: the device that wrote it,
  then the admins' laptops and backup machines, with the addresses that
  device knew.
- **The line installs, then joins**: the installer installs or updates
  pigeon, then runs `pigeon join` on the terminal. It asks for the folder
  and the person's name, the account's by default. `--backup-machine` at the
  end of the line joins a backup machine instead.
- **Whoever proves the secret joins.** The join exchange of ADR 0017 is
  kept: an HMAC bound to both device IDs. The admitter decides the roster
  change (`pigeon-core::invitation::admit`). A new name adds a person who
  is not an admin. The name of a person of the roster adds the device to
  that person, with their rights, admin included, once the joiner confirms.
  A device of the roster that asks again gets its welcome again. A removed
  device is refused. The authority over device entries of ADR 0017 is kept:
  an admin adds or removes any device, and a device of person P adds or
  removes devices of P. The admitter signs under it, and the joiner asks the
  next admitter when one may not sign.
- `pigeon invite PERSON`, `--admin`, `--backup-machine`, `--list`,
  `--cancel`, `invitations.json` and the `pgn1-` codes are removed.

## Consequences

- One line adds everyone, and any admin's computer that is on can admit.
- The line is the key to the group. Whoever has it can join under any
  person's name and gets that person's rights. It must be shared only with
  the group.
- The secret never changes: removing someone does not stop them from
  joining again with the same line. Only refounding the group (ADR 0047)
  gives a new invitation. Every join is still a signed, visible roster
  commit.
- No group older than this decision is supported: none was in use.
