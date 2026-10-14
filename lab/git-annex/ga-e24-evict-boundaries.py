#!/usr/bin/env python3
"""E24 (issue #10): what freeing a folder may lose or show, with real git and git-annex.

Laptop B (unlocked files, as pigeon's) holds folders F and G; A holds every key.
Asserted:
 1. `git sparse-checkout set` hides a clean file by unlinking it after a stat check
    (unpack-trees), so a write through a handle opened before the hide lands in an
    unlinked inode (annex.thin=false) and no file holds it: the save is lost.
    With annex.thin=true the file is the object's inode: the write changes the
    object, which git-annex then calls absent, so it stays inside .git/annex.
 2. A hide that first renames the file aside keeps that write: the aside holds it.
 3. `git annex drop --key` replaces every unmodified file associated with the key by
    a pointer (removeAnnex in Annex/Content.hs), in another kept folder too, and the
    pointer keeps the old mtime.
 4. The same happens to a staged copy moved into a folder being kept, before its
    skip-worktree bits are cleared, since the keys database still associates it.
Exits non-zero on failure.
"""
import os, shutil, subprocess, sys

L = os.environ.get('LAB') or os.path.expanduser('~/ga-lab24')
ENV = dict(os.environ, GIT_CONFIG_GLOBAL='/dev/null', GIT_CONFIG_NOSYSTEM='1',
           GIT_AUTHOR_NAME='lab', GIT_AUTHOR_EMAIL='lab@pigeon',
           GIT_COMMITTER_NAME='lab', GIT_COMMITTER_EMAIL='lab@pigeon')
ENV['PATH'] = os.path.expanduser('~/git-annex.linux') + os.pathsep + ENV['PATH']
FAILS = []


def run(*a, cwd=None, check=True):
    r = subprocess.run(a, cwd=cwd, env=ENV, capture_output=True, text=True, timeout=120)
    if check and r.returncode:
        sys.exit(f'command failed: {a}: {r.stderr.strip()}')
    return r


def ok(cond, msg):
    print(('ok: ' if cond else 'FAIL: ') + msg)
    if not cond:
        FAILS.append(msg)


def holds(root, needle):
    """Paths under root (.git included) whose bytes contain needle."""
    found = []
    for dp, _, fn in os.walk(root):
        for f in fn:
            p = os.path.join(dp, f)
            try:
                if not os.path.islink(p) and needle in open(p, 'rb').read():
                    found.append(os.path.relpath(p, root))
            except OSError:
                pass
    return found


def is_pointer(p):
    try:
        return open(p, 'rb').read(15) == b'/annex/objects/'
    except OSError:
        return False


def lab(name, thin):
    """A holds F/a.txt, G/c.txt (same content as F/a.txt) and F/b.txt; B clones and gets all."""
    root = os.path.join(L, name)
    if os.path.exists(root):
        subprocess.run(['chmod', '-R', 'u+w', root])
        shutil.rmtree(root)
    os.makedirs(root)
    a, b = os.path.join(root, 'A'), os.path.join(root, 'B')
    run('git', 'init', '-q', '-b', 'main', a)
    run('git', 'annex', 'init', '-q', 'A', cwd=a)
    run('git', 'config', 'annex.addunlocked', 'true', cwd=a)
    for d in ('F', 'G'):
        os.makedirs(os.path.join(a, d))
    open(os.path.join(a, 'F/a.txt'), 'w').write('shared-content\n')
    open(os.path.join(a, 'G/c.txt'), 'w').write('shared-content\n')
    open(os.path.join(a, 'F/b.txt'), 'w').write('other-content\n')
    run('git', 'annex', 'add', '-q', '.', cwd=a)
    run('git', 'commit', '-qm', 'init', cwd=a)
    run('git', 'clone', '-q', a, b)
    run('git', 'annex', 'init', '-q', 'B', cwd=b)
    run('git', 'config', 'annex.addunlocked', 'true', cwd=b)
    run('git', 'config', 'annex.thin', 'true' if thin else 'false', cwd=b)
    run('git', 'annex', 'get', '-q', '.', cwd=b)
    key = run('git', 'annex', 'lookupkey', 'F/b.txt', cwd=b).stdout.strip()
    shared = run('git', 'annex', 'lookupkey', 'F/a.txt', cwd=b).stdout.strip()
    return b, key, shared


HIDE_F = ('git', 'sparse-checkout', 'set', '--no-cone', '/*', '!/F/')

# 1. A write through a handle opened before the hide.
for thin in (False, True):
    b, key, _ = lab(f'hide-thin-{thin}', thin)
    h = open(os.path.join(b, 'F/b.txt'), 'r+b')          # an editor holds the file open
    run(*HIDE_F, cwd=b)
    ok(not os.path.exists(os.path.join(b, 'F/b.txt')),
       f'thin={thin}: git hid the clean F/b.txt although a writer held it open')
    h.seek(0); h.write(b'SAVE-THROUGH-HANDLE\n'); h.flush(); os.fsync(h.fileno()); h.close()
    where = holds(b, b'SAVE-THROUGH-HANDLE')
    if not thin:
        ok(where == [], f'thin=False: the save through the handle is in no file ({where})')
    else:
        ok(len(where) == 1 and where[0].startswith('.git/annex/objects/'),
           f'thin=True: the save went into the annex object ({where})')
        run('git', 'annex', 'drop', '-q', '--key', key, cwd=b)
        again = holds(b, b'SAVE-THROUGH-HANDLE')
        ok(again == where, 'thin=True: the drop skips the changed object (inAnnex\' in'
           ' Annex/Content/Presence.hs calls it absent): the save stays only inside'
           ' .git/annex/objects, at no path the person sees')

# 2. The same write, when the hide first renames the file aside.
b, key, _ = lab('aside', False)
h = open(os.path.join(b, 'F/b.txt'), 'r+b')
aside = os.path.join(b, '.git/pigeon/aside')
os.makedirs(aside)
os.rename(os.path.join(b, 'F/b.txt'), os.path.join(aside, 'b.txt'))
run(*HIDE_F, cwd=b)
h.seek(0); h.write(b'SAVE-THROUGH-HANDLE\n'); h.flush(); os.fsync(h.fileno()); h.close()
where = holds(b, b'SAVE-THROUGH-HANDLE')
ok(where == ['.git/pigeon/aside/b.txt'], f'renamed aside first, the save is kept in the aside ({where})')
skip = run('git', 'ls-files', '-t', 'F/', cwd=b).stdout.split()
ok(skip.count('S') == 2, 'the hide still set both skip-worktree bits of F, with nothing to unlink')

# 3. Dropping a key a kept path of another folder uses.
b, _, shared = lab('drop-kept', False)
c = os.path.join(b, 'G/c.txt')
mtime = os.stat(c).st_mtime_ns
run(*HIDE_F, cwd=b)
run('git', 'annex', 'drop', '-q', '--key', shared, cwd=b)
ok(is_pointer(c), 'drop --key of F/a.txt\'s key turned the kept G/c.txt into a pointer file')
ok(os.stat(c).st_mtime_ns == mtime, 'the pointer keeps the old mtime')
st = run('git', 'status', '--porcelain', cwd=b).stdout
print(f'measured: git status after the drop: {st.strip() or "(clean)"}')

# 4. Dropping a key while a staged copy is moved into a folder being kept.
b, _, shared = lab('drop-staged', False)
obj = run('git', 'annex', 'contentlocation', shared, cwd=b).stdout.strip()
run(*HIDE_F, cwd=b)                                        # F freed ...
stage = os.path.join(b, '.git/pigeon/stage/F')
os.makedirs(stage)                                         # ... then kept: stage a complete copy
shutil.copyfile(os.path.join(b, obj), os.path.join(stage, 'a.txt'))
shutil.copyfile(os.path.join(b, 'G/c.txt'), os.path.join(stage, 'b.txt'))
os.rmdir(os.path.join(b, 'F')) if os.path.isdir(os.path.join(b, 'F')) else None
os.rename(stage, os.path.join(b, 'F'))                     # moved in, skip-worktree still set
run('git', 'annex', 'drop', '-q', '--key', shared, cwd=b)  # another drop of that key
ok(is_pointer(os.path.join(b, 'F/a.txt')),
   'a drop of the same key turned the staged, moved-in F/a.txt into a pointer')

print(f'E24: {len(FAILS)} failure(s)')
sys.exit(1 if FAILS else 0)
