#!/usr/bin/env bash
# E15: "free up space" per folder without pointer files, with git sparse-checkout.
# Drop the folder's keys (verified), hide it; to bring it back, get the keys first,
# show it, then let git-annex smudge it. Asserts each step; exits non-zero on failure.
set -euo pipefail
export PATH=~/git-annex.linux:$PATH
# git isolated from the person's configuration (E20), with a lab identity for git-annex's commits
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 GIT_AUTHOR_NAME=lab GIT_AUTHOR_EMAIL=lab@pigeon \
  GIT_COMMITTER_NAME=lab GIT_COMMITTER_EMAIL=lab@pigeon
L=${LAB:-$HOME/ga-lab15}; chmod -R u+w "$L" 2>/dev/null || true; rm -rf "$L"; mkdir -p "$L"; cd "$L"
fail() { echo "FAIL: $*"; exit 1; }
ok() { echo "ok: $*"; }
objects() { find .git/annex/objects -type f 2>/dev/null | wc -l | tr -d ' '; }  # BSD wc pads

git init -q -b main A; cd A; git annex init -q A; git config annex.addunlocked true
mkdir Photos Docs; echo photo-1 > Photos/a.jpg; echo doc-1 > Docs/b.txt
git annex add -q .; git -c user.name=lab -c user.email=lab@pigeon commit -qm init; cd ..
git clone -q A B; cd B; git annex init -q B; git config annex.addunlocked true
git annex get -q .
[ "$(objects)" = 2 ] || fail "B should hold 2 objects"; ok "B holds both files"

# Free up space for Photos: drop its keys while it is still checked out, then hide it.
key=$(git annex lookupkey Photos/a.jpg)
git annex drop -q Photos
[ "$(objects)" = 1 ] || fail "drop of Photos left $(objects) objects"
git sparse-checkout set --no-cone '/*' '!/Photos/'
[ ! -e Photos ] || fail "Photos still on disk"; ok "Photos freed and hidden"
git annex whereis --key="$key" | grep -q ' -- A ' || fail "A no longer holds the photo"; ok "A still holds it"

# A hidden folder is invisible to path-based commands: dropping by path does nothing.
git annex find --in here | grep -q '^Photos/' && fail "find still lists Photos"
ok "path-based commands skip the hidden folder (work by key)"

# Edits outside the hidden folder still capture and sync.
sleep 1; echo doc-2 > Docs/b.txt; git annex add -q Docs
git -c user.name=lab -c user.email=lab@pigeon commit -qm edit
git annex sync -q --content origin >/dev/null 2>&1
(cd ../A && git merge -q synced/main && [ "$(cat Docs/b.txt)" = doc-2 ]) || fail "edit did not reach A"
ok "edits elsewhere still sync"

# Bring Photos back complete: get the keys first, show the folder, then smudge.
git annex get -q --key="$key"
git sparse-checkout add /Photos/
git annex smudge --update >/dev/null 2>&1
[ "$(cat Photos/a.jpg)" = photo-1 ] || fail "Photos/a.jpg is a pointer: $(head -c 60 Photos/a.jpg)"
git annex restage >/dev/null 2>&1
[ -z "$(git status --porcelain)" ] || fail "git status not clean: $(git status --porcelain)"
ok "Photos is back with its content, status clean"
echo "E15 passed"
