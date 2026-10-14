#!/usr/bin/env bash
# E15b (issue #2): can freeing and keeping a folder avoid ever showing a pointer file?
# An observer scans the folder continuously while each sequence runs. The order of
# ADR 0031 (drop, then hide; show, then smudge) is measured; the reordered sequences
# (hide, then drop by key; stage complete files, then show without a checkout) must
# show no pointer, also when killed at random instants and then rerun.
# Asserts; exits non-zero on failure. Needs python3.
set -euo pipefail
export PATH=~/git-annex.linux:$PATH
# git isolated from the person's configuration (E20), with a lab identity for git-annex's commits
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 GIT_AUTHOR_NAME=lab GIT_AUTHOR_EMAIL=lab@pigeon \
  GIT_COMMITTER_NAME=lab GIT_COMMITTER_EMAIL=lab@pigeon
L=${LAB:-$HOME/ga-lab15b}; chmod -R u+w "$L" 2>/dev/null || true; rm -rf "$L"; mkdir -p "$L"
cd "$L"
N=${N:-100}; TRIALS=${TRIALS:-10}
fail() { echo "FAIL: $*"; exit 1; }
ok() { echo "ok: $*"; }
G() { git -c user.name=lab -c user.email=lab@pigeon "$@"; }
# setsid(1) without util-linux (macOS): the job leads its own group, so kill -- -PID ends it all
SETSID=(python3 -c 'import os, sys; os.setsid(); os.execvp(sys.argv[1], sys.argv[1:])')

cat > observe.py <<'PY'
import os, sys, time
root, stop, log = sys.argv[1], sys.argv[2], sys.argv[3]
seen = 0
with open(log, 'w') as out:
    while not os.path.exists(stop):
        for dp, dn, fn in os.walk(root):
            for f in fn:
                p = os.path.join(dp, f)
                try:
                    with open(p, 'rb') as h:
                        if h.read(15) == b'/annex/objects/':
                            out.write(p + '\n'); seen += 1
                except OSError:
                    pass
    out.write(f'# done {seen}\n')
PY
watch_start() { rm -f "$L/stop.flag"; python3 "$L/observe.py" "$1" "$L/stop.flag" "$2" & OBS=$!; sleep 0.2; }
watch_stop() { touch "$L/stop.flag"; wait "$OBS"; grep -vc '^#' "$1" || true; }

# A holds everything (it stands for the backup machines); B is a laptop.
git init -q -b main A; cd A; git annex init -q A; git config annex.addunlocked true
mkdir Photos Docs
for i in $(seq 1 "$N"); do echo "photo-$i" > "Photos/p$i.jpg"; done
echo doc-1 > Docs/b.txt
git annex add -q .; G commit -qm init; cd ..
git clone -q A B; cd B; git annex init -q B; git config annex.addunlocked true
git annex get -q .
# git-annex's batch modes (one process for all keys; --batch-keys, Command/Get.hs, Command/Drop.hs)
keys() { git ls-tree -r HEAD Photos/ --name-only | git annex lookupkey --batch; }
keys > ../keys.txt
[ "$(wc -l < ../keys.txt | tr -d ' ')" = "$N" ] || fail "expected $N keys"
ok "B holds $N photos"

# 1. ADR 0031's free order: drop while checked out, then hide.
watch_start Photos ../obs1.log
git annex drop -q Photos
git sparse-checkout set --no-cone '/*' '!/Photos/'
n=$(watch_stop ../obs1.log)
echo "measured: drop-then-hide showed $n pointer observations"
[ "$n" -gt 0 ] || fail "expected the old order to expose pointers"
ok "the order of ADR 0031 exposes pointer files (issue #2 confirmed)"

# 2. ADR 0031's keep order: get, show, then smudge.
git annex get -q --batch-keys < ../keys.txt >/dev/null
watch_start . ../obs2.log
git sparse-checkout add /Photos/
git annex smudge --update >/dev/null 2>&1
n=$(watch_stop ../obs2.log)
echo "measured: show-then-smudge showed $n pointer observations"
git annex restage >/dev/null 2>&1
[ -z "$(git status --porcelain)" ] || fail "status not clean after old keep"

# The reordered sequences. free: hide first (files go from complete to absent), then
# drop by key. keep: get by key, stage complete copies outside the tree, move the
# folder in with one rename, then clear skip-worktree without any checkout.
free_new() {   # a kill -9 inside `git sparse-checkout set` leaves its lock; pigeond removes it at start
  rm -f .git/index.lock .git/info/sparse-checkout.lock
  git sparse-checkout set --no-cone '/*' '!/Photos/'
  git annex drop -q --batch-keys < ../keys.txt >/dev/null 2>&1 || true
}
keep_new() {
  rm -f .git/index.lock
  git annex get -q --batch-keys < ../keys.txt >/dev/null
  if [ ! -d Photos ]; then
    rm -rf .git/pigeon/stage; mkdir -p .git/pigeon/stage/Photos
    git ls-tree -r HEAD Photos/ --name-only > ../paths.txt
    git annex lookupkey --batch < ../paths.txt | git annex contentlocation --batch > ../locations.txt
    paste ../paths.txt ../locations.txt | while IFS=$'\t' read -r p o; do
      cp "$o" ".git/pigeon/stage/$p"; chmod 0644 ".git/pigeon/stage/$p"   # objects are read-only
    done
    python3 -c 'import os; os.rename(".git/pigeon/stage/Photos", "Photos")'
  fi
  printf '/*\n' > .git/info/sparse-checkout
  git ls-files -t Photos/ | sed -n 's/^S //p' | git update-index -q --no-skip-worktree --stdin
  git update-index -q --refresh >/dev/null || true
  git annex restage >/dev/null 2>&1
}
complete_or_absent() {   # every photo is complete or absent; nothing else
  python3 - "$N" <<'PY'
import os, sys
n = int(sys.argv[1]); bad = []
for i in range(1, n + 1):
    p = f'Photos/p{i}.jpg'
    if os.path.exists(p):
        c = open(p, 'rb').read()
        if c != f'photo-{i}\n'.encode(): bad.append((p, c[:40]))
sys.exit(f'bad files: {bad[:3]}' if bad else 0)
PY
}

# 3. New free order, observed.
watch_start Photos ../obs3.log
free_new
n=$(watch_stop ../obs3.log)
[ "$n" = 0 ] || fail "hide-then-drop showed $n pointers"
[ ! -e Photos ] || fail "Photos still on disk"
git annex whereis --key="$(head -1 ../keys.txt)" | grep -q ' -- A ' || fail "A lost a photo"
[ -z "$(git status --porcelain)" ] || fail "status not clean after free"
ok "hide-then-drop: no pointer observed, Photos absent, status clean"

# 4. New keep order, observed.
watch_start . ../obs4.log
keep_new
n=$(watch_stop ../obs4.log)
[ "$n" = 0 ] || fail "stage-then-show showed $n pointers"
complete_or_absent || fail "keep left wrong content"
[ "$(ls Photos | wc -l | tr -d ' ')" = "$N" ] || fail "not all photos are back"
[ -z "$(git status --porcelain)" ] || fail "status not clean after keep: $(git status --porcelain | head -3)"
ok "stage-then-show: no pointer observed, all $N photos complete, status clean"

# 5. Crashes: kill -9 each sequence (its whole process group) at a random instant,
# check, rerun, check.
export -f free_new keep_new; export N
for t in $(seq 1 "$TRIALS"); do
  d=$(python3 -c 'import random; print(round(random.uniform(0, 1.5), 2))')
  watch_start . ../obs5.log
  "${SETSID[@]}" bash -c free_new & P=$!; sleep "$d"; kill -9 -- -"$P" 2>/dev/null || true; wait "$P" 2>/dev/null || true
  complete_or_absent || fail "trial $t: free killed at ${d}s left a bad file"
  bash -ec free_new > ../rerun.log 2>&1 || fail "trial $t: rerun of free_new failed: $(tail -3 ../rerun.log)"
  [ ! -e Photos ] || fail "trial $t: rerun of free left Photos"
  "${SETSID[@]}" bash -c keep_new & P=$!; sleep "$d"; kill -9 -- -"$P" 2>/dev/null || true; wait "$P" 2>/dev/null || true
  complete_or_absent || fail "trial $t: keep killed at ${d}s left a bad file"
  bash -ec keep_new > ../rerun.log 2>&1 || fail "trial $t: rerun of keep_new failed: $(tail -3 ../rerun.log)"
  n=$(watch_stop ../obs5.log)
  [ "$n" = 0 ] || fail "trial $t: $n pointers observed"
  complete_or_absent || fail "trial $t: bad content after rerun"
  [ -z "$(git status --porcelain)" ] || fail "trial $t: status not clean: $(git status --porcelain | head -3)"
done
ok "$TRIALS kill -9 trials: never a pointer, every file complete or absent, rerun recovers"

# 6. A file saved in the folder just before freeing is not removed by the hide.
echo "edited" > Photos/p1.jpg
git sparse-checkout set --no-cone '/*' '!/Photos/' 2>../dirty.err || true
if [ -e Photos/p1.jpg ]; then
  [ "$(cat Photos/p1.jpg)" = edited ] || fail "the edit was changed"
  ok "git sparse-checkout keeps a modified file in place ($(head -c 80 ../dirty.err | tr '\n' ' '))"
else
  fail "git sparse-checkout removed a modified file"
fi
echo "E15b passed"
