#!/usr/bin/env python3
"""E22 (issues #12, #13): two repositories on one disk, and capacity limits, against git-annex.

A laptop L (numcopies = mincopies = 2, untrusted) and bare custodians. Asserts:
2. git-annex counts two UUIDs on one disk, even one hard-linked inode, as two
   copies: the verified drop succeeds; only a `clone --shared` is untrusted.
   pigeon counts them twice too (ADR 0046) and sets annex.hardlink=false;
3. annex.diskreserve refuses a get and an incoming copy, over a local path and
   over git-annex-shell p2pstdio, and nothing is counted;
4. maxsize with sizebalanced leaves keys with one custody copy, which the
   verified drop refuses; maxsize does not stop an explicit copy;
5. a full disk (a tmpfs with sudo on Linux, a disk image on macOS): ENOSPC fails a
   get and a fetch, claims nothing, git fsck passes, and once space is freed git
   annex fsck passes and the retries succeed; 6. a custodian killed or deleted
   while receiving never counts the key in flight; the verified drop refuses.
Checks 1 and 7, on failure domains, were dropped with them (ADR 0046). Prints PASS, FAIL or SKIP per check; exits non-zero on failure. Environment: LAB.
Sources cited: ~/src/git-annex-10.20261006.
"""
import errno, glob, hashlib, json, os, shutil, signal, subprocess, sys, time

os.environ['PATH'] = os.path.expanduser('~/git-annex.linux') + ':' + os.environ['PATH']
os.environ.update(GIT_CONFIG_GLOBAL='/dev/null', GIT_CONFIG_NOSYSTEM='1', GIT_AUTHOR_NAME='lab',
                  GIT_AUTHOR_EMAIL='lab@pigeon', GIT_COMMITTER_NAME='lab', GIT_COMMITTER_EMAIL='lab@pigeon')
for v in ('GIT_DIR', 'GIT_WORK_TREE', 'GIT_INDEX_FILE'):
    os.environ.pop(v, None)
K = 2
KB, MB = 1000, 1 << 20
LINUX, MAC = sys.platform.startswith('linux'), sys.platform == 'darwin'
FAILS, RESULTS, MOUNTS = [], [], []
# A fake ssh that runs the remote command here: git-annex then talks to `git-annex-shell p2pstdio`,
# the P2P protocol of an ssh remote (Remote/Git.hs copyToRemote', Git.repoIsSsh -> P2PHelper.store).
FAKE_SSH = '''#!/bin/sh
host=
while [ $# -gt 0 ]; do case "$1" in -[oplSFiOJ]) shift 2;; -*) shift;; *) [ -n "$host" ] && break; host=$1; shift;; esac; done
exec sh -c "$*"
'''


def fail(msg):
    FAILS.append(msg)
    print('  fail:', msg, flush=True)


def check(cond, msg):
    if not cond:
        fail(msg)
    return cond


def note(msg):
    print('  info:', msg, flush=True)


def result(n, title, before):
    ok = len(FAILS) == before
    RESULTS.append((n, 'PASS' if ok else 'FAIL'))
    print(f'{"PASS" if ok else "FAIL"}: E22.{n} {title}', flush=True)


def skip(n, why):
    RESULTS.append((n, 'SKIP'))
    print(f'SKIP: E22.{n} {why}', flush=True)


def sh(*args, cwd=None, check=True, env=None, timeout=120):
    r = subprocess.run(list(args), cwd=cwd, capture_output=True, text=True, timeout=timeout,
                       env=dict(os.environ, **(env or {})))
    if check and r.returncode:
        raise RuntimeError(f'{" ".join(args)} in {cwd}: {r.stderr.strip()[-500:]}')
    return r


def said(r, *words):
    """The first line of r's output holding one of words, else its last line."""
    lines = [l.strip() for l in (r.stdout + '\n' + r.stderr).splitlines() if l.strip()]
    return next((l for l in lines if any(w in l for w in words)), lines[-1] if lines else '')


def wait_for(cond, timeout=30):
    end = time.time() + timeout
    while time.time() < end:
        if cond():
            return True
        time.sleep(0.02)
    return False


class Repo:
    def __init__(self, root, name, bare=False):
        self.root, self.name, self.bare = root, name, bare

    def run(self, *args, check=True, env=None):
        return sh('git', *args, cwd=self.root, check=check, env=env).stdout

    def r(self, *args, env=None):
        return sh('git', *args, cwd=self.root, check=False, env=env)

    def ok(self, *args):
        return self.r(*args).returncode == 0

    def uuid(self):                                 # kept, for a repository that is later deleted
        if not getattr(self, 'u', None):
            self.u = self.run('config', 'annex.uuid').strip()
        return self.u

    def rev(self, ref):
        return self.run('rev-parse', '-q', '--verify', ref, check=False).strip() or None

    def present(self, key):
        return self.ok('annex', 'contentlocation', key)

    def whereis(self, key):
        """UUIDs the location log of this repository gives for key (untrusted ones included)."""
        r = self.r('annex', 'whereis', '--json', '--key', key)
        j = json.loads(r.stdout) if r.stdout.strip() else {}
        return {w['uuid'] for w in j.get('whereis', []) + j.get('untrusted', [])}

    def obj(self, key):
        """Where a bare repository keeps key's content (Annex/DirHashes.hs hashDirLower)."""
        h = hashlib.md5(key.encode()).hexdigest()
        return os.path.join(self.root, 'annex', 'objects', h[:3], h[3:6], key, key)

    def tmp(self, key):
        """Size of key's partial download in annex/tmp (Annex/Content.hs getViaTmpFromDisk keeps it)."""
        p = os.path.join(self.root, 'annex' if self.bare else '.git/annex', 'tmp', key)
        return os.path.getsize(p) if os.path.exists(p) else None

    def add(self, files, msg='add'):
        """name -> data, as annexed files of one commit; returns name -> key."""
        for rel, data in files.items():
            with open(os.path.join(self.root, rel), 'wb') as h:
                h.write(data)
        self.run('annex', 'add', '-q', '--', *files)
        self.run('commit', '-q', '-m', msg)
        found = self.run('annex', 'find', '--format=${file}\t${key}\n', '--', *files)
        return dict(line.split('\t') for line in found.splitlines())

    def link(self, other, fetch=True):
        """A remote; git-annex merges a fetched git-annex branch itself (Annex/Branch.hs update)."""
        self.run('remote', 'add', other.name, other.root)
        if fetch:
            self.run('fetch', '-q', other.name)

    def learn(self, *others):
        for o in others:
            self.run('fetch', '-q', o.name)


def make_custodian(root, name):
    r = Repo(root, name, bare=True)
    os.makedirs(r.root)
    r.run('init', '-q', '--bare', '-b', 'main')
    r.run('annex', 'init', '-q', name)
    r.run('annex', 'group', '.', 'custodian')
    r.uuid()
    return r


def make_laptop(root, name, untrusted=True, copies=K):
    r = Repo(root, name)
    os.makedirs(r.root)
    r.run('init', '-q', '-b', 'main')
    r.run('annex', 'init', '-q', name)
    if untrusted:
        r.run('annex', 'untrust', 'here')
    r.run('annex', 'numcopies', str(copies))
    r.run('annex', 'mincopies', str(copies))
    return r


def wipe(path):
    if os.path.lexists(path):                       # rm -rf: E22.6 deletes a repository being written
        subprocess.run(['chmod', '-R', 'u+w', path], stderr=subprocess.DEVNULL)
        subprocess.run(['rm', '-rf', path], stderr=subprocess.DEVNULL)


def free(path):
    s = os.statvfs(path)
    return s.f_bavail * s.f_frsize


def sudo_ok():
    return LINUX and shutil.which('sudo') and sh('sudo', '-n', 'true', check=False).returncode == 0


def mount_tmpfs(path, size):
    os.makedirs(path)
    sh('sudo', 'mount', '-t', 'tmpfs', '-o', f'size={size},uid={os.getuid()},gid={os.getgid()},mode=0755',
       'tmpfs', path)
    MOUNTS.append(path)
    return path


def mount_dmg(path, size='16m'):
    img = path + '.dmg'
    os.makedirs(os.path.dirname(img), exist_ok=True)
    sh('hdiutil', 'create', '-quiet', '-size', size, '-fs', 'HFS+', '-volname', 'e22', img)
    os.makedirs(path)
    sh('hdiutil', 'attach', '-quiet', '-nobrowse', '-mountpoint', path, img)
    MOUNTS.append(path)
    return path


def unmount_all(lab):
    if LINUX and sudo_ok():
        with open('/proc/self/mountinfo') as h:
            stale = [l.split()[4] for l in h if l.split()[4].startswith(lab.rstrip('/') + '/')]
        MOUNTS.extend(m for m in stale if m not in MOUNTS)
    for m in reversed(MOUNTS):
        if MAC:
            subprocess.run(['hdiutil', 'detach', '-quiet', '-force', m])
        else:
            subprocess.run(['sudo', 'umount', '-l', m])
    MOUNTS.clear()


def fill(path):
    """Write zeros to path until its file system is full (ENOSPC); returns the bytes written."""
    fd, n = os.open(path, os.O_WRONLY | os.O_CREAT, 0o644), 0
    try:
        for chunk in (1 << 16, 1 << 12, 1 << 9):
            while True:
                try:
                    n += os.write(fd, b'\0' * chunk)
                except OSError as e:
                    if e.errno != errno.ENOSPC:
                        raise
                    break
    finally:
        os.close(fd)
    return n


def popen(repo, *args, config=()):
    cfg = [a for c in config for a in ('-c', c)]
    return subprocess.Popen(['git', *cfg, 'annex', *args], cwd=repo.root, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, text=True, start_new_session=True)


def finished(p, timeout=90):
    out, err = p.communicate(timeout=timeout)
    return subprocess.CompletedProcess(p.args, p.returncode, out, err)


# ---- E22.2: two repositories on one disk
def e2(lab):
    C1, C2 = (make_custodian(os.path.join(lab, n), n) for n in ('C1', 'C2'))
    before = len(FAILS)
    L = make_laptop(os.path.join(lab, 'L'), 'L')
    for C in (C1, C2):
        L.link(C)
    k = L.add({'a.bin': os.urandom(200 * KB)})['a.bin']
    for C in (C1, C2):
        L.run('annex', 'copy', '-q', '--to', C.name, '--key', k)
    d = L.r('annex', 'drop', '--key', k)
    check(d.returncode == 0 and not L.present(k), f'the verified drop refuses: {said(d, "lock")}')
    L.run('config', 'annex.hardlink', 'true')       # Remote/Git.hs wantHardLink, mkFileCopier
    k2 = L.add({'b.bin': os.urandom(200 * KB)})['b.bin']
    for C in (C1, C2):
        L.run('annex', 'copy', '-q', '--to', C.name, '--key', k2)
    paths = [os.path.join(L.root, L.run('annex', 'contentlocation', k2).strip()), C1.obj(k2), C2.obj(k2)]
    inodes = {os.stat(p).st_ino for p in paths}
    check(len(inodes) == 1, f'annex.hardlink: {len(inodes)} inodes for L, C1 and C2')
    d2 = L.r('annex', 'drop', '--key', k2)
    check(d2.returncode == 0, f'the verified drop refuses one inode in two custodians: {said(d2, "lock")}')
    os.chmod(C1.obj(k2), 0o644)
    with open(C1.obj(k2), 'r+b') as h:
        h.write(b'X' * 16)
    f2 = C2.r('annex', 'fsck', '--key', k2)
    check(f2.returncode != 0, 'C2 does not see the corruption written through C1')
    S = Repo(os.path.join(lab, 'S'), 'S', bare=True)
    sh('git', 'clone', '-q', '--bare', '--shared', C1.root, S.root)
    si = said(S.r('annex', 'init', 'S'), '--shared')
    check('untrusted' in si, f'git annex init in a clone --shared: {si!r}')
    note(f'clone --shared: "{si}" (Annex/Init.hs initSharedClone)')
    result(2, 'numcopies = mincopies = 2: L\'s verified drop succeeds with C1 and C2 on one disk, and with '
              'C1 and C2 holding one hard-linked inode (annex.hardlink), which one write corrupts in both; '
              'only `git clone --shared` is made untrusted', before)


# ---- E22.3: annex.diskreserve
def e3(lab):
    before = len(FAILS)
    L, C = make_laptop(os.path.join(lab, 'L3'), 'L3'), make_custodian(os.path.join(lab, 'C3'), 'C3')
    keys = L.add({'big.bin': os.urandom(128 * MB), 'small.bin': os.urandom(MB)})
    big, small = keys['big.bin'], keys['small.bin']
    L.link(C)
    C.link(L)
    bindir = os.path.join(lab, 'bin')
    os.makedirs(bindir)
    with open(os.path.join(bindir, 'ssh'), 'w') as h:
        h.write(FAKE_SSH)
    os.chmod(os.path.join(bindir, 'ssh'), 0o755)
    os.environ['PATH'] = bindir + ':' + os.environ['PATH']
    L.run('config', 'annex.sshcaching', 'false')
    L.run('remote', 'add', 'P3', f'ssh://lab{C.root}')
    L.run('fetch', '-q', 'P3')

    def reserve(margin=16 * MB):                    # wide margins: a shared disk's free space moves
        C.run('config', 'annex.diskreserve', str(free(C.root) - margin))

    def nothing(what):
        check(not C.present(big) and C.uuid() not in C.whereis(big) | L.whereis(big),
              f'{what}: big.bin is counted at C3')
    reserve()
    g = C.r('annex', 'get', '--key', big)
    mg = said(g, 'not enough free space')
    check(g.returncode != 0 and 'not enough free space' in mg, f'get: {mg}')
    nothing('get')
    check(C.tmp(big) is None, 'the refused get leaves a temporary file')
    note(f'get --key: "{mg}", then "{said(g, "Unable to access")}"')
    reserve(64 * MB)
    check(C.ok('annex', 'get', '-q', '--key', small), 'the reserve refuses a key that fits')
    msgs = {}
    for remote in ('C3', 'P3'):
        reserve()
        c = L.r('annex', 'copy', '--to', remote, '--key', big)
        msgs[remote] = said(c, 'not enough free space')
        check(c.returncode != 0 and 'not enough free space' in msgs[remote], f'copy --to {remote}: {msgs[remote]}')
        nothing(f'copy --to {remote}')
        if remote == 'P3':
            note(f'copy --to P3 (p2pstdio): "{msgs[remote]}", then "{said(c, "transfer already")}" and '
                 f'"{said(c, "Lost connection")}" (P2P/Annex.hs runLocal StoreContent, checktransfer)')
        else:
            note(f'copy --to C3 (local path): "{msgs[remote]}", then "{said(c, "failed to send")}"')
    reserve()
    fo = C.r('annex', 'get', '--force', '--key', big)
    check(fo.returncode == 0 and C.present(big), 'get --force does not override the reserve')
    result(3, 'annex.diskreserve on C3 refuses `get --key` and an incoming `copy --to` over a local path and '
              'over git-annex-shell p2pstdio, before any byte is kept; nothing is counted; --force overrides',
           before)


# ---- E22.4: maxsize with sizebalanced
WANTED = 'sizebalanced=custodian:2'


def e4(lab):
    before = len(FAILS)
    L = make_laptop(os.path.join(lab, 'L4'), 'L4')
    C1, C2 = (make_custodian(os.path.join(lab, n), n) for n in ('C1', 'C2'))
    maxsize = {'C1': 10 ** 9, 'C2': 400 * KB}
    for C in (C1, C2):
        C.run('annex', 'wanted', '.', WANTED)
        C.run('annex', 'maxsize', 'here', str(maxsize[C.name]))
    keys = L.add({f'f{i}.bin': os.urandom(300 * KB) for i in range(5)})
    names = sorted(keys)
    for C in (C1, C2):
        L.link(C)
        C.link(L)
    C1.link(C2)
    C2.link(C1)
    wanted = {}
    for C in (C1, C2):                              # spec 08 §8.3: per-key downloads of wanted keys
        C.learn(L, C2 if C is C1 else C1)
        wanted[C.name] = []
        for n in names:
            if C.ok('annex', 'matchexpression', f'--key={keys[n]}', WANTED):
                wanted[C.name].append(n)
                check(C.ok('annex', 'get', '-q', '--key', keys[n]), f'{C.name} cannot get {n}')
    L.learn(C1, C2)
    custody = {n: sorted(C.name for C in (C1, C2) if C.uuid() in L.whereis(keys[n])) for n in names}
    one = [n for n in names if len(custody[n]) == 1]
    two = [n for n in names if len(custody[n]) == 2]
    note(f'matchexpression {WANTED!r}: C1 wants {wanted["C1"]}, C2 (maxsize 400 kB) wants {wanted["C2"]}')
    note(f'one custody copy: {one}; two: {two}')
    check(wanted['C1'] == names and one and set(one) == set(names) - set(wanted['C2']), f'custody {custody}')
    check(sum(os.path.getsize(C2.obj(keys[n])) for n in two) <= maxsize['C2'], 'C2 is over its maxsize')
    for n in one:
        d = L.r('annex', 'drop', '--key', keys[n])
        check(d.returncode != 0 and L.present(keys[n]), f'the verified drop of {n} succeeds with one custodian')
    note(f'drop: "{said(d, "Could only verify", "Unable to lock down")}"')
    f = L.r('annex', 'fsck', '--fast', '--key', keys[one[0]])
    note(f'fsck --fast: "{said(f, "trustworthy")}"')
    check(f.returncode != 0, 'fsck does not report the missing custody copy')
    if two:
        d = L.r('annex', 'drop', '--key', keys[two[0]])
        check(d.returncode == 0, f'the verified drop of {two[0]} refuses with two custodians: {said(d, "lock")}')
    c = L.r('annex', 'copy', '--to', 'C2', '--key', keys[one[0]])
    check(c.returncode == 0 and C2.present(keys[one[0]]), 'maxsize stops an explicit copy')
    result(4, f'K = 2, C2\'s maxsize 400 kB for 1.5 MB: sizebalanced assigns C2 only {", ".join(wanted["C2"])}, leaves '
              f'{len(one)} with one custody copy; the verified drop refuses them, fsck reports them; an '
              'explicit copy goes over maxsize', before)


# ---- E22.5: a real full disk
def e5(lab):
    if not SUDO and not MAC:
        return skip(5, 'no passwordless sudo, so no small tmpfs to fill (CI Linux does it, macOS a disk image)')
    before = len(FAILS)
    kind = 'tmpfs' if SUDO else 'HFS+ disk image'
    mnt = (mount_tmpfs if SUDO else mount_dmg)(os.path.join(lab, 'full'), '30m')
    L = make_laptop(os.path.join(lab, 'L5'), 'L5', untrusted=False, copies=1)
    keys = list(L.add({f'k{i}.bin': os.urandom(4 * MB) for i in range(8)}).values())
    C = make_custodian(os.path.join(mnt, 'C5'), 'C5')
    C.run('config', 'annex.diskreserve', '0')
    C.link(L)
    C.run('fetch', '-q', 'L5', '+refs/heads/main:refs/heads/main')
    start = free(mnt)
    for k in keys[:3]:
        check(C.ok('annex', 'get', '-q', '--key', k), 'a get that fits fails')
    k4 = keys[3]                                    # a slow get, while another writer fills the disk
    p = popen(C, 'get', '--key', k4, config=['remote.L5.annex-bwlimit=2MiB'])
    check(wait_for(lambda: (C.tmp(k4) or 0) > MB), 'the slow get does not start')
    filler = os.path.join(mnt, 'filler')
    filled = fill(filler)
    g = finished(p)
    enospc = said(g, 'No space left')
    check(g.returncode != 0 and 'No space left' in enospc, f'the get on a full disk: {g.returncode} {enospc}')
    check(not C.present(k4) and C.uuid() not in C.whereis(k4), 'the location log claims the failed key')
    partial = C.tmp(k4)
    note(f'{kind} of 30 MB, {start // MB} MB free after init; three 4 MB keys, then a filler of '
         f'{filled // KB} kB during the fourth get: "{enospc}"; {partial} bytes stay in annex/tmp')
    af, gf = C.r('annex', 'fsck', '-q', '--all'), C.r('fsck', '--no-progress')
    check(gf.returncode == 0, f'git fsck on the full disk: {said(gf, "error")}')
    note(f'git annex fsck on the full disk exits {af.returncode}: "{said(af, "SQLite", "No space", "git-annex:")}"')
    with open(os.path.join(L.root, 'notes.bin'), 'wb') as h:
        h.write(os.urandom(2 * MB))
    L.run('-c', 'annex.largefiles=nothing', 'add', 'notes.bin')
    L.run('commit', '-q', '-m', 'notes in git')
    old = C.rev('refs/heads/main')
    fe = C.r('fetch', 'L5', '+refs/heads/main:refs/heads/main')
    gf = C.r('fsck', '--no-progress')
    tmp_packs = glob.glob(os.path.join(C.root, 'objects', 'pack', 'tmp_*'))
    check(fe.returncode != 0 and C.rev('refs/heads/main') == old, f'the fetch into a full disk: {said(fe, "fatal")}')
    check(gf.returncode == 0, f'git fsck after the failed fetch: {said(gf, "error")}')
    note(f'git fetch: "{said(fe, "fatal", "error")}"; main unchanged, {len(tmp_packs)} tmp_pack left')
    os.remove(filler)
    af = C.r('annex', 'fsck', '-q', '--all')
    check(af.returncode == 0, f'git annex fsck once space is freed: {said(af, "SQLite", "rror", "fail")}')
    g = C.r('annex', 'get', '--key', k4)
    check(g.returncode == 0 and C.ok('annex', 'fsck', '-q', '--key', k4), f'the retry fails: {said(g, "fail")}')
    fe = C.r('fetch', '-q', 'L5', '+refs/heads/main:refs/heads/main')
    check(fe.returncode == 0 and C.rev('refs/heads/main') == L.rev('refs/heads/main'), 'the fetch retry fails')
    refused = next((r for r in (C.r('annex', 'get', '--key', k) for k in keys[4:]) if r.returncode), None)
    check(refused is not None and 'not enough free space' in said(refused, 'not enough'),
          'with diskreserve 0 the disk fills to ENOSPC')
    if refused is not None:
        note(f'diskreserve 0, then the gets go on until "{said(refused, "not enough")}" '
             f'({free(mnt) // KB} kB free): checkDiskSpace refuses a key larger than the free space')
    af, gf = C.r('annex', 'fsck', '-q', '--all'), C.r('fsck', '--no-progress')
    check(af.returncode == 0 and gf.returncode == 0, 'fsck fails after the retries')
    result(5, f'a full {kind}: the get fails with ENOSPC (exit non-zero), the location log does not claim the '
              'key, git fsck passes, a git fetch fails and leaves main and the repository whole; once space is '
              'freed git annex fsck passes and both retries succeed', before)


# ---- E22.6: a custodian lost during ingestion
def e6(lab):
    before = len(FAILS)
    L = make_laptop(os.path.join(lab, 'L6'), 'L6')
    C1, C2, C3 = (make_custodian(os.path.join(lab, n), n) for n in ('C1', 'C2', 'C3'))
    keys = L.add({f'b{i}.bin': os.urandom(4 * MB) for i in range(1, 7)})
    names = sorted(keys)
    for C in (C1, C2, C3):
        L.link(C, fetch=False)
    L.run('annex', 'copy', '-q', '--to', 'C1', *names)
    first, second = names[:3], names[3:]

    def ingest(C, batch):                           # 4 MB keys at 4 MiB/s: about a second each
        p = popen(L, 'copy', '--to', C.name, *batch, config=[f'remote.{C.name}.annex-bwlimit=4MiB'])
        k1, k2 = keys[batch[0]], keys[batch[1]]
        check(wait_for(lambda: os.path.exists(C.obj(k1)) and (C.tmp(k2) or 0) > MB),
              f'{C.name} does not start receiving {batch[1]}')
        return p

    def counted(C, n):                              # by L's location log, or by C's own while C is a repository
        return C.uuid() in L.whereis(keys[n]) or (os.path.exists(os.path.join(C.root, 'HEAD'))
                                                  and C.uuid() in C.whereis(keys[n]))

    def complete(C, n):                             # SHA256E: the key names the hash of the content
        if not os.path.exists(C.obj(keys[n])):
            return False
        with open(C.obj(keys[n]), 'rb') as h:
            return hashlib.sha256(h.read()).hexdigest() == keys[n].split('--')[1].split('.')[0]

    p = ingest(C2, first)
    os.killpg(p.pid, signal.SIGKILL)
    p.wait()
    partial = C2.tmp(keys[first[1]])
    state = {}
    for n in first:
        fs = L.r('annex', 'fsck', '--numcopies=1', '--from', 'C2', '--key', keys[n])
        state[n] = (c := counted(C2, n), complete(C2, n))
        check(fs.returncode == 0, f'fsck --from C2 {n}: {said(fs, "fail")}')
        check(state[n][1] or not c, f'{n} is counted at C2 but not complete there')
    state = {n: ('counted' if c else 'not counted') + (', complete' if ok else '') for n, (c, ok) in state.items()}
    check(state[first[1]] == 'not counted' and partial, f'{first[1]}, in flight, is counted at C2 or left no partial')
    d = L.r('annex', 'drop', '--key', keys[first[1]])
    check(d.returncode != 0 and L.present(keys[first[1]]), 'the verified drop of the partial key succeeds')
    note(f'kill -9 of copy --to C2 during {first[1]}: {state}; {partial} bytes in C2\'s annex/tmp')

    p = ingest(C3, second)                          # Remote/Git.hs gen: the UUID is checked once, at start
    wipe(C3.root)
    c = finished(p)
    left = sorted(os.path.relpath(os.path.join(d, f), C3.root) for d, _, fs in os.walk(C3.root) for f in fs)
    claimed = [n for n in second if counted(C3, n)]
    check(c.returncode != 0, 'the copy to a deleted repository succeeds')
    check(second[1] not in claimed, f'{second[1]}, in flight when C3 was deleted, is counted at C3')
    check(not os.path.exists(os.path.join(C3.root, 'HEAD')), 'C3 is a git repository again')
    fs = {n: L.r('annex', 'fsck', '--numcopies=1', '--from', 'C3', '--key', keys[n]) for n in claimed}
    check(all(f.returncode != 0 for f in fs.values()), 'fsck --from C3 passes for a deleted C3')
    for n in second:
        d = L.r('annex', 'drop', '--key', keys[n])
        check(d.returncode != 0 and L.present(keys[n]), f'the verified drop of {n} succeeds with C3 deleted')
    note(f'C3 deleted during {second[1]}: copy exits {c.returncode} ("{said(c, "failed to send", "cannot")}"), '
         f'then writes the next key into the deleted path, which holds {len(left)} files '
         f'({", ".join(sorted({l.split("/")[1] if "/" in l else l for l in left}))} under annex/) and no repository; '
         f'the location log claims {claimed} at C3, of which {[n for n in claimed if complete(C3, n)]} are '
         f'complete in the husk; fsck --from C3: '
         f'"{said(next(iter(fs.values())), "fail", "not") if fs else ""}"; drop: '
         f'"{said(d, "Could only verify", "Unable to lock down")}"')
    r = L.r('annex', 'copy', '--to', 'C2', *first)
    fs = L.r('annex', 'fsck', '-q', '--numcopies=1', '--from', 'C2', *first)
    check(r.returncode == 0 and fs.returncode == 0 and all(C2.present(keys[n]) for n in first),
          f'the retry to C2 fails: {said(r, "fail")}')
    result(6, 'a copy to C2 killed (-9) mid-transfer: only complete keys are counted (whereis, fsck --from), '
              'the retry resumes; C3 deleted mid-transfer: the key in flight is not counted, but C3 stays '
              'claimed for its earlier key and for the next one, written into a recreated annex/; the verified '
              'drop refuses every key', before)


def main():
    global SUDO
    t_start = time.time()
    lab = os.environ.get('LAB', os.path.expanduser('~/ga-lab22'))
    SUDO = sudo_ok()
    unmount_all(lab)
    wipe(lab)
    try:
        for n, f in (('2', e2), ('3', e3), ('4', e4), ('5', e5), ('6', e6)):
            t = time.time()
            try:
                f(os.path.join(lab, f'e{n}'))
                print(f'  time: {f.__name__} {time.time() - t:.1f} s', flush=True)
            except Exception as e:                  # report and go on with the next check
                fail(f'{type(e).__name__}: {e}')
                RESULTS.append((n, 'FAIL'))
                print(f'FAIL: E22.{n} raised', flush=True)
    finally:
        unmount_all(lab)
    counts = {s: sum(1 for _, r in RESULTS if r == s) for s in ('PASS', 'FAIL', 'SKIP')}
    print(f'E22: {counts["PASS"]} passed, {counts["FAIL"]} failed, {counts["SKIP"]} skipped in '
          f'{time.time() - t_start:.0f} s; ' + sh('git', 'annex', 'version', '--raw').stdout.strip())
    sys.exit(1 if FAILS else 0)


SUDO = False
if __name__ == '__main__':
    main()
