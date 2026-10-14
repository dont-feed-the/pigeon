#!/usr/bin/env python3
"""E23 (issue #10; spec 09 §9.4): git-annex's verified drop over its P2P protocol.

Custodians X, Y, Z are bare repositories served by remotedaemon on a lab P2P
network, as in E12; each knows the others as `p2p-annex::` remotes and from its
location log. A local drop locks its own copy for removal, then locks or checks
the copies it counts (`verifyEnoughCopiesToDrop`, Annex/NumCopies.hs). Asserted:
- over P2P a copy that its owner has locked for removal still answers
  CHECKPRESENT (`inAnnex`, P2P/Annex.hs) and a drop counts it; over a local path
  (E1's setup) `inAnnexSafe` refuses it;
- two drops that hold their removal locks before checking each other both
  succeed: numcopies 1 loses the key; numcopies = mincopies = 2 on three
  custodians leaves one copy. pigeon's git-annex (`-pigeon` in its version,
  third_party/git-annex patch 0001) refuses the counted copy and both races;
- a filter answering CHECKPRESENT by LOCKCONTENT, UNLOCKCONTENT (spec 09 §9.4)
  keeps every copy in the same schedule;
- a stale location log (E5) and `dead` never let a drop count a missing copy;
  `trust` does.
Usage: ga-e23-p2p-drop.py (LAB defaults to ~/ga-lab23). Linux and macOS, ~1 min.
"""
import glob, os, shutil, signal, socket, subprocess, sys, threading, time

LAB = os.path.expanduser(os.environ.get('LAB', '~/ga-lab23'))
NET, BIN = os.path.join(LAB, 'net'), os.path.join(LAB, 'bin')
ENV = dict(os.environ, LABNET=NET, PATH=BIN + ':' + os.environ['PATH'],
           GIT_CONFIG_GLOBAL='/dev/null', GIT_CONFIG_NOSYSTEM='1',
           GIT_AUTHOR_NAME='lab', GIT_AUTHOR_EMAIL='lab@x', GIT_COMMITTER_NAME='lab',
           GIT_COMMITTER_EMAIL='lab@x')
FAILS = []
PATCHED = '-pigeon' in subprocess.run(['git', 'annex', 'version', '--raw'], env=ENV,
                                      capture_output=True, text=True).stdout
# The lab network: `git-annex-p2p-lab`. As a client it can hold a drop at two
# points, so that schedules are deterministic: LABCONN=n waits after connecting
# until n git-annex processes have connected (0: until the file `go` exists);
# LABCHK=n holds the answer to CHECKPRESENT until n processes have one.
HELPER = r'''#!/usr/bin/env python3
import os, socket, subprocess, sys, threading, time
net = os.environ['LABNET']
a = sys.argv[1:]
if a == ['address']:
    d = subprocess.run(['git', 'rev-parse', '--absolute-git-dir'], capture_output=True,
                       text=True).stdout.strip()
    print(os.path.basename(d[:-5] if d.endswith('/.git') else d).removesuffix('.git')); sys.exit(0)
if len(a) == 2:                       # remotedaemon listens on a[1]: publish it
    if os.environ.get('LABFILTER'):   # the lab's filter serves the address
        with open(os.path.join(net, a[0] + '.daemon'), 'w') as f: f.write(os.path.abspath(a[1]))
    else:
        link = os.path.join(net, a[0] + '.sock')
        if os.path.lexists(link): os.unlink(link)
        os.symlink(os.path.abspath(a[1]), link)
    while True: time.sleep(3600)
def barrier(name, n):
    open(os.path.join(net, '%s.%d' % (name, os.getppid())), 'w').close()
    end = time.time() + 30
    while time.time() < end and not os.path.exists(os.path.join(net, 'go')):
        if n and len([f for f in os.listdir(net) if f.startswith(name + '.')]) >= n: return
        time.sleep(0.02)
if os.environ.get('LABCONN') is not None: barrier('conn', int(os.environ['LABCONN']))
chk = os.environ.get('LABCHK')
s = socket.socket(socket.AF_UNIX)
try: s.connect(os.path.join(net, a[0] + '.sock'))
except OSError as e: print('cannot connect:', e, file=sys.stderr); sys.exit(1)
asked = threading.Event()
def up():
    while (b := os.read(0, 65536)):
        if chk and b'CHECKPRESENT ' in b: asked.set()
        s.sendall(b)
    s.shutdown(socket.SHUT_WR)
threading.Thread(target=up, daemon=True).start()
while (b := s.recv(65536)):
    if asked.is_set():
        asked.clear(); barrier('chk', int(chk))
    os.write(1, b)
'''


def check(cond, what):
    print(('ok   ' if cond else 'FAIL ') + what, flush=True)
    if not cond:
        FAILS.append(what)


def run(cwd, *cmd, ok=True, env=None, inp=None, timeout=90):
    p = subprocess.run(cmd, cwd=cwd, env=env or ENV, input=inp, capture_output=True,
                       text=True, timeout=timeout)
    if ok and p.returncode:
        sys.exit(f'{cwd}: {" ".join(cmd)} failed: {p.stderr.strip()[-400:]}')
    return p


def git(cwd, *a, **k):
    return run(cwd, 'git', *a, **k).stdout.strip()


def ann(cwd, *a, **k):
    return run(cwd, 'git', 'annex', *a, **k)


DAEMONS = []


class Daemon:
    """git annex remotedaemon --foreground, in its own process group."""
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


class Lines:
    """One direction of the P2P stream: a line, or `DATA n` and n raw bytes."""
    def __init__(self, sock):
        self.sock, self.buf = sock, b''

    def line(self):
        while b'\n' not in self.buf:
            b = self.sock.recv(65536)
            if not b:
                raise EOFError
            self.buf += b
        line, _, self.buf = self.buf.partition(b'\n')
        return line

    def payload(self, n):
        while n:
            if not self.buf:
                self.buf = self.sock.recv(65536)
                if not self.buf:
                    raise EOFError
            chunk, self.buf = self.buf[:n], self.buf[n:]
            n -= len(chunk)
            yield chunk


class ProbeFilter:
    """The change that 09 §9.4 makes to pigeon's filter, alone: CHECKPRESENT k
    is answered SUCCESS only if LOCKCONTENT k succeeds (then UNLOCKCONTENT), so a
    copy locked for removal is never reported present. Everything else passes."""
    def __init__(self, addr):
        self.daemon_file = os.path.join(NET, addr + '.daemon')
        self.probes = []
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
        lock, probing, answer = threading.Lock(), threading.Event(), []
        got = threading.Event()

        def down():
            fr = Lines(server)
            while True:
                line = fr.line()
                if probing.is_set():          # the answer to our LOCKCONTENT
                    probing.clear()
                    answer.append(line)
                    got.set()
                    continue
                n = int(line[5:]) if line.startswith(b'DATA ') else 0
                with lock:
                    client.sendall(line + b'\n')
                    for chunk in fr.payload(n):
                        client.sendall(chunk)

        def up():
            fr = Lines(client)
            while True:
                line = fr.line()
                if line.startswith(b'DATA '):
                    server.sendall(line + b'\n')
                    for chunk in fr.payload(int(line[5:])):
                        server.sendall(chunk)
                    continue
                if line.startswith(b'CHECKPRESENT '):
                    answer.clear()
                    got.clear()
                    probing.set()
                    server.sendall(b'LOCKCONTENT ' + line[13:] + b'\n')
                    got.wait(30)
                    ok = answer == [b'SUCCESS']
                    if ok:
                        server.sendall(b'UNLOCKCONTENT\n')
                    self.probes.append(ok)
                    with lock:
                        client.sendall(b'SUCCESS\n' if ok else b'FAILURE\n')
                    continue
                server.sendall(line + b'\n')

        def guard(fn):
            try:
                fn()
            except (EOFError, OSError):
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


def has_key(repo, key):
    return ann(repo, 'contentlocation', key, ok=False).returncode == 0


def address(repo):
    out = ann(repo, 'p2p', '--gen-addresses').stdout
    return [l for l in out.splitlines() if l.startswith('p2p-annex::')][0]


def wait_for(pred, seconds=20):
    end = time.time() + seconds
    while time.time() < end:
        if pred():
            return True
        time.sleep(0.1)
    return False


def reset_barriers():
    for f in glob.glob(os.path.join(NET, 'conn.*')) + glob.glob(os.path.join(NET, 'chk.*')):
        os.unlink(f)
    if os.path.exists(os.path.join(NET, 'go')):
        os.unlink(os.path.join(NET, 'go'))


def held():
    return len(glob.glob(os.path.join(NET, 'conn.*')))


def drop_cmd(numcopies, ignore=()):
    cfg = [a for r in ignore for a in ('-c', f'remote.{r}.annex-ignore=true')]
    return ['git', *cfg, 'annex', 'drop', '--key', KEY,
            '--numcopies', str(numcopies), '--mincopies', str(numcopies)]


def drop(repo, numcopies, ignore=()):
    return run(repo, *drop_cmd(numcopies, ignore), ok=False).returncode == 0


def start_drop(repo, numcopies, ignore=(), **holds):
    env = dict(ENV, **{k: str(v) for k, v in holds.items()})
    return subprocess.Popen(drop_cmd(numcopies, ignore), cwd=repo, env=env,
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def restore(*repos):
    for r in repos:
        if not has_key(r, KEY):
            run(r, 'git', '-c', 'remote.w.annex-ignore=false', 'annex', 'get', '--key', KEY,
                '--from', 'w')


def race(repos, numcopies, ignore=()):
    """Drops on every repo of `repos`, each holding its removal lock before it
    checks the others, and each answered CHECKPRESENT before any removes."""
    reset_barriers()
    n = len(repos)
    ps = [start_drop(r, numcopies, ignore, LABCONN=n, LABCHK=n) for r in repos]
    for p in ps:
        p.wait(timeout=90)
    return [p.returncode == 0 for p in ps]


def setup():
    if os.path.exists(LAB):
        subprocess.run(['chmod', '-R', 'u+w', LAB])
        shutil.rmtree(LAB)
    os.makedirs(NET)
    os.makedirs(BIN)
    with open(os.path.join(BIN, 'git-annex-p2p-lab'), 'w') as f:
        f.write(HELPER)
    os.chmod(os.path.join(BIN, 'git-annex-p2p-lab'), 0o755)
    W = os.path.join(LAB, 'W')
    git(LAB, 'init', '-q', '-b', 'main', W)
    ann(W, 'init', 'W')
    with open(os.path.join(W, 'f'), 'wb') as f:
        f.write(os.urandom(65536))
    ann(W, 'add', 'f')
    git(W, 'commit', '-q', '-m', 'one')
    key = git(W, 'annex', 'lookupkey', 'f')
    repos = {}
    for n in 'XYZ':
        r = os.path.join(LAB, n + '.git')
        git(LAB, 'clone', '-q', '--bare', W, r)
        ann(r, 'init', n)
        git(r, 'remote', 'rename', 'origin', 'w')
        git(r, 'config', 'remote.w.annex-ignore', 'true')   # the source, never a proof
        ann(r, 'p2p', '--enable', 'lab')
        repos[n] = r
    return W, repos, key


def link_all(repos):
    for n, r in repos.items():
        for o, ro in repos.items():
            if o != n:
                ann(r, 'p2p', '--link', '--name', o.lower(), inp=address(ro) + '\n')
    uuids = {n: git(r, 'config', 'annex.uuid') for n, r in repos.items()}
    for n, r in repos.items():                  # every location log knows every copy
        for o in repos:
            ann(r, 'setpresentkey', KEY, uuids[o], '1')
    return uuids


def main():
    global KEY
    W, R, KEY = setup()
    X, Y, Z = R['X'], R['Y'], R['Z']
    daemons = [Daemon(r) for r in (X, Y, Z)]
    uuids = link_all(R)
    restore(X, Y, Z)
    check(all(has_key(r, KEY) for r in (X, Y, Z)), 'X, Y and Z hold the key, linked over P2P')

    # 1. Y's own drop holds its removal lock (held after connecting); X drops.
    reset_barriers()
    py = start_drop(Y, 1, ignore=('z',), LABCONN=0)
    wait_for(lambda: held() == 1)
    ok = drop(X, 1, ignore=('z',))
    open(os.path.join(NET, 'go'), 'w').close()
    py.wait(timeout=90)
    if PATCHED:
        check(not ok and has_key(X, KEY),
              "patched: over P2P, X's drop refuses Y's copy while Y holds it locked for removal")
        check(py.returncode == 0 and not has_key(Y, KEY) and has_key(X, KEY),
              "patched: Y's drop, released, counts X's copy and succeeds")
    else:
        check(ok and not has_key(X, KEY) and has_key(Y, KEY),
              "over P2P, X's drop counts Y's copy although Y holds it locked for removal")
        check(py.returncode != 0 and has_key(Y, KEY),
              "Y's drop, released, finds X gone and refuses")
    restore(X, Y)

    # 2. The same with Y as a local-path remote of X: inAnnexSafe sees the lock.
    git(X, 'remote', 'add', 'ylocal', Y)
    ann(X, 'info', 'ylocal', ok=False)              # learns its UUID
    git(X, 'config', 'remote.ylocal.annex-ignore', 'true')
    reset_barriers()
    py = start_drop(Y, 1, ignore=('z',), LABCONN=0)
    wait_for(lambda: held() == 1)
    ok = run(X, 'git', '-c', 'remote.ylocal.annex-ignore=false', *drop_cmd(1, ('y', 'z'))[1:],
             ok=False).returncode == 0
    check(not ok and has_key(X, KEY),
          "over a local path (E1's setup), X's drop refuses Y's copy while Y is dropping")
    open(os.path.join(NET, 'go'), 'w').close()
    py.wait(timeout=90)
    restore(X, Y)

    # 3. Two drops, numcopies 1: each locks its copy, then counts the other's.
    res = race((X, Y), 1, ignore=('z',))
    left = [n for n, r in (('X', X), ('Y', Y)) if has_key(r, KEY)]
    if PATCHED:
        check(left == ['X', 'Y'],
              f'patched: two concurrent drops with numcopies 1 keep both: {left}; drops {res}')
    else:
        check(res == [True, True] and left == [],
              f'over P2P two concurrent drops with numcopies 1 both succeed: copies left {left}')
    restore(X, Y)

    # 4. K = 2 on three custodians, as pigeon drops (--numcopies 2 --mincopies 2).
    res = race((X, Y), 2)
    left = [n for n, r in R.items() if has_key(r, KEY)]
    if PATCHED:
        check(left == ['X', 'Y', 'Z'],
              f'patched: with numcopies = mincopies = 2 the same race keeps every copy: {left}')
    else:
        check(res == [True, True] and left == ['Z'],
              f'with numcopies = mincopies = 2, two concurrent drops leave one copy: {left}')
    restore(X, Y)

    # 5. The same schedules behind the probing filter of spec 09 §9.4.
    for d in daemons:
        d.stop()
    for a in 'XYZ':
        if os.path.lexists(os.path.join(NET, a + '.sock')):
            os.unlink(os.path.join(NET, a + '.sock'))
    daemons = [Daemon(r, env=dict(ENV, LABFILTER='1')) for r in (X, Y, Z)]
    wait_for(lambda: all(os.path.exists(os.path.join(NET, a + '.daemon')) for a in 'XYZ'))
    filters = [ProbeFilter(a) for a in 'XYZ']
    res = race((X, Y), 2)
    left = [n for n, r in R.items() if has_key(r, KEY)]
    probes = [p for f in filters for p in f.probes]
    check(left == ['X', 'Y', 'Z'] and False in probes,
          f'filter: the same race keeps every copy ({left}; drops {res}; probes {probes})')
    res = race((X, Y), 1, ignore=('z',))
    left = [n for n, r in (('X', X), ('Y', Y)) if has_key(r, KEY)]
    check(left == ['X', 'Y'], f'filter: and the numcopies 1 race keeps both: {left}')
    restore(X, Y)
    ok = drop(X, 2)
    check(ok and not has_key(X, KEY), 'filter: a drop without a concurrent one still succeeds')
    restore(X)

    # 6. A stale location log (E5): X's log says Y has the key; Y dropped it.
    run(Y, 'git', 'annex', 'drop', '--force', '--key', KEY)
    ok = drop(X, 1, ignore=('z',))
    check(not ok and has_key(X, KEY), "stale log: X's drop does not count Y's missing copy")
    # 7. `trust`: X counts Y from its log without asking.
    ann(X, 'trust', '--force', 'y')
    ok = drop(X, 1, ignore=('z',))
    check(ok and not has_key(X, KEY) and not has_key(Y, KEY),
          'trust: X drops on the word of its stale log; X and Y hold no copy')
    ann(X, 'semitrust', 'y')
    restore(X, Y)
    # 8. `dead`: a dead custodian's copy never counts, even present.
    ann(X, 'dead', 'y')
    ok = drop(X, 1, ignore=('z',))
    check(not ok and has_key(X, KEY), "dead: X's drop never counts Y's copy, though present")
    ann(X, 'semitrust', 'y')
    for f in filters:
        f.stop()
    print(f'E23 ({"patched" if PATCHED else "stock"} git-annex): {len(FAILS)} failure(s)')
    sys.exit(1 if FAILS else 0)


if __name__ == '__main__':
    try:
        main()
    finally:
        for d in DAEMONS:
            d.stop()
