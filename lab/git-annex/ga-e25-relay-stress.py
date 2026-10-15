#!/usr/bin/env python3
"""E25 (ADR 0069; spec 16 §16.4): git fetch over git-annex's P2P protocol, many times.

A bare custodian C, served by remotedaemon on E12's lab network, gets a commit
holding a blob of random size before each of N fetches (default 200) by a peer
P over `p2p-annex::`. A fetch still running after 20 s hangs: git-annex's relay
sent CONNECTDONE before the end of git's output (P2P/IO.hs runRelayService),
which third_party/git-annex patch 0002 fixes. Asserted for pigeon's git-annex
(`-pigeon` in its version): no fetch hangs and each brings its commit. For any
other git-annex it counts the hangs and asserts only that every fetch that
ended brought its commit: the negative control of git-annex.yml.
Usage: ga-e25-relay-stress.py [N] [--stop-after-hangs N] (LAB defaults to ~/ga-lab25). Linux and macOS,
about 1 min for 200 fetches. The optional hang limit applies only to stock builds.
"""
import argparse, os, shutil, signal, subprocess, sys, time

LAB = os.path.expanduser(os.environ.get('LAB', '~/ga-lab25'))
NET, BIN = os.path.join(LAB, 'net'), os.path.join(LAB, 'bin')
ENV = dict(os.environ, LABNET=NET, PATH=BIN + ':' + os.environ['PATH'],
           GIT_CONFIG_GLOBAL='/dev/null', GIT_CONFIG_NOSYSTEM='1',
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
    link = os.path.join(net, a[0] + '.sock')
    if os.path.lexists(link): os.unlink(link)
    os.symlink(os.path.abspath(a[1]), link)
    while True: time.sleep(3600)
s = socket.socket(socket.AF_UNIX)
try: s.connect(os.path.join(net, a[0] + '.sock'))
except OSError as e: print('cannot connect:', e, file=sys.stderr); sys.exit(1)
def up():
    while (b := os.read(0, 65536)): s.sendall(b)
    try: s.shutdown(socket.SHUT_WR)
    except OSError: pass  # macOS: ENOTCONN once the peer has gone
threading.Thread(target=up, daemon=True).start()
while (b := s.recv(65536)): os.write(1, b)
'''


def check(cond, what):
    print(('ok   ' if cond else 'FAIL ') + what, flush=True)
    if not cond:
        FAILS.append(what)


def run(cwd, *cmd, ok=True, inp=None, timeout=90):
    """On timeout the whole process group is killed, and TimeoutExpired raised."""
    with subprocess.Popen(cmd, cwd=cwd, env=ENV, text=True, start_new_session=True,
                          stdin=subprocess.DEVNULL if inp is None else subprocess.PIPE,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE) as q:
        try:
            out, err = q.communicate(inp, timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(q.pid, signal.SIGKILL)
            q.communicate()
            raise
    if ok and q.returncode:
        sys.exit(f'{cwd}: {" ".join(cmd)} failed: {err.strip()[-400:]}')
    return subprocess.CompletedProcess(cmd, q.returncode, out, err)


def git(cwd, *a, **k):
    return run(cwd, 'git', *a, **k).stdout.strip()


def commit(repo, i):
    """A commit on C's main holding one blob of random size, made with plumbing."""
    blob = run(repo, 'git', 'hash-object', '-w', '--stdin',
               inp=os.urandom(int(10 ** (2 + 3 * (i % 7) / 6))).hex()).stdout.strip()
    tree = run(repo, 'git', 'mktree', inp=f'100644 blob {blob}\tf{i}\n').stdout.strip()
    parent = git(repo, 'rev-parse', '-q', '--verify', 'main', ok=False)
    c = git(repo, 'commit-tree', tree, *(['-p', parent] if parent else []), '-m', f'c{i}')
    git(repo, 'update-ref', 'refs/heads/main', c)
    return c


def setup():
    if os.path.exists(LAB):
        subprocess.run(['chmod', '-R', 'u+w', LAB])
        shutil.rmtree(LAB)
    os.makedirs(NET)
    os.makedirs(BIN)
    with open(os.path.join(BIN, 'git-annex-p2p-lab'), 'w') as f:
        f.write(HELPER)
    os.chmod(os.path.join(BIN, 'git-annex-p2p-lab'), 0o755)
    if not shutil.which('git-remote-p2p-annex', path=ENV['PATH']):
        os.symlink(shutil.which('git-annex', path=ENV['PATH']), os.path.join(BIN, 'git-remote-p2p-annex'))
        NOTES.append('this git-annex has no git-remote-p2p-annex; linked it to git-annex')
    C, P = os.path.join(LAB, 'C.git'), os.path.join(LAB, 'P')
    git(LAB, 'init', '-q', '--bare', '-b', 'main', C)
    git(LAB, 'init', '-q', '-b', 'main', P)
    for r, name in ((C, 'C'), (P, 'P')):
        run(r, 'git', 'annex', 'init', name)
    run(C, 'git', 'annex', 'p2p', '--enable', 'lab')
    return C, P


def positive_int(value):
    n = int(value)
    if n <= 0:
        raise argparse.ArgumentTypeError('must be positive')
    return n


def fetches(custodian, peer, n, stop_after_hangs=None):
    """Exercise the relay, preserving errors even when the stock hang budget ends."""
    hangs, wrong, failed, began, done = [], [], [], time.time(), 0
    for i in range(n):
        if stop_after_hangs is not None and len(hangs) >= stop_after_hangs:
            NOTES.append(f'stopped after {done} fetches: {len(hangs)} hangs observed')
            break
        if len(hangs) >= 5 and hangs[-5:] == list(range(i - 5, i)):
            NOTES.append(f'stopped after fetch {i}: five fetches in a row hung')
            break
        want, done = commit(custodian, i), i + 1
        try:
            r = run(peer, 'git', 'fetch', '-q', 'c', 'main', ok=False, timeout=20)
        except subprocess.TimeoutExpired:
            hangs.append(i)
            continue
        if r.returncode:
            failed.append((i, r.stderr.strip()[-200:]))
        elif git(peer, 'rev-parse', '-q', '--verify', 'refs/remotes/c/main', ok=False) != want:
            wrong.append(i)
    return hangs, wrong, failed, time.time() - began, done


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('n', nargs='?', type=positive_int, default=200)
    parser.add_argument('--stop-after-hangs', type=positive_int,
                        help='stop the stock negative control after this many hangs; ignored for patched builds')
    args = parser.parse_args()
    n = args.n
    C, P = setup()
    patched = '-pigeon' in git(LAB, 'annex', 'version', '--raw')
    daemon = subprocess.Popen(['git', 'annex', 'remotedaemon', '--foreground'], cwd=C, env=ENV,
                              stdin=subprocess.PIPE, stdout=subprocess.DEVNULL,
                              stderr=subprocess.DEVNULL, start_new_session=True)
    try:
        end = time.time() + 30
        while not os.path.lexists(os.path.join(NET, 'C.sock')) and time.time() < end:
            time.sleep(0.1)
        addr = [l for l in run(C, 'git', 'annex', 'p2p', '--gen-addresses').stdout.splitlines()
                if l.startswith('p2p-annex::')][0]
        run(P, 'git', 'annex', 'p2p', '--link', '--name', 'c', inp=addr + '\n')
        hangs, wrong, failed, took, done = fetches(
            C, P, n, None if patched else args.stop_after_hangs)
    finally:
        os.killpg(daemon.pid, signal.SIGKILL)
        daemon.wait()
    flavour = 'patched' if patched else 'stock'
    print(f'E25 ({flavour} git-annex): {done} fetches in {took:.0f} s, {len(hangs)} hung {hangs}, '
          f'{len(failed)} failed, {len(wrong)} without their commit', flush=True)
    check(not wrong and not failed,
          f'every fetch that ended brought its commit (failed: {failed[:3]}; wrong: {wrong[:5]})')
    if patched:
        check(not hangs, f'patched: none of {n} fetches over P2P hangs: {hangs}')
    else:
        NOTES.append(f'stock: {len(hangs)} of {done} attempted fetches hung (the relay race of 16 §16.4)')
    for note in NOTES:
        print('note', note)
    print(f'E25: {len(FAILS)} failure(s)')
    sys.exit(1 if FAILS else 0)


if __name__ == '__main__':
    main()
