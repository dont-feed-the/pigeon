#!/usr/bin/env python3
"""E19 (issue #1, ADR 0040): install never replaces nor loses a save that races it.

A reference implementation of the install sequence of spec 07 §7.5 (journal, temp
file, move aside, writer scan, stat check, RENAME_NOREPLACE, index refresh, kept
aside) runs against real git-annex repositories, thin and not thin. An adversary
acts at every boundary of the sequence: in-place write, atomic replace, create,
delete, write through a handle opened earlier (closed or held), and a shared
writable mapping. Pigeon is also killed at every boundary and then recovered
twice. A fuzz phase races a writer process against installs and captures.
Asserts: the adversary's last content is never lost; the index never calls a
file clean unless its content is the index's key (after a mapped write, once the
stat changes: macOS does not update it); recovery is idempotent; with the writer
scan, no aside changes after its check; the scan sees writers. Linux (/proc,
renameat2) and macOS (libproc, renamex_np). Environment: LAB, FUZZ (seconds).
"""
import ctypes, hashlib, json, mmap, os, random, shutil, signal, subprocess, sys, time

os.environ['PATH'] = os.path.expanduser('~/git-annex.linux') + ':' + os.environ['PATH']
# git isolated from the person's configuration (E20), with a lab identity for git-annex's commits
os.environ.update(GIT_CONFIG_GLOBAL='/dev/null', GIT_CONFIG_NOSYSTEM='1', GIT_AUTHOR_NAME='lab',
                  GIT_AUTHOR_EMAIL='lab@pigeon', GIT_COMMITTER_NAME='lab', GIT_COMMITTER_EMAIL='lab@pigeon')
LIBC = ctypes.CDLL(None, use_errno=True)
DARWIN = sys.platform == 'darwin'
POINTS = ['prepared', 'journaled', 'aside', 'scanned', 'checked', 'renamed', 'indexed']
ACTIONS = ['inplace', 'atomic', 'create', 'delete', 'oldfd', 'oldfd-held', 'mmap']
SIZE = 64
FAILS, BLIND = [], []
MAPPED_WRITES_CHANGE_STAT = True


def fail(msg):
    FAILS.append(msg)
    print('FAIL:', msg, flush=True)


def rename_noreplace(src, dst):
    """renameat2(RENAME_NOREPLACE), or renamex_np(RENAME_EXCL) on macOS:
    FileExistsError if dst exists."""
    if DARWIN:
        r = LIBC.renamex_np(os.fsencode(src), os.fsencode(dst), 0x4)
    else:
        r = LIBC.renameat2(-100, os.fsencode(src), -100, os.fsencode(dst), 1)
    if r != 0:
        err = ctypes.get_errno()
        raise OSError(err, os.strerror(err), dst)


class Mapping:
    """A writable mapping of a file that keeps no descriptor open (Python's mmap
    keeps a duplicate, which the scan would find instead of the mapping)."""

    def __init__(self, path, flags=mmap.MAP_SHARED):
        LIBC.mmap.restype, LIBC.mmap.argtypes = ctypes.c_void_p, [
            ctypes.c_void_p, ctypes.c_size_t, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int64]
        fd = os.open(path, os.O_RDWR)
        self.addr = LIBC.mmap(None, SIZE, mmap.PROT_READ | mmap.PROT_WRITE, flags, fd, 0)
        os.close(fd)
        if self.addr in (None, ctypes.c_void_p(-1).value):
            raise OSError(ctypes.get_errno(), 'mmap failed', path)

    def write(self, data):
        ctypes.memmove(self.addr, data, len(data))

    def flush(self):
        LIBC.msync(ctypes.c_void_p(self.addr), ctypes.c_size_t(SIZE), 0x10 if DARWIN else 4)  # MS_SYNC

    def close(self):
        LIBC.munmap(ctypes.c_void_p(self.addr), ctypes.c_size_t(SIZE))


def key_of_bytes(data):
    return f'SHA256E-s{len(data)}--{hashlib.sha256(data).hexdigest()}'


def key_of_file(path):
    with open(path, 'rb') as h:
        return key_of_bytes(h.read())


def fsync_path(path, directory=False):
    fd = os.open(path, os.O_RDONLY | (os.O_DIRECTORY if directory else 0))
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


class VinfoStat(ctypes.Structure):          # <sys/proc_info.h>, macOS
    _fields_ = [('dev', ctypes.c_uint32), ('mode', ctypes.c_uint16), ('nlink', ctypes.c_uint16),
                ('ino', ctypes.c_uint64), ('uid', ctypes.c_uint32), ('gid', ctypes.c_uint32),
                ('times', ctypes.c_int64 * 8), ('size', ctypes.c_int64), ('blocks', ctypes.c_int64),
                ('blksize', ctypes.c_int32), ('flags', ctypes.c_uint32), ('gen', ctypes.c_uint32),
                ('rdev', ctypes.c_uint32), ('qspare', ctypes.c_int64 * 2)]


class VnodeInfoPath(ctypes.Structure):
    _fields_ = [('stat', VinfoStat), ('type', ctypes.c_int), ('pad', ctypes.c_int),
                ('fsid', ctypes.c_int32 * 2), ('path', ctypes.c_char * 1024)]


class VnodeFdInfoWithPath(ctypes.Structure):
    _fields_ = [('openflags', ctypes.c_uint32), ('status', ctypes.c_uint32), ('offset', ctypes.c_int64),
                ('type', ctypes.c_int32), ('guardflags', ctypes.c_uint32), ('vip', VnodeInfoPath)]


class RegionWithPathInfo(ctypes.Structure):
    _fields_ = [('protection', ctypes.c_uint32), ('max_protection', ctypes.c_uint32),
                ('inheritance', ctypes.c_uint32), ('flags', ctypes.c_uint32), ('offset', ctypes.c_uint64),
                ('counts', ctypes.c_uint32 * 9), ('share_mode', ctypes.c_uint32),
                ('more', ctypes.c_uint32 * 4), ('address', ctypes.c_uint64), ('size', ctypes.c_uint64),
                ('vip', VnodeInfoPath)]


def writers_darwin(inodes):
    """libproc, as Apple's proc_listpidspath walks it: each process's vnode file
    descriptors (PROC_PIDLISTFDS, PROC_PIDFDVNODEPATHINFO, open flag FWRITE) and its
    vnode-backed regions (PROC_PIDREGIONPATHINFO2) mapped writable. A writable
    private mapping also counts: a false alarm only defers the install."""
    found, pids = set(), (ctypes.c_int32 * 16384)()
    n = LIBC.proc_listallpids(pids, ctypes.sizeof(pids))
    fds, vfi, rwpi = (ctypes.c_int32 * 2 * 8192)(), VnodeFdInfoWithPath(), RegionWithPathInfo()
    for pid in pids[:max(n, 0)]:
        used = LIBC.proc_pidinfo(pid, 1, ctypes.c_uint64(0), fds, ctypes.sizeof(fds))
        for fd, kind in [tuple(x) for x in fds[:max(used, 0) // 8]]:
            if kind == 1 and LIBC.proc_pidfdinfo(pid, fd, 2, ctypes.byref(vfi),
                                                 ctypes.sizeof(vfi)) == ctypes.sizeof(vfi):
                st = vfi.vip.stat
                if (st.dev, st.ino) in inodes and vfi.openflags & 2:
                    found.add((st.dev, st.ino))
        a = 0
        while LIBC.proc_pidinfo(pid, 22, ctypes.c_uint64(a), ctypes.byref(rwpi),
                                ctypes.sizeof(rwpi)) == ctypes.sizeof(rwpi):
            st = rwpi.vip.stat
            if (st.dev, st.ino) in inodes and rwpi.protection & 2:
                found.add((st.dev, st.ino))
                ri = RegionWithPathInfo()        # PATHINFO2 leaves the share mode 0
                if LIBC.proc_pidinfo(pid, 7, ctypes.c_uint64(rwpi.address), ctypes.byref(ri), 96) == 96:
                    SHARE_MODES[pid] = ri.share_mode
            a = rwpi.address + rwpi.size
    return found


SHARE_MODES = {}                            # pid -> share mode of a writable mapping seen
if DARWIN:
    assert (ctypes.sizeof(VnodeFdInfoWithPath), ctypes.sizeof(RegionWithPathInfo)) == (1200, 1272)
    LIBC.proc_listallpids.argtypes = [ctypes.c_void_p, ctypes.c_int]
    LIBC.proc_pidinfo.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_uint64, ctypes.c_void_p, ctypes.c_int]
    LIBC.proc_pidfdinfo.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_void_p, ctypes.c_int]
    LIBC.renamex_np.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint]


def writers(inodes):
    """The (dev, ino) of `inodes` that a process holds open for writing or maps
    shared and writable: /proc/*/fd with the access mode of fdinfo, and /proc/*/maps."""
    if DARWIN:
        return writers_darwin(inodes)
    found = set()
    for pid in os.listdir('/proc'):
        if not pid.isdigit():
            continue
        base = f'/proc/{pid}'
        try:
            fds = os.listdir(base + '/fd')
        except OSError:
            continue
        for fd in fds:
            try:
                st = os.stat(f'{base}/fd/{fd}')
            except OSError:
                continue
            if (st.st_dev, st.st_ino) in inodes:
                try:
                    with open(f'{base}/fdinfo/{fd}') as h:
                        flags = int(next(l for l in h if l.startswith('flags:')).split()[1], 8)
                except (OSError, StopIteration):
                    flags = os.O_RDWR  # unreadable: assume a writer
                if flags & 3:
                    found.add((st.st_dev, st.st_ino))
        try:
            with open(base + '/maps') as h:
                for line in h:
                    f = line.split()
                    if len(f) >= 6 and f[1][1] == 'w' and f[1][3] == 's':
                        major, minor = (int(x, 16) for x in f[3].split(':'))
                        if (os.makedev(major, minor), int(f[4])) in inodes:
                            found.add((os.makedev(major, minor), int(f[4])))
        except OSError:
            pass
    return found


class Repo:
    """A laptop repository with unlocked annexed files, and pigeon's install door."""

    def __init__(self, root, thin, scan=True):
        self.root, self.thin, self.scan = root, thin, scan
        self.pg = os.path.join(root, '.git/pigeon')
        self.journal = os.path.join(self.pg, 'install.ndjson')
        self.locs, self.keys, self.windows, self.late = {}, {}, [], 0

    def git(self, *args, check=True, stdin=None):
        r = subprocess.run(['git', '-c', 'user.name=lab', '-c', 'user.email=lab@pigeon', *args],
                           cwd=self.root, capture_output=True, text=True, input=stdin)
        if check and r.returncode:
            raise RuntimeError(f'git {args}: {r.stderr.strip()}')
        return r.stdout

    def path(self, rel):
        return os.path.join(self.root, rel)

    def setup(self, names, contents):
        """Each name gets content contents[name][0]; every other content is put in the annex."""
        os.makedirs(self.root)
        self.git('init', '-q')
        self.git('annex', 'init', '-q', 'laptop')
        self.git('config', 'annex.addunlocked', 'true')
        self.git('config', 'annex.thin', 'true' if self.thin else 'false')
        for name in names:
            for i, data in enumerate(contents[name]):
                p = self.path(f'store/{i}/{name}' if i else name)
                os.makedirs(os.path.dirname(p), exist_ok=True)
                with open(p, 'wb') as h:
                    h.write(data)
        self.git('annex', 'add', '-q', '.')
        self.git('commit', '-qm', 'contents')
        self.git('rm', '-rq', '--cached', 'store')
        shutil.rmtree(self.path('store'))
        self.git('commit', '-qm', 'only the folder')
        for d in ('stage', 'aside', 'recovered'):
            os.makedirs(os.path.join(self.pg, d), exist_ok=True)
        time.sleep(0.05)
        self.git('update-index', '-q', '--really-refresh', check=False)

    def object(self, key):
        if key not in self.locs:
            self.locs[key] = self.path(self.git('annex', 'contentlocation', key).strip())
        return self.locs[key]

    def entry(self, rel):
        """The index entry of rel: mode, key and stat data, or None."""
        out = self.git('ls-files', '-s', '--debug', '--', rel)
        if not out:
            return None
        lines = out.splitlines()
        mode, blob = lines[0].split()[:2]
        f = {}
        for line in lines[1:]:
            parts = line.replace('\t', ' ').split()
            for k, v in zip(parts[0::2], parts[1::2]):
                f[k.rstrip(':')] = v
        sec, nsec = (int(x) for x in f['mtime'].split(':'))
        if blob not in self.keys:
            self.keys[blob] = self.git('cat-file', '-p', blob).strip().rsplit('/', 1)[1]
        key = self.keys[blob]
        return {'mode': mode, 'key': key, 'dev': int(f['dev']), 'ino': int(f['ino']),
                'size': int(f['size']), 'mtime': sec * 10**9 + nsec}

    def append(self, rec):
        with open(self.journal, 'a') as h:
            h.write(json.dumps({'format': 1, **rec}) + '\n')
            h.flush()
            os.fsync(h.fileno())

    def set_index(self, rel, mode, key):
        """Point rel's entry at key without stat data; git's refresh then records the
        stat only if the file's content is that key (racy-git rule, clean filter)."""
        blob = self.git('hash-object', '-w', '--stdin', stdin=f'/annex/objects/{key}\n').strip()
        self.git('update-index', '--cacheinfo', f'{mode},{blob},{rel}')
        self.git('update-index', '-q', '--refresh', check=False)

    def install(self, rel, new_key, hook=lambda point: None, seq=None):
        """One path of spec 07 §7.5. Returns the outcome."""
        self.git('update-index', '-q', '--refresh', check=False)
        e = self.entry(rel)
        if self.git('diff-files', '--name-only', '--', rel).strip() or e is None:
            return 'not-clean'
        if os.stat(os.path.join(self.root, '.git/index')).st_mtime_ns <= e['mtime']:
            return 'racy'
        seq = seq if seq is not None else time.time_ns()
        tmp, aside = os.path.join(self.pg, f'stage/{seq}'), os.path.join(self.pg, f'aside/{seq}')
        p = self.path(rel)
        obj = self.object(new_key)
        if self.thin:
            os.link(obj, tmp)
        else:
            shutil.copyfile(obj, tmp)
            fsync_path(tmp)
        os.chmod(tmp, 0o755 if e['mode'] == '100755' else 0o644)
        tmp_ino = os.stat(tmp).st_ino
        hook('prepared')
        self.append({'seq': seq, 'op': 'begin', 'path': rel, 'mode': e['mode'], 'old_key': e['key'],
                     'old': [e['dev'], e['ino'], e['size'], e['mtime']], 'new_key': new_key,
                     'tmp': tmp, 'tmp_ino': tmp_ino, 'aside': aside})
        hook('journaled')
        try:
            os.rename(p, aside)
        except FileNotFoundError:            # the person deleted p: a save
            os.unlink(tmp)
            self.append({'seq': seq, 'op': 'skip'})
            return 'deleted-first'
        t0 = time.perf_counter()
        hook('aside')
        st = os.lstat(aside)
        busy = self.scan and writers({(st.st_dev, st.st_ino)})
        hook('scanned')
        st = os.lstat(aside)
        m = 0xffffffff                         # the index keeps 32 bits of dev and ino
        own = (st.st_dev & m, st.st_ino & m, st.st_size, st.st_mtime_ns) == \
            (e['dev'] & m, e['ino'] & m, e['size'], e['mtime']) \
            and st.st_nlink == (2 if self.thin else 1)
        if busy or not own:
            outcome = self.put_back(seq, rel, aside)
            os.unlink(tmp)
            self.windows.append(time.perf_counter() - t0)
            return ('busy-' if busy and own else 'saved-') + outcome
        hook('checked')
        try:
            rename_noreplace(tmp, p)
        except FileExistsError:              # the person created p meanwhile
            os.unlink(tmp)
            self.windows.append(time.perf_counter() - t0)
            self.append({'seq': seq, 'op': 'raced'})
            return 'raced'
        self.windows.append(time.perf_counter() - t0)
        fsync_path(os.path.dirname(p), True)
        fsync_path(os.path.dirname(aside), True)
        hook('renamed')
        self.set_index(rel, e['mode'], new_key)
        hook('indexed')
        self.append({'seq': seq, 'op': 'done'})
        return 'installed'

    def put_back(self, seq, rel, aside):
        try:
            rename_noreplace(aside, self.path(rel))
            self.append({'seq': seq, 'op': 'skip'})
            return 'put-back'
        except FileExistsError:
            self.keep_recovered(seq, aside)
            return 'recovered'

    def keep_recovered(self, seq, aside):
        dest = os.path.join(self.pg, f'recovered/{seq}')
        os.rename(aside, dest)
        self.append({'seq': seq, 'op': 'recovered'})

    def entries(self):
        """seq -> begin record and its later ops, from the journal."""
        out = {}
        if os.path.exists(self.journal):
            for line in open(self.journal):
                try:
                    rec = json.loads(line)
                except ValueError:
                    continue                 # torn last line
                if rec['op'] == 'begin':
                    out[rec['seq']] = dict(rec, ops=[])
                elif rec['seq'] in out:
                    out[rec['seq']]['ops'].append(rec['op'])
        return out

    def recover(self):
        """The start-up rule of spec 07 §7.5 for every begun entry without an end."""
        ends = {'done', 'skip', 'raced', 'recovered'}
        for seq, e in self.entries().items():
            if ends & set(e['ops']):
                continue
            p, aside, tmp = self.path(e['path']), e['aside'], e['tmp']
            if os.path.exists(os.path.join(self.pg, f'recovered/{seq}')):
                self.append({'seq': seq, 'op': 'recovered'})
                continue
            try:
                p_ino = os.lstat(p).st_ino
            except FileNotFoundError:
                p_ino = None
            if p_ino == e['tmp_ino']:        # renamed in: the check had passed
                self.set_index(e['path'], e['mode'], e['new_key'])
                self.append({'seq': seq, 'op': 'done'})
            elif os.path.exists(aside) and p_ino is None:
                self.put_back(seq, e['path'], aside)
            elif os.path.exists(aside):      # p is the person's; the aside is unchecked
                if key_of_file(aside) == e['old_key']:
                    self.append({'seq': seq, 'op': 'raced'})
                else:
                    self.keep_recovered(seq, aside)
            else:
                self.append({'seq': seq, 'op': 'skip'})
            if os.path.exists(tmp) and os.lstat(tmp).st_ino != p_ino:
                os.unlink(tmp)
        live = {os.path.basename(e['tmp']) for e in self.entries().values()}
        for name in os.listdir(os.path.join(self.pg, 'stage')):
            if name not in live:
                os.unlink(os.path.join(self.pg, 'stage', name))
        self.git('update-index', '-q', '--refresh', check=False)

    def release(self):
        """Expiry of kept asides: unlink one only if no process writes it and its
        content is still the old key; otherwise it is a recovered version."""
        for seq, e in self.entries().items():
            aside = e['aside']
            if not os.path.exists(aside) or 'released' in e['ops']:
                continue
            st = os.lstat(aside)
            if writers({(st.st_dev, st.st_ino)}):
                continue
            if key_of_file(aside) == e['old_key']:
                os.unlink(aside)
                self.append({'seq': seq, 'op': 'released'})
            else:
                self.late += 1
                self.keep_recovered(seq, aside)

    def lies(self, rels):
        """Paths that git calls clean although their content is not the index's key."""
        self.git('update-index', '-q', '--refresh', check=False)
        dirty = set(self.git('diff-files', '--name-only').split())
        bad = []
        for rel in rels:
            e, p = self.entry(rel), self.path(rel)
            if e and os.path.exists(p) and rel not in dirty and key_of_file(p) != e['key']:
                bad.append(rel)
        return bad

    def holds(self, data, rel):
        """data is at rel, or kept as a recovered version or a kept aside."""
        rec = os.path.join(self.pg, 'recovered')
        places = [self.path(rel)] + [os.path.join(rec, n) for n in os.listdir(rec)] + \
            [e['aside'] for e in self.entries().values() if os.path.exists(e['aside'])]
        for place in places:
            try:
                if open(place, 'rb').read() == data:
                    return True
            except OSError:
                pass
        return False


def scan_selftest(lab):
    """The writer scan sees another process's write handle and its shared writable
    mapping (handle closed), and ignores a read-only handle."""
    p = os.path.join(lab, 'scan-probe')
    with open(p, 'wb') as h:
        h.write(b'\0' * SIZE)
    st = os.stat(p)
    for how, expect in (('read', False), ('write', True), ('mmap', True), ('private', None)):
        r, w = os.pipe()
        pid = os.fork()
        if pid == 0:
            if how in ('mmap', 'private'):
                m = Mapping(p, mmap.MAP_SHARED if how == 'mmap' else mmap.MAP_PRIVATE)
            else:
                fd = os.open(p, os.O_RDONLY if how == 'read' else os.O_RDWR)
            os.write(w, b'x')
            time.sleep(60)
            os._exit(0)
        os.read(r, 1)
        t0 = time.perf_counter()
        seen = bool(writers({(st.st_dev, st.st_ino)}))
        ms = 1000 * (time.perf_counter() - t0)
        os.kill(pid, signal.SIGKILL)
        os.waitpid(pid, 0)
        os.close(r), os.close(w)
        mode = f', share mode {SHARE_MODES[pid]}' if pid in SHARE_MODES else ''
        print(f'scan: a {how} handle of another process is {"seen" if seen else "not seen"} ({ms:.0f} ms{mode})')
        if expect is not None and seen != expect:
            fail(f'the writer scan {"misses" if expect else "flags"} a {how} handle')
    return mapped_write_changes_stat(p)


def mapped_write_changes_stat(p):
    """Whether a write through a shared mapping changes the file's stat data (mtime,
    ctime), which is all git's refresh and capture's scan look at: right after the
    write, after munmap without msync (the adversary's case), and after msync."""
    stamp = lambda: (os.stat(p).st_mtime_ns, os.stat(p).st_ctime_ns)
    seen = []
    for sync in (False, True):
        before = stamp()
        time.sleep(0.05)
        m = Mapping(p)
        m.write(b'map!' if sync else b'MAP!')
        seen.append(('write', stamp() != before))
        if sync:
            m.flush()
        m.close()
        seen.append(('msync' if sync else 'munmap without msync', stamp() != before))
    print('stat after a write through a shared mapping: ' +
          ', '.join(f'{k} {"changed" if v else "unchanged"}' for k, v in seen))
    return seen[0][1]


def pad(text):
    return text.ljust(SIZE).encode()


def contents_for(names, n):
    return {name: [pad(f'v{i} {name}') for i in range(n)] for name in names}


class Adversary:
    """An application acting once on rel: at `point` of the install, or right
    after it if the install never reached that point."""

    def __init__(self, repo, rel, action, tag):
        self.p, self.action, self.tag = repo.path(rel), action, tag
        self.acted = self.wrote = self.deleted = False
        self.fd = self.map = None
        if action in ('oldfd', 'oldfd-held'):
            self.fd = os.open(self.p, os.O_RDWR)
        if action == 'mmap':
            self.map = Mapping(self.p)

    def act(self):
        self.acted, a, p = True, self.action, self.p
        try:
            if a == 'inplace' or (a == 'create' and os.path.exists(p)):
                fd = os.open(p, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o644)
                os.write(fd, self.tag)
                os.close(fd)
            elif a == 'create':
                fd = os.open(p, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o644)
                os.write(fd, self.tag)
                os.close(fd)
            elif a == 'atomic':
                with open(p + '.sv', 'wb') as h:
                    h.write(self.tag)
                os.rename(p + '.sv', p)
            elif a == 'delete':
                os.unlink(p)
                self.deleted = True
                return
            elif a in ('oldfd', 'oldfd-held'):
                os.pwrite(self.fd, self.tag, 0)
                if a == 'oldfd':
                    os.close(self.fd)
                    self.fd = None
            elif a == 'mmap':
                self.map.write(self.tag)
            self.wrote = True
        except FileNotFoundError:            # the name was absent for an instant
            pass

    def hook(self, point):
        return lambda pt: self.act() if pt == point else None

    def finish(self):
        if not self.acted:
            self.act()
        if self.fd is not None:
            os.close(self.fd)
        if self.map is not None:
            self.map.close()


def boundary_trials(repo, names, contents, label):
    """Every action at every boundary, one fresh path each."""
    outcomes = {}
    for (action, point), rel in zip([(a, pt) for a in ACTIONS for pt in POINTS], names):
        tag = pad(f'person {label} {action} {point}')
        adv = Adversary(repo, rel, action, tag)
        new_key = key_of_bytes(contents[rel][1])
        outcome = repo.install(rel, new_key, adv.hook(point))
        adv.finish()
        repo.release()
        outcomes.setdefault(outcome, []).append(f'{action}@{point}')
        if adv.wrote and not repo.holds(tag, rel):
            fail(f'{label}: {action} at {point}: the save is lost ({outcome})')
        p = repo.path(rel)
        if not os.path.exists(p) and not adv.deleted:
            fail(f'{label}: {action} at {point}: the file is missing ({outcome})')
        if os.path.exists(p) and open(p, 'rb').read() not in (tag, *contents[rel]):
            fail(f'{label}: {action} at {point}: unexpected content')
        if repo.lies([rel]) and action == 'mmap' and not MAPPED_WRITES_CHANGE_STAT:
            e, st = repo.entry(rel), os.stat(p)  # the platform's blind spot, not the install's:
            if (st.st_mtime_ns, st.st_size) != (e['mtime'], e['size']) and repo.lies([rel]):
                fail(f'{label}: mmap at {point}: git calls a changed file clean, and its stat changed')
            BLIND.append(f'{label} {point}')
            os.utime(p)                          # once the stat changes, git must see the change
            if repo.lies([rel]):
                fail(f'{label}: mmap at {point}: git calls a changed file clean after its stat changed')
        elif repo.lies([rel]):
            fail(f'{label}: {action} at {point}: git calls a changed file clean')
    return outcomes


def snapshot(repo, rel):
    p = repo.path(rel)
    open_entries = [s for s, e in repo.entries().items()
                    if not {'done', 'skip', 'raced', 'recovered'} & set(e['ops'])]
    return (open(p, 'rb').read() if os.path.exists(p) else None, repo.entry(rel)['key'],
            sorted(os.listdir(os.path.join(repo.pg, 'aside'))),
            sorted(os.listdir(os.path.join(repo.pg, 'recovered'))),
            sorted(os.listdir(os.path.join(repo.pg, 'stage'))), open_entries)


def crash_trials(repo, names, contents, label):
    """kill -9 at every boundary, optionally a save before the restart, then
    recovery twice."""
    cases = [(pt, after) for pt in POINTS for after in ('none', 'create', 'inplace')]
    for (point, after), rel in zip(cases, names):
        tag, p = pad(f'person {label} crash {point} {after}'), repo.path(rel)
        new_key = key_of_bytes(contents[rel][1])
        pid = os.fork()
        if pid == 0:
            try:
                repo.install(rel, new_key, lambda pt: os.kill(os.getpid(), signal.SIGKILL)
                             if pt == point else None)
            finally:
                os._exit(3)
        os.waitpid(pid, 0)
        wrote = False
        if after == 'create' and not os.path.exists(p):
            with open(p, 'xb') as h:
                h.write(tag)
            wrote = True
        elif after == 'inplace' and os.path.exists(p):
            with open(p, 'r+b') as h:
                h.write(tag)
            wrote = True
        repo.recover()
        first = snapshot(repo, rel)
        repo.recover()
        if snapshot(repo, rel) != first:
            fail(f'{label}: crash at {point}, {after}: a second recovery changed the state')
        if first[5] or first[4]:
            fail(f'{label}: crash at {point}, {after}: open journal entries or temp files remain')
        if first[0] not in (tag, *contents[rel]):
            fail(f'{label}: crash at {point}, {after}: the file is {first[0]!r}')
        repo.release()
        if wrote and not repo.holds(tag, rel):
            fail(f'{label}: crash at {point}, {after}: the save is lost')
        if repo.lies([rel]):
            fail(f'{label}: crash at {point}, {after}: git calls a changed file clean')


def fuzz(repo, names, contents, seconds):
    """A writer process saves at random while installs and captures run."""
    deadline, log = time.time() + seconds, repo.root + '.writer.json'
    pid = os.fork()
    if pid == 0:
        rng, last, n = random.Random(1), {}, 0
        try:
            while time.time() < deadline:
                rel, n = rng.choice(names), n + 1
                p, tag, a = repo.path(rel), pad(f'fuzz {n}'), rng.choice(['inplace', 'atomic', 'held'])
                try:
                    if a == 'atomic':
                        with open(p + '.sv', 'wb') as h:
                            h.write(tag)
                        os.rename(p + '.sv', p)
                    else:
                        fd = os.open(p, os.O_RDWR)
                        if a == 'held':
                            time.sleep(rng.uniform(0, 0.03))
                        os.pwrite(fd, tag, 0)
                        if a == 'held':
                            time.sleep(rng.uniform(0, 0.03))
                        os.close(fd)
                    last[rel] = tag.decode()
                except FileNotFoundError:
                    pass
                time.sleep(rng.uniform(0, 0.4))
        finally:
            with open(log, 'w') as h:
                json.dump(last, h)
            os._exit(0)
    rng, stats, captured = random.Random(2), {}, set()
    while time.time() < deadline:
        rel = rng.choice(names)
        current = repo.entry(rel)['key']
        out = repo.install(rel, rng.choice([key_of_bytes(c) for c in contents[rel]
                                            if key_of_bytes(c) != current]))
        stats[out] = stats.get(out, 0) + 1
        if out != 'installed':               # capture what the person saved
            repo.git('annex', 'add', '-q', '--', rel, check=False)
            captured.add(repo.entry(rel)['key'])
        if rng.random() < 0.2:
            repo.release()
    os.waitpid(pid, 0)
    repo.release()
    for rel in names:                        # the last saves are captured at the end
        repo.git('annex', 'add', '-q', '--', rel, check=False)
        captured.add(repo.entry(rel)['key'])
    for rel, tag in json.load(open(log)).items():
        if key_of_bytes(tag.encode()) not in captured and not repo.holds(tag.encode(), rel):
            fail(f'fuzz: the last save of {rel} is lost')
    if repo.lies(names):
        fail('fuzz: git calls a changed file clean')
    if repo.late:
        fail(f'fuzz: {repo.late} asides changed after their check')
    return stats


def main():
    lab = os.environ.get('LAB', os.path.expanduser('~/ga-lab19'))
    if os.path.exists(lab):
        subprocess.run(['chmod', '-R', 'u+w', lab])
        shutil.rmtree(lab)
    os.makedirs(lab)
    global MAPPED_WRITES_CHANGE_STAT
    MAPPED_WRITES_CHANGE_STAT = scan_selftest(lab)
    nb, nc = len(ACTIONS) * len(POINTS), len(POINTS) * 3
    windows = []
    for thin in (False, True):
        label = 'thin' if thin else 'copy'
        names = [f'Docs/t{i:03d}' for i in range(2 * nb + nc)]
        contents = contents_for(names, 2)
        repo = Repo(os.path.join(lab, label), thin)
        repo.setup(names, contents)
        for outcome, cases in sorted(boundary_trials(repo, names[:nb], contents, label).items()):
            print(f'{label}: {outcome}: {len(cases)} ({", ".join(cases[:4])}…)')
        if repo.late:
            fail(f'{label}: {repo.late} asides changed after their check despite the writer scan')
        crash_trials(repo, names[nb:nb + nc], contents, label)
        print(f'{label}: {nc} kill -9 trials recovered')
        repo.scan, repo.late = False, 0
        boundary_trials(repo, names[nb + nc:], contents, label + ' without scan')
        print(f'{label} without scan: {repo.late} writes through old handles landed in an aside,'
              ' kept as recovered versions')
        if repo.late == 0:
            fail(f'{label}: without the scan no write reached an aside; the trials miss the race')
        windows += repo.windows
    names = [f'Docs/f{i}' for i in range(8)]
    contents = contents_for(names, 3)
    repo = Repo(os.path.join(lab, 'fuzz'), False)
    repo.setup(names, contents)
    stats = fuzz(repo, names, contents, float(os.environ.get('FUZZ', '20')))
    print('fuzz:', ', '.join(f'{k} {v}' for k, v in sorted(stats.items())))
    windows = sorted(windows + repo.windows)
    print(f'name absent while installing: median {1000 * windows[len(windows) // 2]:.2f} ms,'
          f' max {1000 * windows[-1]:.2f} ms over {len(windows)} installs')
    if BLIND:
        print(f'SKIP (platform): in {len(BLIND)} mmap trials git calls the file clean although a shared'
              ' mapping changed it, because its stat data did not change; each save is kept, and git sees'
              ' the change once the stat changes (13 §13.8)')
    print('FAILED' if FAILS else 'ok: E19 passed')
    sys.exit(1 if FAILS else 0)


if __name__ == '__main__':
    main()
