# pigeon

pigeon keeps a group's files on everyone's computers, without a central
server, and tells you whether they are safe.

> Alpha: keep another copy of anything you care about.

## Install

Linux (x86_64) or macOS (Apple silicon):

```sh
curl -fsSL https://github.com/dont-feed-the/pigeon/releases/latest/download/pigeon-install | sh
```

## Update

```sh
pigeon update
```

Apple Silicon Macs can also use the Homebrew tap:

```sh
brew tap dont-feed-the/pigeon https://github.com/dont-feed-the/pigeon
brew install pigeon
brew services start pigeon
```

## Use

```sh
pigeon create Family ~/Family    # start a group; prints the line to share
pigeon invite                    # prints that line again
pigeon status                    # are my files safe?
```

More: [using pigeon](docs/guide.md), [developing pigeon](docs/development.md).

[Quick start: your first custodian and Nextcloud bridge](docs/nextcloud-quick-start.md):
bootstrap a new group on a fresh server, connect each app as its own peer, and
review suggestions in the browser. See the [bridge reference](docs/nextcloud-bridge.md)
for details.

MIT OR Apache-2.0.
