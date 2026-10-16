# 0077. The web interface ships a bundled TypeScript client, built with Node

- **Status:** Accepted, 16 October 2026, by Guillaume (chat). Supersedes
  0021.

## Context

ADR 0021 gave pigeon two interfaces, the `pigeon` CLI and a web interface
served by `pigeond` on 127.0.0.1, rendered on the server without a
JavaScript framework, "small scripts only refresh status". Those scripts
were strings inside the Rust code. Guillaume found the result bare and
asked for a file browser in the manner of Nextcloud's: a sidebar, a table
of files with icons, sizes and dates, a details panel, actions without a
page load, filtering. That needs more script than a string can hold well,
type-checked and tested, and a stylesheet larger than an inline `<style>`.
He said JavaScript is fine as long as the application is bundled properly:
nothing fetched from elsewhere, a strict Content-Security-Policy, the
existing session and form checks kept. Node is an accepted build
dependency.

## Decision

- ADR 0021 stands for everything but its sentence on scripts: two
  interfaces with the same requests; the web interface listens on
  127.0.0.1 only, behind the login token, the session cookie, the
  `Host`/`Origin` check and a form token on every state change.
- Pages are still rendered on the server, in the person's language from
  the Fluent catalogue (ADR 0028), and still work without JavaScript.
  The client enhances them: live status, details and actions without a
  page load, filtering, local times. It holds no message of its own.
- The client is TypeScript in `crates/pigeon-web/client/`, bundled by
  esbuild and type-checked by `tsc`, both pinned in a committed
  `package-lock.json`, with the Node version in `.nvmrc`.
  `crates/pigeon-web/build.rs` installs them with `npm ci` and builds the
  bundle into `OUT_DIR` whenever the client's sources change; the binary
  embeds it. No framework: the pages are the server's.
- The script, stylesheet and icon are served under content-hashed names
  with a year's caching. Every page carries a Content-Security-Policy
  without `unsafe-inline` or `unsafe-eval`: scripts, styles, images and
  connections from this server only. Icons are inline SVG.

## Consequences

- Building pigeon from source needs Node 22.12 or later with npm, and
  the npm registry (or npm's cache) the first time; CI and the release
  workflow install Node. `pigeon-install --main` and `--source` check for
  it before building.
- The client's logic has its own tests (`node --test`), run by
  `cargo nextest` through pigeon-web.
- Supersedes 0021, whose other decisions this restates.
