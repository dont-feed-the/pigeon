#!/usr/bin/env bash
# Builds pigeon's git-annex (ADR 0069) into OUTDIR: the release pinned in
# VERSION, fetched from Hackage and checked against its sha256, with the
# patches of patches/series applied in order, built by Stack with the
# release's own stack.yaml and flags, less the assistant and the benchmarks,
# which pigeon never runs. OUTDIR gets `git-annex` and the names it
# dispatches on (CmdLine/Multicall.hs). Linux x86_64 and macOS arm64. A cold build takes
# about an hour on a GitHub runner; run it there, not on a machine in use.
# Usage: build.sh OUTDIR
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
out=$(mkdir -p "${1:?usage: build.sh OUTDIR}" && cd "$1" && pwd)
# shellcheck source=VERSION
. "$here/VERSION"
work=${PIGEON_GIT_ANNEX_WORK:-$HOME/.cache/pigeon-git-annex}
mkdir -p "$work/bin"

sha256() { if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1; }
fetch() { # URL FILE SHA256
  [ -f "$2" ] && [ "$(sha256 "$2")" = "$3" ] && return
  curl -fsSL --retry 3 -o "$2" "$1"
  [ "$(sha256 "$2")" = "$3" ] || { echo "error: $1 does not match its pinned sha256" >&2; rm -f "$2"; exit 1; }
}

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) plat=linux-x86_64; stack_sha=$STACK_SHA256_LINUX_X86_64 ;;
  Darwin-arm64) plat=osx-aarch64; stack_sha=$STACK_SHA256_OSX_AARCH64 ;;
  *) echo "error: no pinned Stack for $(uname -s)-$(uname -m)" >&2; exit 1 ;;
esac
stack=$work/bin/stack-$STACK_VERSION
if [ ! -x "$stack" ]; then
  st=$work/stack-$STACK_VERSION-$plat.tar.gz
  fetch "https://github.com/commercialhaskell/stack/releases/download/v$STACK_VERSION/stack-$STACK_VERSION-$plat.tar.gz" "$st" "$stack_sha"
  tar -C "$work" -xzf "$st"
  cp "$work/stack-$STACK_VERSION-$plat/stack" "$stack"
fi

tarball=$work/git-annex-$UPSTREAM_VERSION.tar.gz
fetch "$TARBALL_URL" "$tarball" "$TARBALL_SHA256"
src=$work/src
rm -rf "$src" && mkdir -p "$src"
tar -C "$src" -xzf "$tarball"
cd "$src/git-annex-$UPSTREAM_VERSION"
while read -r p; do
  [ -n "$p" ] && patch -p1 --forward -s < "$here/patches/$p"
done < "$here/patches/series"

# Build/Version.hs: a release build takes its version from CHANGELOG alone,
# never from the git commit of whatever checkout holds the work directory.
export RELEASE_BUILD=1
# A --flag on the command line replaces every flag stack.yaml sets for the
# package, so the two that pigeon turns off are turned off in stack.yaml.
sed -i.orig -e 's/^    assistant: true$/    assistant: false/' \
  -e 's/^    benchmark: true$/    benchmark: false/' stack.yaml
grep -q '^    assistant: false$' stack.yaml && grep -q '^    benchmark: false$' stack.yaml
"$stack" --no-terminal setup
"$stack" --no-terminal build --copy-bins --local-bin-path "$out"
for name in git-annex-shell git-remote-annex git-remote-p2p-annex git-remote-tor-annex; do
  ln -sf git-annex "$out/$name"
done
got=$("$out/git-annex" version --raw)
[ "$got" = "$PIGEON_VERSION" ] || { echo "error: built $got, expected $PIGEON_VERSION" >&2; exit 1; }
echo "built git-annex $got into $out"
