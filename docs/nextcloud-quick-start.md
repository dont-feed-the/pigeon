# Quick start: your first custodian and Nextcloud bridge

> Mission: bootstrap a new Pigeon group on its first server, run its custodian
> and bridge, and connect clients that can review changes in the browser.

Start here with a fresh server and no existing Pigeon network. You will finish
with a custodian storing the group's files, a bridge website, and a server
address that Nextcloud clients can connect to. Client devices do not need Pigeon.

**Only connecting a client?** Ask the server operator for the bridge address,
your username, and your password, then skip to [Connect your clients](#connect-your-clients).

This guide uses **Ubuntu 24.04, x86_64, Bash, and systemd**. Run the commands
under one normal Linux account, using `sudo` only where shown. Pigeon and the
bridge are alpha software; start with test files and keep another copy.

## What will run on the server

| Component | What it does |
|---|---|
| Custodian | Keeps verified file versions for the group. |
| Personal control peer | Creates the group, invites clients, and handles your review decisions. |
| Bridge | Serves Nextcloud file sync and the browser portal; runs a distinct Pigeon peer for each authorized client. |

**Bootstrap limitation:** Pigeon's current `create` command always creates a
personal peer. There is no command to create a new group directly as a lone
custodian. The steps below create that control peer and the custodian together
on this first server. You do not need an existing laptop or another machine.
The control peer stays running because the bridge needs it for your identity
and review permissions.

All server data goes under `~/.pigeon`. Put this directory on the disk you want
to use for storage before starting. Its custodian repository, bridge state, and
content pool must share a filesystem to share file contents without copying.
Use this directory layout throughout the guide; arbitrary longer paths can
exceed Unix socket path limits.

## 1. Install Pigeon and the bridge

Install the build tools:

```sh
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev git curl ca-certificates
```

Install [Rust with rustup](https://rustup.rs) if needed, then open a shell where
`cargo` works. Cargo selects the Rust version pinned by Pigeon.

```sh
git clone https://github.com/dont-feed-the/pigeon.git "$HOME/pigeon"
cd "$HOME/pigeon"
cargo build --release --locked -p pigeon -p pigeond -p pigeon-bridge -p git-annex-p2p-iroh
```

If you already have this checkout, build from there instead. Install the four
matching programs together and the repository's pinned official Git Annex:

```sh
PIGEON_PREFIX="$HOME/.local/share/pigeon-bridge-tools"
mkdir -p "$PIGEON_PREFIX/bin"
install -m 755 target/release/{pigeon,pigeond,pigeon-bridge,git-annex-p2p-iroh} "$PIGEON_PREFIX/bin/"
packaging/install-git-annex --stock --prefix "$PIGEON_PREFIX"
export PATH="$PIGEON_PREFIX/bin:$PATH"
pigeon --version
```

Keep the programs together. The `--stock` option is supported for source
setups. If the Git Annex checksum check fails, use an updated pin from the
project rather than disabling the check. The release installer does not yet
install the bridge.

## 2. Start the custodian and control services

Create private state directories and disable desktop notifications on this
headless server:

```sh
umask 077
mkdir -p "$HOME/.pigeon/"{control,custodian}/config
mkdir -p "$HOME/.config/systemd/user"
printf 'notifications = false\n[loops]\ninterval_seconds = 2\n' > "$HOME/.pigeon/control/config/config.toml"
printf 'notifications = false\n[loops]\ninterval_seconds = 2\n' > "$HOME/.pigeon/custodian/config/config.toml"
```

Create the control service:

```sh
cat > "$HOME/.config/systemd/user/pigeon-control.service" <<'EOF'
[Unit]
Description=Pigeon personal control peer

[Service]
ExecStart=%h/.local/share/pigeon-bridge-tools/bin/pigeond --home %h/.pigeon/control
Restart=on-failure

[Install]
WantedBy=default.target
EOF
```

Create the custodian service. Its separate home gives it a different identity:

```sh
cat > "$HOME/.config/systemd/user/pigeon-custodian.service" <<'EOF'
[Unit]
Description=Pigeon custodian

[Service]
ExecStart=%h/.local/share/pigeon-bridge-tools/bin/pigeond --home %h/.pigeon/custodian
Restart=on-failure

[Install]
WantedBy=default.target
EOF

systemctl --user daemon-reload
systemctl --user enable --now pigeon-control.service pigeon-custodian.service
sudo loginctl enable-linger "$USER"
```

Linger keeps these user services running when you sign out and starts them
after reboot. Both daemons must run as the same Linux account as the bridge:
their local sockets admit only that account.

## 3. Create the group and enroll its first custodian

In the same shell, set convenient names for the two local sockets. In a new
shell, repeat these exports and the `PATH` export from step 1:

```sh
export CONTROL_SOCKET="$HOME/.pigeon/control/run/pigeond.sock"
export CUSTODIAN_SOCKET="$HOME/.pigeon/custodian/run/pigeond.sock"
```

Allow a few seconds for startup. Create the group through the control peer,
then make a one-time invitation for its first custodian:

```sh
PIGEON_SOCKET="$CONTROL_SOCKET" pigeon create Family "$HOME/.pigeon/control-files" --k 1 --name server-control
PIGEON_SOCKET="$CONTROL_SOCKET" pigeon invite --backup-machine server-custodian
```

The invitation prints a `pgn1-…` code. Copy that complete code into the command
below, replacing `PASTE_THE_INVITATION_CODE`. Run it **on this same server**,
this time addressing the custodian daemon:

```sh
PIGEON_SOCKET="$CUSTODIAN_SOCKET" pigeon join --backup-machine 'PASTE_THE_INVITATION_CODE' "$HOME/.pigeon/storage.git"
PIGEON_SOCKET="$CONTROL_SOCKET" pigeon machines
```

You should see `server-control` and `server-custodian`, with the latter shown
as a backup machine. `storage.git` is the custodian's managed repository;
upload files through clients rather than copying files into it.

Tell the control peer not to keep an ordinary working copy once files are
protected by the custodian:

```sh
PIGEON_SOCKET="$CONTROL_SOCKET" pigeon free "$HOME/.pigeon/control-files"
```

This uses Pigeon's normal safety checks. The control peer can retain local
content while it waits for the custodian to verify it.

The example uses **`--k 1`**: one custodian's verified copy is enough for Pigeon's
protected status. Everything on this server still shares one failure domain.
If your goal is two independent custody copies, choose **`--k 2` at creation**
and add a custodian on another machine later; protection will wait for both.

## 4. Configure the bridge to use the custodian

Choose the password for your first portal account, called `alice` here. The
commands hide your typing and put only a password hash in the configuration:

```sh
read -r -s -p 'Choose a bridge password: ' bridge_password
printf '\n'
bridge_password_hash=$(printf '%s' "$bridge_password" | pigeon-bridge hash-password)
unset bridge_password

cat > "$HOME/.pigeon/bridge.toml" <<EOF
listen = "127.0.0.1:8787"
public_url = "http://127.0.0.1:8787"
state_dir = "$HOME/.pigeon/bridge"
pigeond = "$HOME/.local/share/pigeon-bridge-tools/bin/pigeond"

[users.alice]
password_hash = "$bridge_password_hash"
control_socket = "$CONTROL_SOCKET"
storage_socket = "$CUSTODIAN_SOCKET"
EOF

chmod 600 "$HOME/.pigeon/bridge.toml"
unset bridge_password_hash
```

`control_socket` supplies your identity and review permissions; `storage_socket`
supplies the custodian's file contents. They are deliberately different.
`alice` is a bridge login name; it represents the Pigeon person created by
the control peer, initially named after the Linux account running it.

The shared content pool defaults to `~/.pigeon/bridge/content`. Clients have
different keys and metadata while sharing immutable contents with this pool
and the custodian. Adding clients does not add a full stored copy per peer.
Temporary uploads and the control peer can still need additional space.

Create and start the bridge service:

```sh
cat > "$HOME/.config/systemd/user/pigeon-bridge.service" <<'EOF'
[Unit]
Description=Pigeon Nextcloud bridge
Wants=pigeon-control.service pigeon-custodian.service
After=pigeon-control.service pigeon-custodian.service

[Service]
ExecStart=%h/.local/share/pigeon-bridge-tools/bin/pigeon-bridge --config %h/.pigeon/bridge.toml
Restart=on-failure
KillMode=control-group

[Install]
WantedBy=default.target
EOF

systemctl --user daemon-reload
systemctl --user enable --now pigeon-bridge.service
systemctl --user status pigeon-control.service pigeon-custodian.service pigeon-bridge.service
```

The bridge starts with both native services but can keep serving cached files
if the custodian is temporarily stopped. New authorizations and review need
the control peer.

All three services should be running. On a server with a browser, open
**<http://127.0.0.1:8787>**, sign in as **`alice`**, and look for **Your clients**.
For a headless server, continue to HTTPS setup before opening the portal from
another device. `127.0.0.1` in your laptop's browser refers to your laptop.

## 5. Give clients an HTTPS address

Choose a name such as `files.example.com` that points to this server. For a
public server, arrange DNS and allow inbound ports **80 and 443** through its
firewall or router. Clients need to trust the HTTPS certificate.

One simple reverse proxy is [Caddy](https://caddyserver.com/docs/install):

```sh
sudo apt-get install -y caddy
sudoedit /etc/caddy/Caddyfile
```

Add this site, replacing the example name with your actual domain:

```caddyfile
files.example.com {
    reverse_proxy 127.0.0.1:8787
}
```

Caddy obtains the public certificate when DNS and connectivity are set up.
Apply the configuration:

```sh
sudo caddy validate --config /etc/caddy/Caddyfile
sudo systemctl reload caddy
```

Edit `~/.pigeon/bridge.toml` and change its `public_url` to your actual address:

```toml
public_url = "https://files.example.com"
```

Keep `listen = "127.0.0.1:8787"`, then restart the bridge:

```sh
systemctl --user restart pigeon-bridge.service
```

Open **`https://files.example.com`** from a client device and check that portal
sign-in works. Use this exact address for both the portal and Nextcloud apps,
without adding a path such as `/nextcloud`.

With another proxy, preserve the original `Host` header, allow WebDAV methods,
and set upload size limits and timeouts to suit your files. A private-network
deployment also needs a reachable name and a certificate its clients trust.

## Connect your clients

Use a Nextcloud app that supports browser-based login (Login Flow v2). Client
users need just the HTTPS address, their bridge username, and their password.

1. In the app, choose **Add account** or **Log in to your Nextcloud** and enter
   the bridge address, for example `https://files.example.com`.
2. Let it open the browser. Sign in to **Pigeon Bridge**, for example as `alice`.
3. Give this installation a recognizable name such as **Alice laptop** or
   **Alice phone**, then select **Connect this client**.
4. Return to the app. It receives its own credential automatically. On a
   desktop client, choose a local folder for synchronized files.
5. Repeat the entire authorization for each additional installation.

Each authorization creates a **different Pigeon peer**, with its own identity
and pending changes. Your own devices can use the same portal login, but must
authorize separately. Copying an app's saved credential reuses the same peer.
The portal password is for browser sign-in; it is not the app-specific password
the bridge issues to the client.

Add a new text file in one app and let it synchronize. Connect a second app and
check that the file arrives. In the portal, each app should have a different
peer ID. On the server, the operator can check the file's protection status
after the custodian receives it:

```sh
PIGEON_SOCKET="$CONTROL_SOCKET" pigeon status
PIGEON_SOCKET="$CONTROL_SOCKET" pigeon machines
```

With `--k 1`, the uploaded version can become protected once this custodian
acknowledges it. A client sync checkmark alone means the bridge received the
file. Bridge client peers are not additional independent backups.

This is file sync; calendars, contacts, sharing links, and other full Nextcloud
server features are unavailable. The command-line Nextcloud client is exercised
by the integration checks; desktop and mobile releases need their own
[compatibility checks](nextcloud-bridge.md#current-limits).

## Use the website to review changes

Open the same bridge HTTPS address in a browser. Nextcloud apps do not show
Pigeon's approval workflow.

- **Clients** lists your separately authorized devices. Open a device's name
  to use the same Pigeon interface as a regular peer: **Home** shows protection
  status, **Files** offers downloads, history and restore, **Space** explains
  storage, and **Settings** changes that client's preferences.
- **Suggestions** lists proposed changes. People allowed to decide see
  **Accept** and **Decline**; other people see that the change is awaiting review.
- **Take the group's version** replaces a client's differing version.
  **Keep mine as a copy** saves it under another name. **Suggest again**
  submits it for another review. Find these under **Manage client and conflicts**,
  linked from every client peer page.
- **Revoke this client** disables that installation's access. Other clients keep
  their identities and access. Authorizing it again creates a new peer.

Each peer page names the selected device. Actions there use that device's
Pigeon identity; the portal's top-level **Suggestions** link uses your personal
control peer. Choose files to keep offline inside the Nextcloud app. The bridge
manages the peer's group and its sparse server storage, so those local setup and
pinning controls are omitted from its Pigeon pages.

For example, an edit to someone else's file can wait as a suggestion. The app
that uploaded it sees its own saved version while other clients see the accepted
group version. Accept the suggestion to let the group receive the change.

For each additional person, add a separate personal control peer on this server,
join it to the group with an invitation for that person, and configure their own
bridge account. They can share this custodian through `storage_socket`. Sharing
Alice's login would make everyone's apps Alice's Pigeon devices. See the
[account setup reference](nextcloud-bridge.md#build-and-configure).

## Keep the server's identities and files

Keep `~/.pigeon` across restarts. It contains the custodian's repository, the
control peer, device keys, bridge client records, and the shared content pool.
Old pooled versions remain stored; automatic pool garbage collection is not
implemented, and revoking a client does not reclaim those versions.

To take a consistent backup, stop all three services, back up that directory
while preserving hard links, then start the services again:

```sh
systemctl --user stop pigeon-bridge.service pigeon-control.service pigeon-custodian.service
# Back up ~/.pigeon to a separate storage location here.
systemctl --user start pigeon-control.service pigeon-custodian.service pigeon-bridge.service
```

Keep your backup outside this server. If you move the content pool with
`content_dir`, include it too. Save a recovery sheet securely using
`PIGEON_SOCKET="$CONTROL_SOCKET" pigeon recovery-sheet`.

To add a custodian on another machine later, make another invitation with
`PIGEON_SOCKET="$CONTROL_SOCKET" pigeon invite --backup-machine` and use its
join command on that machine. The [Pigeon user guide](guide.md#use-it-with-other-people)
describes ordinary peer and backup-machine setup.

## If something does not work

| Symptom | What to check |
|---|---|
| Browser or app cannot reach the server | Use its HTTPS domain, not `127.0.0.1` on a different device. Check DNS, ports 80/443, and the certificate. |
| A service does not start | Read its journal below. Check executable paths, file permissions, and that all services run under the same Linux account. |
| A CLI command cannot reach Pigeon | Repeat the socket exports from step 3. Use `CONTROL_SOCKET` for creating/inviting/reviewing and `CUSTODIAN_SOCKET` for the custodian's join. |
| A client cannot finish connecting | Check both native services, the two socket paths in `bridge.toml`, group membership, and matching binary versions. |
| An upload is visible on only one client | Check **Suggestions** and the submitting client's page for a pending change. |
| Sync works but protection is pending | Check the custodian is online and has storage space; with K = 2, a second independent custodian is also required. |
| A bulk deletion gets HTTP 409 | The bridge refuses batches deleting more than 100 existing files and paths held for review; see its [limits](nextcloud-bridge.md#current-limits). |

Read service logs with:

```sh
journalctl --user -u pigeon-control.service -u pigeon-custodian.service -u pigeon-bridge.service -f
```

A managed client's startup log is under
`~/.pigeon/bridge/peers/<client-id>/daemon.log`. The
[bridge reference](nextcloud-bridge.md) and
[example configuration](../packaging/bridge.example.toml) cover the remaining options.
