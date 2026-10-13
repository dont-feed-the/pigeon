#!/usr/bin/env python3
"""E12 and E13 (issue #9; ADRs 0011, 0014, 0043): what peers can do to a
repository over git-annex's generic P2P transport, and what stops them.

A lab P2P network (`git-annex-p2p-lab`) joins a bare custodian C, a laptop L and
a peer P. Asserted:
- E12: remotedaemon fetches a changed remote by its refspecs only; `main`, `HEAD`,
  the index and the working tree never move;
- by default a peer pushes refs into C (even `main`) and L; `pre-receive` refuses;
- E13: remotedaemon serves `REMOVE`, even with `GIT_ANNEX_SHELL_APPENDONLY`;
  append-only `git-annex-shell p2pstdio` refuses it but relays no git;
- pigeon's filter before remotedaemon (random read sizes): reads, stores, fetches,
  change notices pass; `REMOVE` on C and any push are refused; VERSION capped;
  an unknown message closes; payloads holding protocol lines pass intact;
- `.gitattributes` numcopies lowers path drops, not key drops; explicit
  `--numcopies` outweighs a lowered group setting; the hooks git-annex installs.
Usage: ga-e12-e13-p2p-boundary.py (LAB defaults to ~/ga-lab12). Linux, about 90 s.
"""
import os, random, shutil, signal, socket, subprocess, sys, threading, time

LAB = os.path.expanduser(os.environ.get('LAB', '~/ga-lab12'))
NET, BIN = os.path.join(LAB, 'net'), os.path.join(LAB, 'bin')
ENV = dict(os.environ, LABNET=NET, PATH=BIN + ':' + os.environ['PATH'],
           GIT_AUTHOR_NAME='lab', GIT_AUTHOR_EMAIL='lab@x', GIT_COMMITTER_NAME='lab',
           GIT_COMMITTER_EMAIL='lab@x')
FAILS, NOTES = [], []
HELPER = r'''#!/usr/bin/env python3
import os, socket, subprocess, sys, threading, time
net = os.environ['LABNET']
a = sys.argv[1:]
if a == ['address']:
    d = subprocess.run(['git', 'rev-parse', '--absolute-git-dir'], capture_output=True, text=True).stdout.strip()
    print(os.path.basename(d[:-5] if d.endswith('/.git') else d).removesuffix('.git')); sys.exit(0)
if len(a) == 2:                       # remotedaemon listens on a[1]: publish it
    if os.environ.get('LABFILTER'):   # the lab's filter serves the address
        with open(os.path.join(net, a[0] + '.daemon'), 'w') as f: f.write(os.path.abspath(a[1]))
    else:
        link = os.path.join(net, a[0] + '.sock')
        if os.path.lexists(link): os.unlink(link)
        os.symlink(os.path.abspath(a[1]), link)
    while True: time.sleep(3600)
s = socket.socket(socket.AF_UNIX)
try: s.connect(os.path.join(net, a[0] + '.sock'))
except OSError as e: print('cannot connect:', e, file=sys.stderr); sys.exit(1)
def up():
    while (b := os.read(0, 65536)): s.sendall(b)
    s.shutdown(socket.SHUT_WR)
threading.Thread(target=up, daemon=True).start()
while (b := s.recv(65536)): os.write(1, b)
'''


def check(cond, what):
    print(('ok   ' if cond else 'FAIL ') + what, flush=True)
    if not cond:
        FAILS.append(what)


def run(cwd, *cmd, ok=True, inp=None, env=None, timeout=90):
    p = subprocess.run(cmd, cwd=cwd, env=env or ENV, input=inp, capture_output=True,
                       text=True, timeout=timeout)
    if ok and p.returncode:
        sys.exit(f'{cwd}: {" ".join(cmd)} failed: {p.stderr.strip()[-400:]}')
    return p


def git(cwd, *a, **k):
    return run(cwd, 'git', *a, **k).stdout.strip()


def ann(cwd, *a, **k):
    return run(cwd, 'git', 'annex', *a, **k)


def has_key(repo, key):
    return ann(repo, 'contentlocation', key, ok=False).returncode == 0


DAEMONS = []


class Daemon:
    """git annex remotedaemon --foreground; it ends when its stdin closes."""
    def __init__(self, repo, env=None):
        DAEMONS.append(self)
        self.p = subprocess.Popen(['git', 'annex', 'remotedaemon', '--foreground'], cwd=repo,
                                  env=env or ENV, stdin=subprocess.PIPE,
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                  start_new_session=True)
        time.sleep(1.5)

    def stop(self):
        if self.p.poll() is None:
            os.killpg(self.p.pid, signal.SIGKILL)
            self.p.wait()


class Shim:
    """Serves address `addr` with git-annex-shell p2pstdio (append-only): it reads
    the client's AUTH line itself, as pigeond would after authenticating the peer."""
    def __init__(self, addr, repo, appendonly=True):
        self.repo, self.env = repo, dict(ENV)
        if appendonly:
            self.env['GIT_ANNEX_SHELL_APPENDONLY'] = '1'
        path = os.path.join(NET, addr + '.sock')
        if os.path.lexists(path):
            os.unlink(path)
        self.sock = socket.socket(socket.AF_UNIX)
        self.sock.bind(path)
        self.sock.listen()
        threading.Thread(target=self.serve, daemon=True).start()

    def serve(self):
        while True:
            try:
                conn, _ = self.sock.accept()
            except OSError:
                return
            threading.Thread(target=self.session, args=(conn,), daemon=True).start()

    def session(self, conn):
        f = conn.makefile('rb')
        auth = f.readline().split()          # AUTH <client uuid> <token>
        if len(auth) < 2 or auth[0] != b'AUTH':
            conn.close()
            return
        p = subprocess.Popen(['git-annex-shell', 'p2pstdio', self.repo, auth[1].decode()],
                             env=self.env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                             stderr=open(os.path.join(LAB, 'shim.log'), 'ab'))

        def up():
            while (b := f.read1(65536)):
                p.stdin.write(b)
                p.stdin.flush()
            p.stdin.close()
        threading.Thread(target=up, daemon=True).start()
        while (b := p.stdout.read1(65536)):
            conn.sendall(b)
        conn.shutdown(socket.SHUT_RDWR)
        conn.close()

    def stop(self):
        self.sock.close()


class Closed(Exception):
    pass


class Frames:
    """One direction of git-annex's line-based P2P serialization, frame by frame:
    a line, or `DATA n` and n raw bytes. Reads in random sizes to stress framing."""
    def __init__(self, sock):
        self.sock, self.buf = sock, b''

    def fill(self):
        b = self.sock.recv(random.randint(1, 4096))
        if not b:
            raise Closed('end of stream')
        self.buf += b

    def line(self):
        while b'\n' not in self.buf:
            if len(self.buf) > 4096:
                raise Closed('line too long')
            self.fill()
        line, _, self.buf = self.buf.partition(b'\n')
        return line

    def payload(self, n):
        while n:
            if not self.buf:
                self.fill()
            chunk, self.buf = self.buf[:n], self.buf[n:]
            n -= len(chunk)
            yield chunk


def data_length(line):
    digits = line[5:]
    if not digits.isdigit() or len(digits) > 15:
        raise Closed(f'bad DATA line {line[:40]!r}')
    return int(digits)


class Filter:
    """The reference filter of spec 09 §9.4 in front of remotedaemon's socket:
    an allow-list of client messages (protocol versions 0 to MAX_VERSION),
    VERSION capped, REMOVE answered FAILURE on custodians, CONNECT only to
    git-upload-pack (pull only, on every device), anything else closes."""
    MAX_VERSION = 4
    ALLOWED = {'AUTH', 'VERSION', 'BYPASS', 'CHECKPRESENT', 'LOCKCONTENT', 'UNLOCKCONTENT',
               'GET', 'PUT', 'VALID', 'INVALID', 'DATA-PRESENT', 'SUCCESS', 'FAILURE',
               'GETTIMESTAMP', 'NOTIFYCHANGE', 'CONNECT', 'CONNECTDONE', 'ERROR'}

    def __init__(self, addr, custodian):
        self.daemon_file = os.path.join(NET, addr + '.daemon')
        self.custodian, self.refused = custodian, []
        path = os.path.join(NET, addr + '.sock')
        if os.path.lexists(path):
            os.unlink(path)
        self.sock = socket.socket(socket.AF_UNIX)
        self.sock.bind(path)
        self.sock.listen()
        threading.Thread(target=self.serve, daemon=True).start()

    def serve(self):
        while True:
            try:
                client, _ = self.sock.accept()
            except OSError:
                return
            threading.Thread(target=self.session, args=(client,), daemon=True).start()

    def session(self, client):
        server = socket.socket(socket.AF_UNIX)
        server.connect(open(self.daemon_file).read())
        lock = threading.Lock()

        def down():                       # server to client: framing only
            fr = Frames(server)
            while True:
                line = fr.line()
                n = data_length(line) if line.startswith(b'DATA ') else 0
                with lock:
                    client.sendall(line + b'\n')
                    for chunk in fr.payload(n):
                        client.sendall(chunk)

        def up():                         # client to server: the allow-list
            fr = Frames(client)
            while True:
                line = fr.line()
                word = line.split(b' ', 1)[0].decode('ascii', 'replace')
                if word == 'DATA':
                    n = data_length(line)     # checked before anything is forwarded
                    server.sendall(line + b'\n')
                    for chunk in fr.payload(n):
                        server.sendall(chunk)
                    continue
                if word == 'VERSION':
                    v = line[8:]
                    if not v.isdigit():
                        raise Closed('bad VERSION')
                    line = b'VERSION %d' % min(int(v), self.MAX_VERSION)
                elif word in ('REMOVE', 'REMOVE-BEFORE') and self.custodian:
                    self.refused.append(line)
                    with lock:
                        client.sendall(b'FAILURE\n')
                    continue
                elif word == 'CONNECT' and line != b'CONNECT git-upload-pack':
                    self.refused.append(line)
                    with lock:
                        client.sendall(b'ERROR pigeon: this device accepts no push\n')
                    raise Closed('push refused')
                elif word not in self.ALLOWED and word not in ('REMOVE', 'REMOVE-BEFORE'):
                    self.refused.append(line)
                    raise Closed(f'unknown message {word!r}')
                server.sendall(line + b'\n')

        def guard(fn):
            try:
                fn()
            except (Closed, OSError):
                pass
            for s in (client, server):
                try:
                    s.shutdown(socket.SHUT_RDWR)
                except OSError:
                    pass

        t = threading.Thread(target=guard, args=(down,), daemon=True)
        t.start()
        guard(up)
        t.join(5)
        client.close()
        server.close()

    def stop(self):
        self.sock.close()


def filter_against_fake_server():
    """The filter alone, against a server that records what reaches it."""
    path = os.path.join(NET, 'fake-daemon.sock')
    srv = socket.socket(socket.AF_UNIX)
    srv.bind(path)
    srv.listen()
    with open(os.path.join(NET, 'X.daemon'), 'w') as f:
        f.write(path)
    flt, got = Filter('X', custodian=True), []

    def record():
        for _ in range(2):
            c, _ = srv.accept()
            c.settimeout(3)
            data = b''
            try:
                while (b := c.recv(65536)):
                    data += b
            except OSError:
                pass
            got.append(data)
    t = threading.Thread(target=record, daemon=True)
    t.start()
    payload = b'REMOVE k\nCONNECT git-receive-pack\nDATA 99\n'
    sent = b'AUTH u t\nVERSION 9\nDATA %d\n%sCHECKPRESENT k\n' % (len(payload), payload)
    c = socket.socket(socket.AF_UNIX)
    c.connect(os.path.join(NET, 'X.sock'))
    c.settimeout(5)
    for i in range(0, len(sent), 7):      # small writes cut lines and payload
        c.sendall(sent[i:i + 7])
        time.sleep(0.01)
    c.sendall(b'REMOVE k\n')
    answer = c.recv(4096)
    c.sendall(b'DATA 12x\n')
    try:
        closed = c.recv(4096) == b''
    except OSError:
        closed = True
    c2 = socket.socket(socket.AF_UNIX)
    c2.connect(os.path.join(NET, 'X.sock'))
    c2.settimeout(5)
    c2.sendall(b'A' * 5000)
    try:
        closed2 = c2.recv(4096) == b''
    except OSError:
        closed2 = True
    t.join(10)
    flt.stop()
    expect = sent.replace(b'VERSION 9', b'VERSION 4')
    check(len(got) == 2 and got[0] == expect,
          'filter unit: VERSION 9 becomes VERSION 4; a payload holding REMOVE, CONNECT and DATA lines passes as data')
    check(answer == b'FAILURE\n' and b'REMOVE k\n' not in got[0][len(expect):],
          'filter unit: REMOVE is answered FAILURE and never reaches the server')
    check(closed and closed2 and got[1] == b'',
          'filter unit: a malformed DATA line or a line over 4096 bytes closes the session')


def address(repo):
    out = ann(repo, 'p2p', '--gen-addresses').stdout
    return [l for l in out.splitlines() if l.startswith('p2p-annex::')][0]


def link(repo, name, addr):
    ann(repo, 'p2p', '--link', '--name', name, inp=addr + '\n')


def wait_for(pred, seconds=20):
    end = time.time() + seconds
    while time.time() < end:
        if pred():
            return True
        time.sleep(0.3)
    return False


def commit_in(repo, msg):
    """A commit on main made with plumbing, as pigeond does (no hooks run)."""
    tree = git(repo, 'rev-parse', 'main^{tree}')
    c = git(repo, 'commit-tree', tree, '-p', 'main', '-m', msg)
    git(repo, 'update-ref', 'refs/heads/main', c)
    return c


def hooks(gitdir):
    d = os.path.join(gitdir, 'hooks')
    return sorted(h for h in os.listdir(d) if not h.endswith('.sample'))


def refuse_pushes(gitdir):
    """pigeon's pre-receive hook: no device accepts a push (pull only, 03 §3.6)."""
    path = os.path.join(gitdir, 'hooks', 'pre-receive')
    with open(path, 'w') as f:
        f.write('#!/bin/sh\necho "pigeon: this repository accepts no push" >&2\nexit 1\n')
    os.chmod(path, 0o755)


def pushed(repo, remote, spec):
    """git push over P2P; the client waits forever after an ERROR to CONNECT."""
    try:
        return run(repo, 'git', 'push', remote, spec, ok=False, timeout=15).returncode == 0
    except subprocess.TimeoutExpired:
        return False


def ref(repo, name):
    return run(repo, 'git', 'rev-parse', '-q', '--verify', name, ok=False).stdout.strip()


def worktree_state(repo):
    with open(os.path.join(repo, '.git', 'index'), 'rb') as f:
        index = f.read()
    return (ref(repo, 'main'), git(repo, 'symbolic-ref', 'HEAD'), index,
            git(repo, 'status', '--porcelain'), sorted(os.listdir(repo)))


def setup():
    if os.path.exists(LAB):
        subprocess.run(['chmod', '-R', 'u+w', LAB])
        shutil.rmtree(LAB)
    os.makedirs(NET)
    os.makedirs(BIN)
    with open(os.path.join(BIN, 'git-annex-p2p-lab'), 'w') as f:
        f.write(HELPER)
    os.chmod(os.path.join(BIN, 'git-annex-p2p-lab'), 0o755)
    L, C, P = (os.path.join(LAB, n) for n in ('L', 'C.git', 'P'))
    git(LAB, 'init', '-q', '-b', 'main', L)
    ann(L, 'init', 'L')
    with open(os.path.join(L, 'f1'), 'wb') as f:
        f.write(os.urandom(65536))
    ann(L, 'add', 'f1')
    git(L, 'commit', '-q', '-m', 'one')
    git(LAB, 'clone', '-q', '--bare', L, C)
    git(LAB, 'clone', '-q', L, P)
    for r, name in ((C, 'C'), (P, 'P')):
        git(r, 'remote', 'remove', 'origin')
        ann(r, 'init', name)
    for r in (C, L):
        ann(r, 'p2p', '--enable', 'lab')
    return L, C, P


def main():
    L, C, P = setup()
    hl, hc = hooks(os.path.join(L, '.git')), hooks(C)
    NOTES.append(f'hooks installed by git-annex: non-bare {hl}; bare {hc}')
    check(hl == ['post-checkout', 'post-merge', 'post-receive', 'pre-commit'],
          f'git-annex installs exactly these hooks in a laptop repository: {hl}')
    check(set(hc) <= {'post-receive'}, f'and these in a bare custodian repository: {hc}')

    # Phase A: remotedaemon serves C and L.
    dC, dL = Daemon(C), Daemon(L)
    link(L, 'c', address(C))
    link(P, 'c', address(C))
    link(P, 'l', address(L))
    k1 = git(L, 'annex', 'lookupkey', 'f1')
    ann(L, 'copy', '--to', 'c', 'f1')
    check(has_key(C, k1), 'a laptop stores content on C over P2P')

    # E12: a change notice makes L's remotedaemon fetch, by L's refspec only.
    git(L, 'config', 'remote.c.fetch', '+refs/heads/main:refs/pigeon/peers/C/main')
    dL.stop()
    dL = Daemon(L)
    before = worktree_state(L)
    new = commit_in(C, 'changed on C')
    check(wait_for(lambda: ref(L, 'refs/pigeon/peers/C/main') == new),
          "E12: L's remotedaemon fetched C's change into L's configured refspec")
    check(worktree_state(L) == before,
          'E12: and main, HEAD, the index and the working tree of L did not move')
    check(ref(L, 'refs/remotes/c/main') == '', 'E12: no other ref was written')

    # Pushes into peers over P2P.
    pc = commit_in(P, 'unvalidated, by P')
    r = run(P, 'git', 'push', 'c', '+main:refs/heads/main', ok=False)
    check(r.returncode == 0 and ref(C, 'main') == pc,
          'by default a peer pushes an unvalidated main into custodian C over P2P')
    git(C, 'update-ref', 'refs/heads/main', new)
    refuse_pushes(C)
    r = run(P, 'git', 'push', 'c', '+main:refs/heads/main', ok=False)
    check(r.returncode != 0 and ref(C, 'main') == new, "pigeon's pre-receive hook refuses it")
    r = run(P, 'git', 'push', 'l', 'main:refs/heads/main', ok=False)
    check(r.returncode != 0 and ref(L, 'main') == before[0],
          "git refuses a push to L's checked-out main by default (receive.denyCurrentBranch)")
    r = run(P, 'git', 'push', 'l', 'main:refs/pigeon/peers/X/main', ok=False)
    check(r.returncode == 0 and ref(L, 'refs/pigeon/peers/X/main') == pc,
          "but accepts a push to any other ref of L, such as refs/pigeon/peers/X/main")
    refuse_pushes(os.path.join(L, '.git'))
    r = run(P, 'git', 'push', 'l', 'main:refs/pigeon/peers/Y/main', ok=False)
    check(r.returncode != 0 and ref(L, 'refs/pigeon/peers/Y/main') == '',
          "pigeon's pre-receive hook refuses that too")

    # E13: remote REMOVE.
    r = ann(P, 'drop', '--from', 'c', '--force', '--key', k1, ok=False)
    check(not has_key(C, k1), 'E13: remotedaemon serves REMOVE: P dropped C\'s copy')
    ann(L, 'copy', '--to', 'c', '--key', k1)
    dC.stop()
    dC = Daemon(C, env=dict(ENV, GIT_ANNEX_SHELL_APPENDONLY='1'))
    r = ann(P, 'drop', '--from', 'c', '--force', '--key', k1, ok=False)
    check(not has_key(C, k1), 'E13: remotedaemon ignores GIT_ANNEX_SHELL_APPENDONLY: REMOVE served')
    ann(L, 'copy', '--to', 'c', '--key', k1)
    dC.stop()

    # Phase B: git-annex-shell p2pstdio, append-only, behind a one-line shim.
    shim = Shim('C', C)
    r = ann(P, 'drop', '--from', 'c', '--force', '--key', k1, ok=False)
    check(r.returncode != 0 and has_key(C, k1),
          'E13: git-annex-shell p2pstdio in append-only mode refuses REMOVE')
    try:
        fetched = run(P, 'git', 'fetch', 'c', ok=False, timeout=15).returncode == 0
    except subprocess.TimeoutExpired:
        fetched = False                   # the client waits forever after the ERROR
    with open(os.path.join(LAB, 'shim.log')) as f:
        refused = 'relaying to git not supported' in f.read()
    check(not fetched and refused,
          'E13: but it relays no git (CONNECT), so it cannot serve a P2P remote alone')
    shim.stop()

    # Phase C: remotedaemon behind pigeon's filter, on C (custodian) and L (laptop).
    os.unlink(os.path.join(C, 'hooks', 'pre-receive'))
    os.unlink(os.path.join(L, '.git', 'hooks', 'pre-receive'))
    dC = Daemon(C, env=dict(ENV, LABFILTER='1'))
    dL.stop()
    dL = Daemon(L, env=dict(ENV, LABFILTER='1'))
    check(wait_for(lambda: all(os.path.exists(os.path.join(NET, a + '.daemon')) for a in 'CL')),
          'remotedaemon gives its listening socket to the network helper')
    fc, fl = Filter('C', custodian=True), Filter('L', custodian=False)
    payload = (os.urandom(3000) + b'\nREMOVE ' + k1.encode() + b'\nDATA 5\nCONNECT git-receive-pack\n'
               + os.urandom(70000))
    with open(os.path.join(L, 'f2'), 'wb') as f:
        f.write(payload)
    ann(L, 'add', 'f2')
    git(L, 'commit', '-q', '-m', 'two')
    k2 = git(L, 'annex', 'lookupkey', 'f2')
    ann(L, 'copy', '--to', 'c', 'f2')
    check(has_key(C, k2), 'filter: a laptop stores on C a file whose bytes hold protocol lines')
    git(P, 'fetch', 'c')
    check(ref(P, 'refs/remotes/c/main') == new, 'filter: git fetch (CONNECT git-upload-pack) passes')
    git(P, 'fetch', 'l')                  # L's location log knows that C has k2
    ann(P, 'merge')
    ann(P, 'get', '--from', 'c', '--key', k2)
    with open(os.path.join(P, git(P, 'annex', 'contentlocation', k2)), 'rb') as f:
        got = f.read()
    check(got == payload, 'filter: a peer reads it back intact (DATA framing, random read sizes)')
    new2 = commit_in(C, 'changed on C again')
    check(wait_for(lambda: ref(L, 'refs/pigeon/peers/C/main') == new2),
          "filter: change notices still reach L's remotedaemon")
    r = ann(P, 'drop', '--from', 'c', '--force', '--key', k1, ok=False)
    check(r.returncode != 0 and has_key(C, k1) and any(l.startswith(b'REMOVE') for l in fc.refused),
          'E13 filter: REMOVE is answered FAILURE on C; C keeps its copy')
    check(not pushed(P, 'c', '+main:refs/heads/main') and ref(C, 'main') == new2
          and any(l.startswith(b'CONNECT git-receive-pack') for l in fc.refused),
          'filter: a push into C is refused, without any hook')
    check(not pushed(P, 'l', 'main:refs/pigeon/peers/Z/main')
          and ref(L, 'refs/pigeon/peers/Z/main') == '', 'filter: a push into laptop L is refused too')
    filter_against_fake_server()
    NOTES.append(f'filter refused: {sorted(set(l.split()[0].decode() for l in fc.refused + fl.refused))}')

    # Phase D: drop settings. Any device can lower numcopies in the git-annex branch,
    # and a commit can lower it per file in .gitattributes; explicit flags win.
    ann(L, 'numcopies', '1')
    r = ann(L, 'drop', '--key', k1, '--numcopies', '2', '--mincopies', '2', ok=False)
    check(r.returncode != 0 and has_key(L, k1),
          "explicit --numcopies 2 --mincopies 2 outweighs numcopies 1 in the branch")
    r = ann(L, 'drop', '--key', k1, ok=False)
    check(r.returncode == 0 and not has_key(L, k1), 'without them the lowered setting drops')
    ann(L, 'numcopies', '2')
    ann(P, 'drop', '--key', k2)           # L and C hold k2 now
    with open(os.path.join(L, '.gitattributes'), 'w') as f:
        f.write('* annex.numcopies=1\n')
    git(L, 'add', '.gitattributes')
    git(L, 'commit', '-q', '-m', 'override')
    r = ann(L, 'drop', '--key', k2, '--numcopies', '2', '--mincopies', '2', ok=False)
    check(r.returncode != 0 and has_key(L, k2), 'explicit flags outweigh .gitattributes too')
    r = ann(L, 'drop', '--key', k2, ok=False)
    check(r.returncode == 0 and not has_key(L, k2),
          "drop --key obeys annex.numcopies=1 of the key's file in .gitattributes")
    fc.stop()
    fl.stop()
    dC.stop()
    dL.stop()
    for n in NOTES:
        print('note', n)
    print(f'E12/E13: {len(FAILS)} failure(s)')
    sys.exit(1 if FAILS else 0)


if __name__ == '__main__':
    try:
        main()
    finally:
        for d in DAEMONS:
            d.stop()
