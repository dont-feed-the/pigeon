# Report: supported folder sharing rules

M4 progress, version 0.4.10, 16 October 2026. Previously, exercising an owned
folder required editing a signed-roster fixture. Admins can now configure
ownership and add-only stewards through the CLI and the browser's Files →
folder → Sharing rule. This patch does not complete M4.

## Behavior

The CLI lists all rules or a folder's effective rule. It accepts a current
person's unique name or ID, or `me`. Add-only with no explicit stewards
means the current admins. Reset removes only the direct scope, preserving
ancestor and descendant rules. Ordinary members can read rules but cannot
change them; recovery keys cannot change them either.

The change is signed from the accepted tree, verified through the normal
authority judge and published with a compare-and-swap. Only roster metadata
is staged. Unpublished working files and unrelated index entries are not
included in that commit. Web forms require the session's CSRF token, work
without JavaScript, and send selected person identities to the same API.

Rules version 1 already matches literal path prefixes. Applying portable
case folding globally here would change the meaning of existing signed
history, contrary to ADR 0048. This editor preserves that behavior, reports
the effective literal rule, and refuses replacing a saved scope through a
case or Unicode alias. Any future change to authority scope matching needs
an explicit rules-version compatibility transition. No new rules version
or conformance claim is made here.

## Verification

- Five core laws run 1000 cases each: admin-only edits, preservation of
  unrelated roster state, idempotence, inherited reset, and refusal of
  malformed paths, unknown people and scope aliases.
- A 1000-case protocol law preserves identities, Unicode names, implicit
  admin stewards, direct/inherited scope and editability across split reads.
  The existing framing laws include both new requests and the response.
- Web tests check repeated steward fields, CSRF refusal, escaped names and
  a French ordinary member's read-only view. All 22 web and 12 i18n tests
  pass.
- A real two-laptop test configures an owned folder, refuses an ordinary
  member's rule edit, holds their content change as a suggestion, then
  changes to add-only with that person as steward and accepts their next
  edit. A private open working file and an unrelated staged entry keep
  their exact bytes and index identity across a roster-only reset. The
  final test passes in 10.16 seconds. An initial fixture update raced
  Git's index lock; the fixture now retries only that specific lock error.

Normal-build independent usability and the complete pre-commit checks
remain pending and will be recorded before the patch is marked ready.
