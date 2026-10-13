#!/usr/bin/env python3
"""E20 (issue #14; ADR 0044): what git and git-annex do with each kind of
file-system object a shared folder may hold, on Linux and macOS.

Laptop repositories are set up as spec 03 §3.3–3.4 says; capture adds with
`git -c annex.thin=false annex add --batch -z --json` and changes are observed
with `git status --porcelain=v2 -z -uall`. Asserts, citing git-annex
10.20261006's source:
- regular files become SHA256E pointers and keep only the owner's execute bit;
- empty directories, FIFOs, sockets and a nested repository's files are never
  listed; symlinks are listed, and add stages them as git symlinks, or as locked
  annexed files when they point into the annex, so pigeon must filter them;
- hard links share a key; a thin get links at most one path to an object;
- xattrs, resource forks and Finder tags do not travel; unreadable paths are
  never reported deleted; in-tree .gitignore and .gitattributes files are
  neutralised by `--ignored=traditional`, `--no-check-gitignore` and
  `.git/info/attributes`; a file rewritten during add is refused or captured
  whole; the bytes status reports for awkward names.
Exits non-zero on failure. Environment: LAB (default ~/ga-lab20), E20_TRIALS.
"""
import errno, hashlib, json, os, random, shutil, socket, stat, subprocess, sys, threading, time
import unicodedata

GA = os.path.expanduser('~/git-annex.linux')
if os.path.isdir(GA):
    os.environ['PATH'] = GA + os.pathsep + os.environ['PATH']
LAB = os.path.expanduser(os.environ.get('LAB', '~/ga-lab20'))
TRIALS = int(os.environ.get('E20_TRIALS', '15'))
MACOS = sys.platform == 'darwin'
LINUX = sys.platform.startswith('linux')
ROOT = os.geteuid() == 0
COUNTS = {'PASS': 0, 'FAIL': 0, 'SKIP': 0}
GENESIS = b'* annex.largefiles=anything\n.gitattributes annex.largefiles=nothing\n.pigeon/** annex.largefiles=nothing\n'
# pigeon's override: info/attributes outranks every in-tree .gitattributes (gitattributes(5)).
OVERRIDE = (b'* filter=annex annex.largefiles=anything annex.backend=SHA256E'
            b' -text -eol -ident -working-tree-encoding\n'
            b'/.gitattributes annex.largefiles=nothing\n/.pigeon/** annex.largefiles=nothing\n')
MINIMAL = (b'* filter=annex annex.largefiles=anything annex.backend=SHA256E -working-tree-encoding\n'
           b'/.gitattributes annex.largefiles=nothing\n/.pigeon/** annex.largefiles=nothing\n')


def report(kind, msg):
    COUNTS[kind] += 1
    print(f'{kind}: {msg}', flush=True)


def check(cond, msg):
    report('PASS' if cond else 'FAIL', msg)
    return cond


def skip(msg):
    report('SKIP', msg)


def note(msg):
    print(f'NOTE: {msg}', flush=True)


def run(args, cwd, input=None, timeout=120, check=True):
    r = subprocess.run(args, cwd=cwd, input=input, capture_output=True, timeout=timeout)
    if check and r.returncode:
        raise RuntimeError(f'{cwd}: {args}: {r.stderr.decode(errors="replace").strip()}')
    return r


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for block in iter(lambda: f.read(1 << 20), b''):
            h.update(block)
    return h.hexdigest()


def key_hash(key):
    return key.split('--', 1)[1].split('.', 1)[0]


class Repo:
    def __init__(self, path):
        self.path = path

    def p(self, rel):
        return os.path.join(self.path.encode() if isinstance(rel, bytes) else self.path, rel)

    def git(self, *args, input=None, check=True, timeout=120):
        return run(['git', '-c', 'user.name=lab', '-c', 'user.email=lab@pigeon', *args], self.path,
                   input=input, check=check, timeout=timeout)

    def out(self, *args):
        return self.git(*args).stdout.decode().strip()

    def write(self, rel, data, mode=None):
        path = self.p(rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, 'wb') as f:
            f.write(data)
        if mode is not None:
            os.chmod(path, mode)

    def read(self, rel):
        with open(self.p(rel), 'rb') as f:
            return f.read()

    def status(self, *extra):
        """Entries of `git status --porcelain=v2 -z -uall` as (kind, fields, path), and stderr."""
        r = self.git('status', '--porcelain=v2', '-z', '-uall', *extra)
        items, entries = r.stdout.split(b'\0'), []
        i = 0
        while i < len(items):
            item = items[i]
            i += 1
            if not item:
                continue
            kind = item[:1].decode()
            if kind in '?!':
                entries.append((kind, [], item[2:]))
            elif kind == '1':
                f = item.split(b' ', 8)
                entries.append((kind, [x.decode() for x in f[1:8]], f[8]))
            elif kind == '2':
                f = item.split(b' ', 9)
                entries.append((kind, [x.decode() for x in f[1:9]], f[9]))
                i += 1  # the rename's origin path
            else:
                entries.append((kind, [], item))
        return entries, r.stderr.decode(errors='replace')

    def paths(self, *extra):
        return [path for _, _, path in self.status(*extra)[0]]

    def capture(self, paths, *opts, timeout=120):
        """What capture does: one JSON result per path, None where add printed nothing."""
        data = b''.join((p if isinstance(p, bytes) else p.encode()) + b'\0' for p in paths)
        r = self.git('-c', 'annex.thin=false', 'annex', 'add', '--batch', '-z', '--json', *opts,
                     input=data, check=False, timeout=timeout)
        lines = r.stdout.split(b'\n')[:len(paths)]
        res = [json.loads(line) if line.strip() else None for line in lines]
        return res + [None] * (len(paths) - len(res)), r.stderr.decode(errors='replace')

    def staged(self):
        """Index entries: path -> (mode, blob id)."""
        out = {}
        for item in self.git('ls-files', '-s', '-z').stdout.split(b'\0'):
            if item:
                meta, path = item.split(b'\t', 1)
                mode, sha, _ = meta.decode().split(' ')
                out[path] = (mode, sha)
        return out

    def blob(self, sha):
        return self.git('cat-file', 'blob', sha).stdout

    def kind(self, rel):
        """'SHA256E', 'WORM', ... for an annex pointer, 'blob' for plain content, None if absent."""
        entry = self.staged().get(rel.encode() if isinstance(rel, str) else rel)
        if not entry:
            return None
        data = self.blob(entry[1])
        if data.startswith(b'/annex/objects/'):
            return data[len(b'/annex/objects/'):].split(b'-', 1)[0].decode()
        return 'blob'

    def commit(self, msg='lab'):
        self.git('commit', '-q', '--allow-empty', '-m', msg)

    def key(self, rel):
        return self.out('annex', 'lookupkey', rel)

    def obj(self, key):
        return self.p(self.out('annex', 'contentlocation', key))


def configure(repo, name, attributes=None):
    for k, v in [('annex.addunlocked', 'true'), ('annex.thin', 'true'), ('annex.dotfiles', 'true'),
                 ('annex.autocommit', 'false'), ('annex.synccontent', 'false'),
                 ('annex.resolvemerge', 'false')]:
        repo.git('config', k, v)
    repo.git('annex', 'init', '-q', name)
    info = os.path.join(repo.path, '.git', 'info', 'attributes')
    with open(info, 'rb') as f:
        written = f.read()
    if attributes is not None:
        with open(info, 'wb') as f:
            f.write(attributes)
    return written


def laptop(name, attributes=None, precompose=None):
    path = os.path.join(LAB, name)
    run(['git', 'init', '-q', '-b', 'main', path], LAB)
    repo = Repo(path)
    if precompose is not None:
        repo.git('config', 'core.precomposeunicode', 'true' if precompose else 'false')
    repo.write('.gitattributes', GENESIS)
    repo.init_attributes = configure(repo, name, attributes)
    repo.git('add', '.gitattributes')
    repo.commit('genesis')
    return repo


def clone(src, name, attributes=None):
    path = os.path.join(LAB, name)
    run(['git', 'clone', '-q', src.path, path], LAB)
    repo = Repo(path)
    configure(repo, name, attributes)
    repo.git('annex', 'get', '-q', '.', check=False)
    return repo


def c1_regular_files():
    """Regular files, modes, empty directories."""
    r = laptop('r1')
    r.write('plain.txt', b'plain\n')
    r.write('run.sh', b'#!/bin/sh\necho hi\n', 0o755)
    r.write('secret.txt', b'secret\n', 0o600)
    r.write('ro.txt', b'read only\n', 0o444)
    names = ['plain.txt', 'run.sh', 'secret.txt', 'ro.txt']
    res, _ = r.capture(names)
    staged = r.staged()
    check(all(r.kind(n) == 'SHA256E' for n in names), 'regular files are captured as SHA256E pointers')
    modes = {n: staged[n.encode()][0] for n in names}
    check(set(modes.values()) == {'100644'},
          'git annex add stages an executable unlocked file as 100644: ingestAdd\' (Annex/Ingest.hs) '
          'stats contentLocation after cleanCruft removed the locked-down hard link, so the mode '
          f'is Nothing (an upstream bug): {modes}')
    r.write('viagit.sh', b'#!/bin/sh\n', 0o755)
    r.git('add', 'viagit.sh')
    check(r.staged()[b'viagit.sh'][0] == '100755' and r.kind('viagit.sh') == 'SHA256E',
          'git add through the clean filter keeps the execute bit (100755)')
    r.git('update-index', '--chmod=+x', 'run.sh')
    check(r.staged()[b'run.sh'] == ('100755', staged[b'run.sh'][1]),
          'update-index --chmod=+x stages the bit and keeps the pointer: what capture must do')
    r.commit()
    pointer = staged[b'plain.txt'][1]
    os.chmod(r.p('plain.txt'), 0o755)
    entry = [f for k, f, p in r.status()[0] if p == b'plain.txt']
    check(entry and entry[0][0] == '.M' and entry[0][4] == '100755',
          f'chmod +x shows as a mode change in status: {entry}')
    r.capture(['plain.txt'])
    after = r.staged()[b'plain.txt']
    check(after == ('100644', pointer), f'git annex add does not stage a mode-only change: {after}')
    r.git('update-index', '--chmod=+x', 'plain.txt')
    dirty = [f[0] for k, f, p in r.status()[0] if p == b'plain.txt' and f[0][1] != '.']
    check(r.staged()[b'plain.txt'] == ('100755', pointer) and not dirty,
          'update-index --chmod=+x captures it with the same key, and the work tree is clean')
    r.commit()
    os.makedirs(r.p('empty/inner'))
    check(not [p for p in r.paths() if p.startswith(b'empty')], 'an empty directory is never listed by status')
    check(not [p for p in r.paths('--ignored=traditional') if p.startswith(b'empty')],
          'nor with --ignored=traditional')
    old = os.umask(0)
    os.umask(old)
    c = clone(r, 'r1c')
    same = all(c.read(n) == r.read(n) for n in names)
    xbits = {n: bool(os.stat(c.p(n)).st_mode & 0o100) for n in names}
    check(same and xbits == {'plain.txt': True, 'run.sh': True, 'secret.txt': False, 'ro.txt': False},
          f'a clone and get give the content and the execute bit: {xbits}')
    perms = {n: oct(stat.S_IMODE(os.stat(c.p(n)).st_mode)) for n in names}
    note(f'clone modes {perms} (umask {oct(old)}): a get from a local path copies the object file, '
         'whose bits the tree does not record; over P2P the receiver writes a new file')
    check(not os.path.exists(c.p('empty')), 'an empty directory does not reach a clone')


def c3_symlinks():
    r = laptop('r3')
    r.write('a.txt', b'hello\n')
    r.write('d/f.txt', b'in d\n')
    r.capture(['a.txt', 'd/f.txt'])
    r.commit()
    key = r.key('a.txt')
    objrel = os.path.relpath(r.obj(key), r.path)
    links = {'rel': 'a.txt', 'abs': '/etc/hosts', 'dangling': 'nowhere', 'todir': 'd', 'annexlink': objrel}
    for name, target in links.items():
        os.symlink(target, r.p(name))
    listed = r.paths()
    check(all(n.encode() in listed for n in links), f'status lists every symlink as untracked: {listed}')
    check(b'todir/f.txt' not in listed, 'status does not follow a symlink to a directory')
    res, _ = r.capture(list(links))
    staged = r.staged()
    modes = {n: staged.get(n.encode(), ('-',))[0] for n in links}
    check(all(m == '120000' for m in modes.values()),
          f'add stages every symlink as a git symlink, 120000 (Command/Add.hs addFile TreeSymlink): {modes}')
    got = {n: (x or {}).get('key') for n, x in zip(links, res)}
    check(got['annexlink'] == key and not any(got[n] for n in links if n != 'annexlink'),
          'a symlink into .git/annex/objects is taken as a locked annexed file '
          '(Command/Add.hs start: lookupKey, fixuplink); the others get no key')
    found = r.out('annex', 'find', '--include=*').split('\n')
    check('annexlink' in found, f'git annex find lists the link as annexed content: {found}')
    note('so capture must lstat every listed path and never pass a symlink to add')


def c4_hard_links():
    r = laptop('r4')
    data = os.urandom(1 << 16)
    r.write('h1.bin', data)
    os.link(r.p('h1.bin'), r.p('h2.bin'))
    res, _ = r.capture(['h1.bin', 'h2.bin'])
    keys = [x and x.get('key') for x in res]
    check(keys[0] and keys[0] == keys[1], f'two names of one inode get one key: {keys}')
    key = keys[0]
    ino = {n: os.stat(r.p(n)).st_ino for n in ['h1.bin', 'h2.bin']}
    check(ino['h1.bin'] == ino['h2.bin'] and os.stat(r.obj(key)).st_ino != ino['h1.bin'],
          'on the capturing laptop the names stay linked and the object is a separate copy')
    r.commit()
    c = clone(r, 'r4c')
    obj = c.obj(key)
    oino = os.stat(obj).st_ino
    linked = [n for n in ['h1.bin', 'h2.bin'] if os.stat(c.p(n)).st_ino == oino]
    check(len(linked) <= 1 and all(c.read(n) == data for n in ['h1.bin', 'h2.bin']),
          f'a thin get links at most one path to an object (Annex/Content/LowLevel.hs linkOrCopy\': '
          f'copies once the link count is above 1): linked {linked}')
    note(f'thin clone: {len(linked)} path(s) share the object inode, link count {os.stat(obj).st_nlink}')
    # What a pigeon linking every path would cause: one in-place save changes both names.
    first = linked[0] if linked else 'h1.bin'
    other = 'h2.bin' if first == 'h1.bin' else 'h1.bin'
    if not linked:
        os.unlink(c.p(first))
        os.link(obj, c.p(first))
    os.unlink(c.p(other))
    os.link(obj, c.p(other))
    with open(c.p(first), 'r+b') as f:
        f.write(b'saved in place')
    check(c.read(other).startswith(b'saved in place') and sha256_file(obj) != key_hash(key),
          'if two paths were linked to the object, a save to one would change the other and the object')
    fsck = c.git('annex', 'fsck', '--key', key, check=False)
    check(fsck.returncode == 0, 'and fsck on a thin laptop does not report it: checkBackend '
          '(Command/Fsck.hs) skips a modified object of an unlocked thin file, '
          f'printing "{fsck.stdout.decode().strip().splitlines()[0][-16:]}"')


def set_xattrs(path):
    """Sets the lab's extended attributes; returns their names, or None if refused."""
    if LINUX:
        try:
            os.setxattr(path, 'user.pigeon', b'lab')
        except OSError as e:
            note(f'setxattr refused: {e}')
            return None
        return ['user.pigeon']
    names = {'user.pigeon': 'lab', 'com.apple.metadata:_kMDItemUserTags': 'Red'}
    for name, value in names.items():
        run(['xattr', '-w', name, value, path], LAB)
    run(['xattr', '-wx', 'com.apple.ResourceFork', '48454c4c4f', path], LAB)
    return list(names) + ['com.apple.ResourceFork']


def list_xattrs(path):
    if LINUX:
        return os.listxattr(path)
    out = run(['xattr', path], LAB).stdout.decode().split()
    return out


def c5_xattrs():
    if not (LINUX or MACOS):
        return skip('xattrs: neither Linux nor macOS')
    r = laptop('r5')
    r.write('tagged.txt', b'tagged\n')
    names = set_xattrs(r.p('tagged.txt'))
    if names is None:
        return skip('xattrs: the file system refuses them')
    r.write('later.txt', b'later\n')
    res, _ = r.capture(['tagged.txt', 'later.txt'])
    key = res[0] and res[0].get('key')
    note(f'xattrs on the captured object: {list_xattrs(r.obj(key))}')
    r.commit()
    pointer = r.staged()[b'later.txt']
    set_xattrs(r.p('later.txt'))
    entry = [f for k, f, p in r.status()[0] if p == b'later.txt']
    note(f'status after setting xattrs on a committed file: {entry}')
    r.capture(['later.txt'])
    check(r.staged()[b'later.txt'] == pointer, 'an xattr-only change captures nothing new')
    check(r.staged()[b'tagged.txt'][0] == '100644' and r.kind('tagged.txt') == 'SHA256E',
          'a file with xattrs is captured as its bytes alone: a tree entry has only a mode and a blob')
    c = clone(r, 'r5c')
    left = [n for n in list_xattrs(c.p('tagged.txt')) if n in names]
    check(c.read('tagged.txt') == b'tagged\n', 'a clone and get give the bytes')
    note(f'xattrs in the clone after a get from a local path: {left}. Such a get copies the object '
         'file (cp keeps xattrs on macOS); the P2P protocol sends only DATA bytes (P2P/Protocol.hs)')


def c6_special_files():
    r = laptop('r6')
    os.mkfifo(r.p('fifo'))
    here = os.getcwd()
    os.chdir(r.path)
    try:
        s = socket.socket(socket.AF_UNIX)
        s.bind('sock')
    finally:
        os.chdir(here)
    listed = r.paths()
    check(b'fifo' not in listed and b'sock' not in listed,
          f'a FIFO and a socket are not listed by status: {listed}')
    check(b'fifo' not in r.paths('--ignored=traditional'), 'nor with --ignored=traditional')
    try:
        res, _ = r.capture(['fifo', 'sock'], timeout=30)
        check(res == [None, None] and b'fifo' not in r.staged() and b'sock' not in r.staged(),
              f'add neither blocks on them nor adds them (Command/Add.hs start: not regular -> stop): {res}')
    except subprocess.TimeoutExpired:
        check(False, 'add of a FIFO blocks')
    s.close()


def c7_nested_repository():
    r = laptop('r7')
    run(['git', 'init', '-q', r.p('proj')], LAB)
    r.write('proj/code.c', b'int main;\n')
    r.write('proj/src/lib.c', b'int lib;\n')
    listed = r.paths()
    check(b'proj/' in listed and not [p for p in listed if p.startswith(b'proj/') and p != b'proj/'],
          f'status shows a nested repository as one entry and never its files: {listed}')
    check(not [p for p in r.paths('--ignored=traditional') if p.startswith(b'proj/') and p != b'proj/'],
          'nor with --ignored=traditional')
    res, err = r.capture(['proj/code.c'])
    staged = r.staged()
    note(f'add of a file inside it: {res[0]}; stderr {err.strip()[:160]!r}; staged {sorted(staged)}')
    check(b'proj/code.c' in staged, 'add does not refuse a file inside a nested repository, '
          'so capture must take its paths from status only')


def c8_unreadable():
    if ROOT:
        return skip('unreadable paths: running as root')
    r = laptop('r8')
    r.write('t.txt', b'tracked\n')
    r.write('td/f.txt', b'tracked in td\n')
    r.capture(['t.txt', 'td/f.txt'])
    r.commit()
    r.write('ud/g.txt', b'untracked\n')
    for rel in ['t.txt', 'td', 'ud']:
        os.chmod(r.p(rel), 0)
    try:
        entries, err = r.status()
        deleted = [p for k, f, p in entries if k in '12' and 'D' in f[0]]
        check(not deleted, f'unreadable paths are never reported deleted: {entries}')
        t = [f[0] for k, f, p in entries if p == b't.txt']
        check(t in ([], ['.M']), 'a tracked file at mode 000 shows as unchanged (its stat still matches '
              f'the index) or modified (git could not read it), never deleted: {t}')
        check(not [p for k, f, p in entries if p.startswith(b'td/') or p.startswith(b'ud/')],
              'a tracked or untracked directory at mode 000 lists nothing')
        check('Permission denied' in err, f'git warns on stderr: {err.strip()!r}')
        res, aerr = r.capture(['t.txt'])
        check(res[0] and res[0].get('success') is False and r.kind('t.txt') == 'SHA256E',
              f'add of an unreadable file fails and leaves the index alone: {aerr.strip()[:120]!r}')
    finally:
        for rel in ['t.txt', 'td', 'ud']:
            os.chmod(r.p(rel), 0o755)


def c9_gitignore():
    r = laptop('r9')
    r.write('ig/.gitignore', b'*.pdf\nbuild/\n')
    r.write('ig/a.pdf', b'pdf\n')
    r.write('ig/b.txt', b'txt\n')
    r.write('ig/build/out.o', b'obj\n')
    r.write('mine.doc', b'doc\n')
    r.write('.excludes', b'*.doc\n')
    r.git('config', 'core.excludesFile', r.p('.excludes'))  # a person's global ignore file
    hidden = [b'ig/a.pdf', b'ig/build/out.o', b'mine.doc']
    listed = r.paths()
    check(not [p for p in hidden if p in listed] and b'ig/b.txt' in listed,
          f'in-tree .gitignore and core.excludesFile hide files from default status: {listed}')
    matching = r.status('--ignored=matching')[0]
    check(('!', [], b'ig/build/') in matching and ('!', [], b'ig/build/out.o') not in matching,
          f'--ignored=matching shows an ignored directory, not its files: {matching}')
    trad = r.status('--ignored=traditional')[0]
    shown = [p for k, f, p in trad if k == '!']
    check(all(p in shown for p in hidden), f'--ignored=traditional with -uall lists every ignored file: {shown}')
    res, _ = r.capture(hidden)
    check(res == [None, None, None], f'add skips ignored files (Command/Add.hs checkIgnored): {res}')
    res, _ = r.capture(hidden, '--no-check-gitignore')
    check(all(x and x.get('key') for x in res) and all(r.kind(p) == 'SHA256E' for p in hidden),
          'add --no-check-gitignore captures them')


ATTR_CASES = {  # file in ga/ -> the in-tree attribute set for it
    'big.bin': 'annex.largefiles=nothing', 'worm.bin': 'annex.backend=WORM', 'nofilter.txt': '-filter',
    'lfs.bin': 'filter=lfs', 'crlf.txt': 'text eol=crlf', 'id.txt': 'ident',
    'enc.txt': 'working-tree-encoding=UTF-16LE', '.hidden': 'annex.backend=WORM',
}


def attributes_variant(name, attributes):
    """Captures ga/* under an in-tree ga/.gitattributes; returns {file: (kind, same bytes in a clone)}."""
    r = laptop(name, attributes)
    r.write('ga/.gitattributes', ''.join(f'{f} {a}\n' for f, a in ATTR_CASES.items()).encode())
    r.write('.pigeon/group.toml', b'format = 1\n')
    files = list(ATTR_CASES) + ['.gitattributes']
    for f in ATTR_CASES:
        r.write(f'ga/{f}', f'line one of {f}\nline $Id$ two\r\n'.encode())
    r.capture([f'ga/{f}' for f in files] + ['.pigeon/group.toml'])
    kinds = {f: r.kind(f'ga/{f}') for f in files}
    r.commit()
    c = clone(r, name + 'c', attributes)
    out = {f: (kinds[f], c.read(f'ga/{f}') == r.read(f'ga/{f}')) for f in files}
    out['/.pigeon/group.toml'] = (r.kind('.pigeon/group.toml'), True)
    out['/.gitattributes'] = (r.kind('.gitattributes'), True)
    return r, out


def c10_gitattributes():
    r, none = attributes_variant('r10a', None)
    note(f'git annex init writes .git/info/attributes: {r.init_attributes!r}')
    note(f'without an override: {none}')
    harmed = sorted(f for f, (k, same) in none.items()
                    if f in ATTR_CASES and (k != 'SHA256E' or not same))
    check(harmed == ['.hidden', 'big.bin', 'enc.txt', 'worm.bin'],
          f'in-tree attributes change capture: a git blob, WORM keys, and a UTF-16 pointer that '
          f'a clone never populates: {harmed}')
    check(none['nofilter.txt'] == ('SHA256E', True) and none['lfs.bin'] == ('SHA256E', True),
          "init's `* filter=annex` in info/attributes already outranks an in-tree -filter or filter=lfs")
    for variant, attrs in [('r10b', OVERRIDE), ('r10m', MINIMAL)]:
        _, got = attributes_variant(variant, attrs)
        bad = {f: v for f, v in got.items() if f in ATTR_CASES and v != ('SHA256E', True)}
        label = 'the full' if attrs is OVERRIDE else 'the minimal'
        check(not bad, f'{label} .git/info/attributes override gives SHA256E pointers and the same '
                       f'bytes in a clone, dotfiles included: {bad or "all"}')
        check(got['/.gitattributes'][0] == 'blob' and got['/.pigeon/group.toml'][0] == 'blob',
              f"with {label} override, the root .gitattributes and .pigeon/ stay git blobs")
        note(f'with {label} override, the nested ga/.gitattributes is {got[".gitattributes"][0]}')


def c11_changed_during_add():
    r = laptop('r11')
    size, outcomes, mism = 30 << 20, {'refused': 0, 'captured': 0}, []
    for i in range(TRIALS):
        name = f'race{i}.bin'
        r.write(name, os.urandom(size))
        stop = threading.Event()

        def writer(path=r.p(name), delay=(i % 3) * 0.05, limit=(0.25 if i % 3 == 2 else None)):
            time.sleep(delay)
            start = time.monotonic()
            with open(path, 'r+b') as f:
                while not stop.is_set() and (limit is None or time.monotonic() - start < limit):
                    f.seek(random.randrange(0, size - 4096) & ~4095)
                    f.write(os.urandom(4096))
                    f.flush()
        t = threading.Thread(target=writer)
        t.start()
        res, err = r.capture([name])
        stop.set()
        t.join()
        key = res[0] and res[0].get('key')
        if not key:
            outcomes['refused'] += 1
            reason = 'changed while' if 'changed while' in err else (
                'failed to link' if 'failed to link' in err else err.strip()[:120])
            outcomes[reason] = outcomes.get(reason, 0) + 1
            continue
        outcomes['captured'] += 1
        obj = r.obj(key)
        fsck = r.git('annex', 'fsck', '--key', key, check=False)
        if sha256_file(obj) != key_hash(key) or fsck.returncode:
            mism.append((i, key, fsck.returncode))
        r.git('annex', 'dropkey', '--force', key, check=False)
        r.git('rm', '-q', '--cached', name, check=False)
        os.unlink(r.p(name))
    note(f'{TRIALS} trials of a 30 MB file rewritten in place during add: {outcomes}')
    check(not mism, 'a file changed during add is refused ("changed while it was being added" '
                    'after hashing, or "failed to link to annex" when the copy saw a change: '
                    'Annex/Ingest.hs ingest\', linkAnnex checksrcunchanged in Annex/Content.hs) '
                    f'or captured with an object that matches its key: mismatches {mism}')


def c12_names():
    r = laptop('r12')
    created = {}
    for label, name in [('invalid UTF-8', b'bad\xff.txt'), ('NFD', unicodedata.normalize('NFD', 'café.txt').encode()),
                        ('Icon\\r', b'Icon\r'), ('colon', b'a:b.txt'), ('upper', b'Case.txt')]:
        try:
            with open(r.p(name), 'xb') as f:
                f.write(label.encode())
            created[label] = name
        except OSError as e:
            created[label] = None
            note(f'{label} {name!r}: {errno.errorcode.get(e.errno, e.errno)} ({e.strerror})')
    if MACOS:
        check(created['invalid UTF-8'] is None, 'macOS refuses a name that is not UTF-8 (EILSEQ expected)')
    else:
        check(created['invalid UTF-8'] is not None, 'Linux accepts a name that is not UTF-8')
    try:
        with open(r.p(b'case.txt'), 'xb') as f:
            f.write(b'lower')
        note('the volume is case-sensitive: Case.txt and case.txt both exist')
        insensitive = False
    except FileExistsError:
        note('the volume is case-insensitive: case.txt is Case.txt')
        insensitive = True
    if MACOS:
        check(insensitive, 'the macOS runner volume is case-insensitive (APFS default)')
    nfd, nfc = created['NFD'], unicodedata.normalize('NFC', 'café.txt').encode()
    for precompose in [False, True]:
        r.git('config', 'core.precomposeunicode', 'true' if precompose else 'false')
        listed = r.paths()
        note(f'core.precomposeunicode={precompose}: status lists {listed}')
        want = nfc if (MACOS and precompose) else nfd
        check(want in listed, f'precomposeunicode={precompose}: status reports the NFD name as '
                              f'{"NFC" if want == nfc else "NFD"} bytes')
    for label in ['invalid UTF-8', 'Icon\\r', 'colon']:
        if created[label]:
            check(created[label] in r.paths(), f'status reports the {label} name as its raw bytes')
    r.git('config', 'core.precomposeunicode', 'false')
    names = [n for n in created.values() if n]
    res, _ = r.capture(names)
    note('capture of these names: ' + ', '.join(f'{n!r}: {"key" if x and x.get("key") else x}'
                                                for n, x in zip(names, res)))


def main():
    if os.path.exists(LAB):
        subprocess.run(['chmod', '-R', 'u+rwx', LAB], check=False)  # annex objects are read-only
        shutil.rmtree(LAB)
    os.makedirs(LAB)
    version = run(['git', 'annex', 'version', '--raw'], LAB).stdout.decode().strip()
    gitv = run(['git', '--version'], LAB).stdout.decode().strip()
    print(f'E20 on {sys.platform}, git-annex {version}, {gitv}, root={ROOT}', flush=True)
    for fn in [c1_regular_files, c3_symlinks, c4_hard_links, c5_xattrs, c6_special_files,
               c7_nested_repository, c8_unreadable, c9_gitignore, c10_gitattributes,
               c11_changed_during_add, c12_names]:
        print(f'-- {fn.__name__}', flush=True)
        try:
            fn()
        except Exception as e:  # a crash is a failure of that check, the others still run
            check(False, f'{fn.__name__} raised {type(e).__name__}: {e}')
    print(f'E20: {COUNTS["PASS"]} passed, {COUNTS["FAIL"]} failed, {COUNTS["SKIP"]} skipped', flush=True)
    return 1 if COUNTS['FAIL'] else 0


if __name__ == '__main__':
    sys.exit(main())
