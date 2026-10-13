#!/usr/bin/env bash
# E18 (issue #3): do captured versions survive offline in-place edits until a custodian holds them?
# A laptop (untrusted) captures five versions of one file while its custodian is away, with
# failed uploads, then reconnects. With annex.thin (ADR 0002) an in-place save destroys the
# laptop's only copy of the previous version: the negative test. Capturing a copy instead
# (`-c annex.thin=false`, ADR 0041) keeps every version until the custodian holds it; the old
# versions are then dropped and the current file is re-thinned, back to one copy. Also checks
# that atomic saves are safe even when thin, and that no git-annex command re-thins a copy.
# Asserts; exits non-zero on failure.
set -euo pipefail
export PATH=~/git-annex.linux:$PATH
L=${LAB:-$HOME/ga-lab18}; chmod -R u+w "$L" 2>/dev/null || true; rm -rf "$L"; mkdir -p "$L"
cd "$L"
SIZE_MB=${SIZE_MB:-4}; VERSIONS=5
fail() { echo "FAIL: $*"; exit 1; }
ok() { echo "ok: $*"; }
G() { git -c user.name=lab -c user.email=lab@pigeon "$@"; }
intact() {  # intact REPO KEY: REPO holds KEY's content and it hashes to KEY
  local loc; loc=$(git -C "$1" annex contentlocation "$2" 2>/dev/null) || return 1
  [ "$(sha256sum < "$1/$loc" | cut -c1-64)" = "${2##*--}" ]
}
kb() { du -sk "$1" | cut -f1; }

new_custodian() { chmod -R u+w "$L/C" 2>/dev/null || true; rm -rf "$L/C"; git init -q "$L/C"
  (cd "$L/C" && git annex init -q custodian && G commit -q --allow-empty -m genesis); }
# setup MODE SAVE: a laptop repository; MODE thin|copy decides how capture adds content
setup() {
  rm -rf "L-$1-$2"; git clone -q C "L-$1-$2"; cd "L-$1-$2"
  git annex init -q "laptop-$1-$2"; git annex untrust -q here; git annex numcopies 1 >/dev/null 2>&1
  git config annex.addunlocked true; git config annex.thin true; git annex wanted -q here "include=*"  # spec 03 §3.4
  mv ../C ../C.away   # the custodian is out of reach
}
capture() {  # capture MODE: what pigeon's capture step 6 runs
  if [ "$1" = copy ]; then G -c annex.thin=false annex add -q f; else G annex add -q f; fi
  G commit -qm "capture" f; git annex lookupkey f
}
save() {  # save HOW N: a new version, in place (same inode) or by atomic replace
  head -c $((SIZE_MB * 1024 * 1024)) /dev/urandom > "../v$2"
  if [ "$1" = inplace ]; then cat "../v$2" > f; else cp "../v$2" f.tmp; mv f.tmp f; fi
}

for MODE in thin copy; do
  for HOW in inplace atomic; do
    cd "$L"; new_custodian; setup "$MODE" "$HOW"; keys=()
    save atomic 1; keys+=("$(capture "$MODE")")
    for n in $(seq 2 "$VERSIONS"); do
      save "$HOW" "$n"; keys+=("$(capture "$MODE")")
      if git annex copy -q --all --to origin >/dev/null 2>&1; then fail "$MODE/$HOW: upload worked offline"; fi
    done
    [ "$MODE" = copy ] && [ "$(stat -c %h f)" != 1 ] && fail "copy/$HOW: capture made a hard link"
    git annex fsck -q f >/dev/null 2>&1 || true; git annex restage >/dev/null 2>&1 || true
    [ "$MODE" = copy ] && [ "$(stat -c %h f)" != 1 ] && fail "copy/$HOW: fsck or restage re-thinned the copy"
    pending=$(kb .)
    mv ../C.away ../C   # back online
    git annex copy -q --all --to origin >/dev/null 2>&1 || true
    held=0; for k in "${keys[@]}"; do intact ../C "$k" && held=$((held + 1)); done
    echo "$MODE/$HOW: the custodian holds $held of $VERSIONS versions (laptop used ${pending} KiB offline)"
    case "$MODE/$HOW" in
      thin/inplace) [ "$held" -lt "$VERSIONS" ] || fail "thin/inplace: expected lost versions (E8)";
                    ok "thin + in-place saves lose $((VERSIONS - held)) unprotected versions (the issue)";;
      *) [ "$held" = "$VERSIONS" ] || fail "$MODE/$HOW: $((VERSIONS - held)) versions lost";
         ok "$MODE/$HOW keeps every version until the custodian holds it";;
    esac
    if [ "$MODE" = copy ]; then
      # git-annex refuses `drop --auto --all`: old versions are dropped by key, verified
      for k in "${keys[@]:0:$((VERSIONS - 1))}"; do git annex drop -q --key "$k" >/dev/null 2>&1 \
        || fail "copy/$HOW: verified drop of a protected old version refused"; done
      if git annex drop --auto --all >/dev/null 2>&1; then fail "drop --auto --all is accepted now"; fi
      # re-thin the protected current version (pigeon uses the install sequence, ADR 0040)
      obj=$(git annex contentlocation "${keys[-1]}"); ln "$obj" .f.thin; chmod u+w .f.thin; mv -T .f.thin f
      G update-index -q --refresh || true; git annex restage >/dev/null 2>&1 || true
      [ -z "$(git status --porcelain)" ] || fail "copy/$HOW: re-thinned file shows as changed"
      [ "$(stat -c %h f)" = 2 ] || fail "copy/$HOW: re-thin did not link the object"
      after=$(kb .); echo "copy/$HOW: laptop ${pending} KiB offline, ${after} KiB once protected and re-thinned"
      [ "$after" -lt $((SIZE_MB * 1024 * 3 / 2 + 512)) ] || fail "copy/$HOW: more than about one copy left"
      ok "copy/$HOW: back to about one copy"
    fi
  done
done
if cp --reflink=always "$L/v1" "$L/v1.reflink" 2>/dev/null; then
  echo "reflink: supported here; a copy shares its blocks (measure with df)"
else
  echo "reflink: not supported on this file system ($(stat -f -c %T "$L")); to measure on APFS and btrfs"
fi
echo "E18 passed"
