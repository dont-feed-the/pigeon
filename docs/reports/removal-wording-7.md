# Removal confirmation wording

The accepted ADR 0075 keeps one invitation per group. Removal refuses the
removed device identity and stops counting its copies; it does not change
the invitation. A holder can join under a new device identity. Refounding
is the accepted way to create a group with a new invitation.

Two independent simulated newcomer reviews, English and French, read the
existing CLI/web messages and guide before comparing the accepted design.
Both interpreted “You can invite it again later” as needing a fresh
invitation controlled by the remover. The guide already explains the
actual boundary. These are agent usability observations, authorized by the
user, not runtime tests or a new joining security review.

The five CLI/web removal confirmation messages in each language replace
that sentence with an explicit existing-invitation/new-computer statement.
Backup shortfall warnings, retained-file statements and copy-count
consequences remain intact. No authority, key, roster or wire behavior
changes. The removed-device headline's suggested next steps are outside
this narrow wording correction: joining with a new identity is distinct
from the same removed identity reconnecting.

Focused wording retakes and the existing Fluent message/argument/rendering
checks are recorded after execution below.

Both focused language retakes found the five revised confirmations clear
about the unchanged invitation and fresh computer identity. Both confirmed
that the existing insufficient-backup branches remain explicit. These are
simulated wording retakes, not runtime identity-transition evidence.
All 18 existing Fluent message, argument, plural and rendering checks pass.
All-target workspace Clippy with lab/trace/console features, formatting,
header/length, ADR and spec-reference checks pass before this commit.
