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
pigeon create Family ~/Family    # start a group with a folder
pigeon invite Laurent            # prints a line for Laurent to run
pigeon status                    # are my files safe?
```

More: [using pigeon](docs/guide.md), [developing pigeon](docs/development.md).

MIT OR Apache-2.0.
