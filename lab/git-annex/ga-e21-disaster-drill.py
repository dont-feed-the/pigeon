#!/usr/bin/env python3
"""E21 (issue #11): a disaster drill. Every laptop and its key is lost; a fresh
machine recovers the group from the surviving custodians alone.

Laptops L (Paul, admin) and M (Martine), bare custodians C1 and C2, K = 2,
numcopies = mincopies = 2, laptops untrusted, every commit signed with an
ed25519 device key (gpg.format=ssh), `.pigeon/roster.toml` in the tree, a
recovery sheet S of Paul in the roster (role "recovery"). Pull only. Asserts:
1. content on K custodians whose commit they lack is lost to names: the old
   rule of spec 06 §6.2 calls it protected, the new one (holders must have
   the commit) does not; 2-4. a fresh machine N rebuilds names, hashes,
   deleted history, the suggestion and every signature from C1 and C2;
5. S adds a device for Paul and nothing else; 6. plain git-annex commands
   export complete files without pigeon; 7. a copied repository counts only
   under a new UUID and after a hashing fsck; 8. git's default fsync leaves
   loose objects and refs unsynced, git-annex never fsyncs its objects.
Prints PASS or FAIL per check, exits non-zero on failure. Environment: LAB.
"""
import base64, hashlib, json, os, shutil, struct, subprocess, sys, tempfile, time, tomllib

os.environ['PATH'] = os.path.expanduser('~/git-annex.linux') + ':' + os.environ['PATH']
os.environ.update(GIT_CONFIG_GLOBAL='/dev/null', GIT_CONFIG_NOSYSTEM='1', GIT_AUTHOR_NAME='lab',
                  GIT_AUTHOR_EMAIL='lab@pigeon', GIT_COMMITTER_NAME='lab', GIT_COMMITTER_EMAIL='lab@pigeon')
for v in ('GIT_DIR', 'GIT_WORK_TREE', 'GIT_INDEX_FILE'):
    os.environ.pop(v, None)
K = 2
FAILS, RESULTS = [], []
SIGS = {}                                   # commit -> (signer device ID or None, verified)


def fail(msg):
    FAILS.append(msg)
    print('  fail:', msg, flush=True)


def check(cond, msg):
    if not cond:
        fail(msg)
    return cond


def result(n, title, before):
    ok = len(FAILS) == before
    RESULTS.append((n, ok))
    print(f'{"PASS" if ok else "FAIL"}: E21.{n} {title}', flush=True)


def blob(label, size=2000):
    return (f'{label} ' * size).encode()[:size]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def sh(*args, cwd=None, check=True, stdin=None, env=None):
    r = subprocess.run(list(args), cwd=cwd, capture_output=True, text=True, input=stdin,
                       env=dict(os.environ, **(env or {})))
    if check and r.returncode:
        raise RuntimeError(f'{" ".join(args)} in {cwd}: {r.stderr.strip()}')
    return r


def keygen(path, comment):
    sh('ssh-keygen', '-q', '-t', 'ed25519', '-N', '', '-C', comment, '-f', path)
    return device_id(open(path + '.pub').read())


def device_id(pub_line):
    """Spec 09 §9.1: the device ID is the ed25519 public key in 64 hex digits."""
    raw = base64.b64decode(pub_line.split()[1])
    return ssh_string(raw, ssh_string(raw, 0)[1])[0].hex()


def ssh_string(buf, i):
    n = struct.unpack('>I', buf[i:i + 4])[0]
    return buf[i + 4:i + 4 + n], i + 4 + n


class Repo:
    def __init__(self, root, name, bare=False, key=None):
        self.root, self.name, self.bare, self.key = root, name, bare, key
        self.trees = {}

    def run(self, *args, check=True, stdin=None, env=None):
        return sh('git', *args, cwd=self.root, check=check, stdin=stdin, env=env).stdout

    def ok(self, *args):
        return sh('git', *args, cwd=self.root, check=False).returncode == 0

    def rev(self, ref):
        return self.run('rev-parse', '-q', '--verify', ref + '^{commit}', check=False).strip() or None

    def head(self):
        return self.rev('refs/heads/main')

    def is_ancestor(self, a, b):
        return bool(a and b) and self.ok('merge-base', '--is-ancestor', a, b)

    def uuid(self):
        return self.run('config', 'annex.uuid').strip()

    def write(self, rel, data):
        p = os.path.join(self.root, rel)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, 'wb') as h:
            h.write(data)

    def read(self, rel):
        with open(os.path.join(self.root, rel), 'rb') as h:
            return h.read()

    def commit(self, msg, add=(), remove=(), plain=(), key=None):
        for rel in remove:
            self.run('rm', '-q', '--', rel)
        if add:
            self.run('annex', 'add', '-q', '--', *add)
        if plain:
            self.run('add', '--', *plain)
        self.run('-c', 'gpg.format=ssh', '-c', f'user.signingkey={key or self.key}',
                 'commit', '-S', '-q', '--allow-empty', '-m', msg)
        return self.rev('HEAD')

    def tree(self, commit):
        """path -> annex key (or None for a plain blob) in commit's tree."""
        if commit not in self.trees:
            out = {}
            for item in self.run('ls-tree', '-r', '-z', '--full-tree', commit).split('\0'):
                if item:
                    meta, path = item.split('\t', 1)
                    data = self.run('cat-file', 'blob', meta.split()[2])
                    out[path] = data.strip().rsplit('/', 1)[1] if 'annex/objects/' in data else None
            self.trees[commit] = out
        return self.trees[commit]

    def proposals(self):
        return self.run('for-each-ref', '--format=%(objectname)', 'refs/pigeon/proposals/').split()

    def references(self, tips):
        """(commit, path, key) for every commit of tips' history that introduces key at path."""
        refs = []
        for line in self.run('rev-list', '--parents', *[t for t in tips if t]).splitlines():
            c, *parents = line.split()
            theirs = [self.tree(p) for p in parents]
            refs += [(c, p, k) for p, k in self.tree(c).items() if k and all(t.get(p) != k for t in theirs)]
        return refs

    def present(self, key):
        return self.ok('annex', 'contentlocation', key)

    def object_bytes(self, key):
        path = self.run('annex', 'contentlocation', key).strip()
        with open(os.path.join(self.root, path), 'rb') as h:
            return h.read()

    def show(self, rev, path):
        r = sh('git', 'show', f'{rev}:{path}', cwd=self.root, check=False)
        return r.stdout if r.returncode == 0 else None

    def roster(self, commit):
        return tomllib.loads(self.show(commit, '.pigeon/roster.toml'))

    def whereis(self, key):
        r = sh('git', 'annex', 'whereis', '--json', '--key', key, cwd=self.root, check=False)
        return {w['uuid'] for w in json.loads(r.stdout or '{"whereis": []}')['whereis']} if r.stdout else set()


# ---- signatures and the authority rule (spec 04 §4.4, a lab subset)
def signature(repo, commit):
    """(signer device ID, verified) of commit's SSHSIG, checked with ssh-keygen -Y verify."""
    if commit in SIGS:
        return SIGS[commit]
    raw = subprocess.run(['git', 'cat-file', 'commit', commit], cwd=repo.root, capture_output=True,
                         check=True).stdout
    head, _, body = raw.partition(b'\n\n')
    kept, sig, in_sig = [], [], False
    for line in head.split(b'\n'):
        if line.startswith(b'gpgsig '):
            in_sig, sig = True, [line[7:]]
        elif in_sig and line.startswith(b' '):
            sig.append(line[1:])
        else:
            in_sig = False
            kept.append(line)
    if not sig:
        SIGS[commit] = (None, False)
        return SIGS[commit]
    armored = b'\n'.join(sig) + b'\n'
    der = base64.b64decode(b''.join(l for l in sig if not l.startswith(b'-----')))
    pub = ssh_string(der, 10)[0]                    # b'SSHSIG' + uint32 version, then the public key
    signer = ssh_string(pub, ssh_string(pub, 0)[1])[0].hex()
    with tempfile.TemporaryDirectory() as d:
        open(os.path.join(d, 'sig'), 'wb').write(armored)
        open(os.path.join(d, 'allowed'), 'w').write(
            f'signer namespaces="git" ssh-ed25519 {base64.b64encode(pub).decode()}\n')
        r = subprocess.run(['ssh-keygen', '-Y', 'verify', '-f', os.path.join(d, 'allowed'), '-I', 'signer',
                            '-n', 'git', '-s', os.path.join(d, 'sig')], input=b'\n'.join(kept) + b'\n\n' + body,
                           capture_output=True)
    SIGS[commit] = (signer, r.returncode == 0)
    return SIGS[commit]


def roster_valid(r):
    uuids = [d['annex_uuid'] for d in r['devices'].values() if 'annex_uuid' in d]
    return (r.get('format') == 1 and r.get('k', 0) >= 1 and any(p.get('admin') for p in r['persons'].values())
            and all(d['person'] in r['persons'] for d in r['devices'].values()) and len(uuids) == len(set(uuids)))


def allowed(repo, commit, parent, R, signer):
    """None if every change of commit relative to parent is allowed to signer under roster R."""
    dev = R['devices'][signer]
    person, recovery = dev['person'], dev.get('role') == 'recovery'
    admin = R['persons'][person].get('admin', False)
    for line in repo.run('diff-tree', '-r', '--no-renames', '--name-status', parent, commit).splitlines():
        status, path = line.split('\t', 1)
        if path == '.pigeon/roster.toml':
            new = repo.roster(commit)
            if not roster_valid(new):
                return 'invalid-roster'
            same = {k: v for k, v in R.items() if k != 'devices'} == {k: v for k, v in new.items() if k != 'devices'}
            changed = [i for i in set(R['devices']) | set(new['devices'])
                       if R['devices'].get(i) != new['devices'].get(i)]
            own = all(e.get('person') == person for i in changed
                      for e in (R['devices'].get(i, {}), new['devices'].get(i, {})) if e)
            if same and own:
                continue                    # ADR 0017: a device of P may add or remove devices of P
            if recovery:
                return 'recovery-scope'
            if not admin:
                return 'roster-needs-admin'
        elif recovery:
            return 'recovery-scope'         # a recovery sheet changes device entries of its person only
        elif path.startswith('.pigeon/') and not admin:
            return 'pigeon-needs-admin'
        elif status != 'A' and not admin:
            return 'modify-needs-admin'     # the default rule: add-only, the admins as stewards
    return None


def validate(repo, tip, genesis):
    """commit -> 'accepted' or the reason it is not, for tip's history."""
    verdicts = {}
    for line in repo.run('rev-list', '--topo-order', '--reverse', '--parents', tip).splitlines():
        c, *parents = line.split()
        signer, good = signature(repo, c)
        if not good:
            verdicts[c] = 'bad-signature'
        elif not parents:
            R = repo.roster(c)
            verdicts[c] = 'accepted' if c == genesis and signer in R['devices'] and roster_valid(R) else 'not-genesis'
        else:
            A = [p for p in parents if verdicts.get(p) == 'accepted']
            rosters = [repo.roster(p) for p in A]
            if not A:
                verdicts[c] = 'no-accepted-parent'
            elif not all(signer in R['devices'] for R in rosters):
                verdicts[c] = 'signer-not-in-roster'
            else:
                verdicts[c] = next((v for p, R in zip(A, rosters) if (v := allowed(repo, c, p, R, signer))),
                                   'accepted')
    return verdicts


def accept(repo, peer):
    """Validate the peer's main fetched into refs/pigeon/peers/<peer>/main and fast-forward to it."""
    tip, genesis = repo.rev(f'refs/pigeon/peers/{peer}/main'), GENESIS[0]
    v = validate(repo, tip, genesis)
    if v[tip] != 'accepted':
        return v[tip]
    cur = repo.head()
    if cur and not repo.is_ancestor(cur, tip):
        return 'not-fast-forward'
    if repo.bare or not cur:
        repo.run('update-ref', 'refs/heads/main', tip)
        if not repo.bare:
            repo.run('reset', '-q', '--hard', tip)
    else:
        repo.run('merge', '-q', '--ff-only', tip)
    return 'accepted'


def fetch(repo, remote, peer, main=True):
    """Spec 03 §3.6 refspecs, pull only."""
    specs = [f'+refs/heads/git-annex:refs/remotes/{remote}/git-annex',
             'refs/pigeon/proposals/*:refs/pigeon/proposals/*']
    if main:
        specs.append(f'+refs/heads/main:refs/pigeon/peers/{peer}/main')
    repo.run('fetch', '-q', remote, *specs)
    repo.run('annex', 'merge', '-q')
    return accept(repo, peer) if main else None


GENESIS = []


def roster_toml(persons, devices, top=None):
    top = top or {'format': 1, 'group': 'Lab', 'k': K, 'retention_days': 30}
    out = ''.join(f'{k} = {json.dumps(v)}\n' for k, v in top.items())
    for pid, p in persons.items():
        out += f'\n[persons.{pid}]\nname = "{p["name"]}"\n' + ('admin = true\n' if p.get('admin') else '')
    for did, d in devices.items():
        out += f'\n[devices."{did}"]\n' + ''.join(f'{k} = "{v}"\n' for k, v in d.items())
    return out.encode()


def new_rule(holders_of, has_commit, ref):
    """Spec 06 §6.2 rule 1, as issue #11 asks: holders count only if they also have the commit."""
    c, _, k = ref
    return sum(1 for x in holders_of(k) if has_commit(x, c)) >= K


def make_custodian(lab, name):
    r = Repo(os.path.join(lab, name), name, bare=True)
    os.makedirs(r.root)
    r.run('init', '-q', '--bare', '-b', 'main')
    r.run('annex', 'init', '-q', name)
    r.run('annex', 'group', '.', 'custodian')
    return r


def make_laptop(lab, name, key):
    r = Repo(os.path.join(lab, name), name, key=key)
    os.makedirs(r.root)
    r.run('init', '-q', '-b', 'main')
    r.run('annex', 'init', '-q', name)
    r.run('config', 'annex.addunlocked', 'true')
    r.run('annex', 'untrust', 'here')
    return r


def wipe(path):
    if os.path.exists(path):
        subprocess.run(['chmod', '-R', 'u+w', path])
        shutil.rmtree(path)


def main():
    t_start = time.time()
    lab = os.environ.get('LAB', os.path.expanduser('~/ga-lab21'))
    wipe(lab)
    keys = os.path.join(lab, 'keys')
    os.makedirs(keys)
    kp = {n: os.path.join(keys, n) for n in ('L', 'M', 'S', 'N')}
    ids = {n: keygen(kp[n], n) for n in ('L', 'M', 'S')}

    # ---- the group: genesis by L, then custodians, the recovery sheet and Martine
    C1, C2 = make_custodian(lab, 'C1'), make_custodian(lab, 'C2')
    L = make_laptop(lab, 'L', kp['L'])
    L.run('annex', 'numcopies', str(K))
    L.run('annex', 'mincopies', str(K))
    persons = {'paul': {'name': 'Paul', 'admin': True}}
    devices = {ids['L']: {'person': 'paul', 'name': "Paul's laptop", 'role': 'laptop', 'annex_uuid': L.uuid()}}
    L.write('.pigeon/roster.toml', roster_toml(persons, devices))
    GENESIS.append(L.commit('genesis', plain=['.pigeon']))
    devices[ids['S']] = {'person': 'paul', 'name': "Paul's recovery sheet", 'role': 'recovery'}
    for c in (C1, C2):
        ids[c.name] = keygen(os.path.join(keys, c.name), c.name)
        devices[ids[c.name]] = {'person': 'paul', 'name': f"Paul's NAS {c.name}", 'role': 'custodian',
                                'annex_uuid': c.uuid()}
    L.write('.pigeon/roster.toml', roster_toml(persons, devices))
    L.commit('add the custodians and the recovery sheet', plain=['.pigeon'])
    M = make_laptop(lab, 'M', kp['M'])
    persons['martine'] = {'name': 'Martine'}
    devices[ids['M']] = {'person': 'martine', 'name': "Martine's laptop", 'role': 'laptop', 'annex_uuid': M.uuid()}
    L.write('.pigeon/roster.toml', roster_toml(persons, devices))
    L.commit('invite Martine', plain=['.pigeon'])
    repos = {r.name: r for r in (L, M, C1, C2)}
    for r in repos.values():
        for o in repos.values():
            if o is not r:
                r.run('remote', 'add', o.name, o.root)

    # ---- E21.2 history: adds, an edit, a deletion, a rename, a suggestion by Martine
    before = len(FAILS)
    data = {'Docs/a.txt': blob('a1'), 'Docs/b.txt': blob('b1'), 'Photos/p1.jpg': blob('p1', 5000),
            'Notes/n.md': blob('n1')}
    for p, d in data.items():
        L.write(p, d)
    L.commit('A1: add four files', add=list(data))
    data['Docs/a.txt'] = blob('a2')
    L.write('Docs/a.txt', data['Docs/a.txt'])
    L.commit('A2: edit a.txt', add=['Docs/a.txt'])
    deleted = data.pop('Notes/n.md')
    a3 = L.commit('A3: delete n.md', remove=['Notes/n.md'])
    os.makedirs(os.path.join(L.root, 'Photos/2026'))
    L.run('mv', 'Photos/p1.jpg', 'Photos/2026/p1.jpg')
    data['Photos/2026/p1.jpg'] = data.pop('Photos/p1.jpg')
    L.commit('A4: rename p1.jpg')
    check(fetch(M, 'L', 'L') == 'accepted', 'M does not accept L\'s main')
    M.run('checkout', '-q', '-b', 'suggestion')
    suggestion = blob('b-by-martine')
    M.write('Docs/b.txt', suggestion)
    P = M.commit('suggestion: b.txt', add=['Docs/b.txt'])
    M.run('checkout', '-q', 'main')
    M.run('branch', '-q', '-D', 'suggestion')
    M.run('update-ref', f'refs/pigeon/proposals/{ids["M"]}/{P}', P)
    pkey = M.tree(P)['Docs/b.txt']
    for c in (C1, C2):
        check(fetch(c, 'L', 'L') == 'accepted', f'{c.name} does not accept L\'s main')
        fetch(c, 'M', 'M', main=False)
        check(c.rev(f'refs/pigeon/proposals/{ids["M"]}/{P}') == P, f'{c.name} lacks the suggestion')
        check(validate(c, P, GENESIS[0])[P] == 'modify-needs-admin', 'the suggestion is accepted')
    M.run('annex', 'copy', '-q', '--to', 'C1', '--key', pkey)
    history = L.references([L.head()])
    for _, _, k in history:
        L.run('annex', 'copy', '-q', '--to', 'C1', '--key', k)
    for k in sorted({r[2] for r in C2.references([C2.head()] + C2.proposals())}):
        C2.run('annex', 'get', '-q', '--key', k)
    # the control: custodians fetch the commit first, then receive its content
    data['Photos/fetched.jpg'] = blob('fetched')
    L.write('Photos/fetched.jpg', data['Photos/fetched.jpg'])
    a5 = L.commit('A5: fetched.jpg', add=['Photos/fetched.jpg'])
    for c in (C1, C2):
        fetch(c, 'L', 'L')
        L.run('annex', 'copy', '-q', '--to', c.name, '--key', L.tree(a5)['Photos/fetched.jpg'])
    for c in (C1, C2):                          # custodians learn each other's location logs
        for o in (C1, C2):
            if o is not c:
                fetch(c, o.name, o.name)
    manifest = {p: sha(d) for p, d in data.items()}
    refs = C1.references([C1.head(), P])
    holders = lambda k: [c for c in (C1, C2) if c.present(k)]
    has = lambda x, c: x.is_ancestor(c, x.head()) or any(x.is_ancestor(c, t) for t in x.proposals())
    unsafe = [r for r in refs if not new_rule(holders, has, r)]
    check(not unsafe, f'not protected under the new rule: {unsafe}')
    kinds = {'deletion': 'Notes/n.md' not in L.tree(a3) and 'Notes/n.md' in L.tree(f'{a3}^'),
             'rename': L.tree(a5).get('Photos/2026/p1.jpg') == L.tree(a3).get('Photos/p1.jpg') is not None,
             'edit': L.tree(a3)['Docs/a.txt'] != L.tree(f'{a3}~2')['Docs/a.txt']}
    check(all(kinds.values()), f'history lacks {kinds}')
    result(2, f'history ({len(refs)} references: adds, an edit, a deletion, a rename, a suggestion), '
              f'all on C1 and C2 with their commits', before)

    # ---- E21.1 content on K custodians whose commit they lack
    before = len(FAILS)
    last = blob('last')
    L.write('Photos/last.jpg', last)
    a6 = L.commit('A6: last.jpg, just before the disaster', add=['Photos/last.jpg'])
    klast = L.tree(a6)['Photos/last.jpg']
    for c in (C1, C2):
        L.run('annex', 'copy', '-q', '--to', c.name, '--key', klast)
        L.run('fetch', '-q', c.name, f'+refs/heads/main:refs/pigeon/peers/{c.name}/main')
    cust = {C1.uuid(): 'C1', C2.uuid(): 'C2'}
    seen = sorted(cust[u] for u in L.whereis(klast) if u in cust)
    old = len(seen) >= K
    peer_has = lambda x, c: L.is_ancestor(c, L.rev(f'refs/pigeon/peers/{x.name}/main'))
    new = new_rule(lambda k: [c for c in (C1, C2) if c.name in seen], peer_has, (a6, '', klast))
    check(old and not new, f'last.jpg: whereis {seen}, old rule {old}, new rule {new}')
    ctl = (a5, 'Photos/fetched.jpg', L.tree(a5)['Photos/fetched.jpg'])
    check(new_rule(lambda k: [C1, C2], peer_has, ctl), 'fetched.jpg is not protected under the new rule')
    # ---- E21.3 the disaster: every laptop and every laptop key
    for r in (L, M):
        wipe(r.root)
    for n in ('L', 'M'):
        os.remove(kp[n])
        os.remove(kp[n] + '.pub')
    unused = C1.run('annex', 'unused')
    check(klast in unused, 'C1: git annex unused does not list the key of last.jpg')
    named = {r[2] for r in C1.references([C1.head()] + C1.proposals())}
    check(klast not in named, 'a commit on a custodian names last.jpg')
    result(1, f'last.jpg on {"+".join(seen)} without its commit: old rule "protected", new rule not; after the '
              'disaster its content has no name (git annex unused on C1); fetched.jpg, commit first, is '
              'protected under both', before)
    print('PASS: E21.3 disaster: L, M and their keys are deleted', flush=True)
    RESULTS.append((3, True))

    # ---- E21.4 a fresh machine N from C1 and C2 only
    before = len(FAILS)
    N = Repo(os.path.join(lab, 'N'), 'N', key=kp['N'])
    sh('git', 'clone', '-q', C1.root, N.root)
    for cmd in (['annex', 'init', '-q', 'N'], ['remote', 'add', 'C2', C2.root], ['fetch', '-q', 'C2'],
                ['fetch', '-q', 'origin', 'refs/pigeon/proposals/*:refs/pigeon/proposals/*'],
                ['annex', 'merge', '-q']):
        N.run(*cmd)
    tips = [N.head()] + N.proposals()
    all_keys = sorted({r[2] for r in N.references(tips)})
    got = [k for k in all_keys if N.ok('annex', 'get', '-q', '--key', k)]
    check(got == all_keys, f'N got {len(got)} of {len(all_keys)} keys')
    fsck = sh('git', 'annex', 'fsck', '-q', '--all', cwd=N.root, check=False)
    check(fsck.returncode == 0, f'fsck --all on N: {fsck.stdout[-300:]} {fsck.stderr[-300:]}')
    files = {p: sha(N.read(p)) for p, k in N.tree(N.head()).items() if k}
    check(files == manifest, f'names and hashes differ: {sorted(set(files.items()) ^ set(manifest.items()))}')
    dkey = N.tree(N.rev(f'{a3}^'))['Notes/n.md']
    check(sha(N.object_bytes(dkey)) == sha(deleted), 'the deleted n.md does not come back')
    check(sha(N.object_bytes(pkey)) == sha(suggestion), 'the suggestion does not come back')
    v_main, v_prop = validate(N, N.head(), GENESIS[0]), validate(N, P, GENESIS[0])
    check(all(x == 'accepted' for x in v_main.values()), f'N rejects {v_main}')
    check(v_prop[P] == 'modify-needs-admin' and all(SIGS[c][1] for c in v_prop), f'suggestion: {v_prop[P]}')
    check(N.whereis(klast) >= {C1.uuid(), C2.uuid()} and klast not in {r[2] for r in N.references(tips)},
          'N should know that C1 and C2 hold last.jpg, under no name')
    result(4, f'N from C1+C2: {len(all_keys)} keys (history and suggestion), fsck --all hashed, '
              f'{len(files)} names and hashes equal, n.md and the suggestion back, '
              f'{len(v_main)} commits signed by roster devices; last.jpg unnamed', before)

    # ---- E21.5 the recovery sheet adds a device of Paul, and nothing else
    before = len(FAILS)
    ids['N'] = keygen(kp['N'], 'N')
    roster = N.roster(N.head())
    roster['devices'][ids['N']] = {'person': 'paul', 'name': "Paul's new laptop", 'role': 'laptop',
                                   'annex_uuid': N.uuid()}
    N.write('.pigeon/roster.toml', roster_toml(roster['persons'], roster['devices']))
    base = N.head()
    rec = N.commit('recovery sheet: add Paul\'s new laptop', plain=['.pigeon'], key=kp['S'])
    C1.run('remote', 'add', 'N', N.root)
    verdict = fetch(C1, 'N', 'N')
    check(verdict == 'accepted' and C1.head() == rec, f'C1 refuses the sheet\'s commit: {verdict}')
    bad = {}
    for label, change in (('k', lambda r: r.update(k=1)),
                          ('another person', lambda r: r['devices'].__setitem__('ab' * 32, {
                              'person': 'martine', 'name': 'x', 'role': 'laptop'})),
                          ('a file', None)):
        N.run('checkout', '-q', '-b', 'try', base)
        if change:
            r = N.roster(base)
            change(r)
            top = {k: v for k, v in r.items() if k not in ('persons', 'devices')}
            N.write('.pigeon/roster.toml', roster_toml(r['persons'], r['devices'], top))
            c = N.commit(f'sheet tries {label}', plain=['.pigeon'], key=kp['S'])
        else:
            N.write('Docs/new.txt', blob('new'))
            c = N.commit('sheet tries a file', add=['Docs/new.txt'], key=kp['S'])
        bad[label] = validate(N, c, GENESIS[0])[c]
        N.run('checkout', '-q', 'main')
        N.run('branch', '-q', '-D', 'try')
    check(all(v == 'recovery-scope' for v in bad.values()), f'the sheet may: {bad}')
    roster = N.roster(rec)
    for gone in (ids['L'], ids['S']):
        del roster['devices'][gone]
    N.write('.pigeon/roster.toml', roster_toml(roster['persons'], roster['devices']))
    rot = N.commit('remove the lost laptop and the used sheet', plain=['.pigeon'])
    check(fetch(C1, 'N', 'N') == 'accepted' and C1.head() == rot, 'C1 refuses N\'s own admin commit')
    result(5, f'the sheet\'s commit adding N for Paul is accepted by C1 (pulled); the sheet is refused for '
              f'{", ".join(bad)}; N, now Paul\'s device, removes L and the used sheet', before)

    # ---- E21.6 export with plain git-annex, from a partial C1 and C2
    before = len(FAILS)
    kp1 = N.tree(N.head())['Photos/2026/p1.jpg']
    C1.run('annex', 'drop', '-q', '--force', '--key', kp1)      # C1 partial: only C2 has p1.jpg now
    check(not C1.present(kp1) and C2.present(kp1), 'C1 is not partial')
    out = os.path.join(lab, 'export')
    recipe = [['git', 'clone', '-q', C1.root, out], ['git', 'annex', 'init', '-q'],
              ['git', 'remote', 'add', 'c2', C2.root], ['git', 'fetch', '-q', 'c2'],
              ['git', 'annex', 'get', '-q', '.']]
    for cmd in recipe:
        r = sh(*cmd, cwd=out if os.path.isdir(out) else lab, check=False)
        check(r.returncode == 0, f'{" ".join(cmd)}: {r.stderr.strip()[-300:]}')
    E = Repo(out, 'export')
    exported = {}
    for p, k in E.tree(E.head()).items():
        b = E.read(p)
        check(not b.startswith(b'/annex/objects/'), f'{p} is still a pointer file')
        if k:
            exported[p] = sha(b)
    check(exported == manifest, f'export differs: {sorted(set(exported.items()) ^ set(manifest.items()))}')
    hist = sh('git', 'annex', 'get', '--all', cwd=out, check=False)
    missing = [l.split()[1] for l in hist.stdout.splitlines() if l.startswith('get ') and 'not available' in l]
    check(E.present(dkey) and E.present(pkey), 'get --all does not bring the deleted n.md and the suggestion')
    check(all(not any(C.ok('annex', 'contentlocation', k) for C in (C1, C2)) for k in missing),
          f'get --all misses keys that a custodian holds: {missing}')
    result(6, f'plain git-annex export: {len(exported)} complete files with their hashes, p1.jpg from C2 '
              f'only; get --all brings history keys, and reports {len(missing)} key only N had (the '
              'refused sheet commit)', before)
    RECIPE = [' '.join(c).replace(C1.root, '<custodian 1>').replace(C2.root, '<custodian 2>')
              .replace(out, '<folder>') for c in recipe]

    # ---- E21.7 a copied repository counts only under a new UUID and after a hashing fsck
    before = len(FAILS)
    C3 = Repo(os.path.join(lab, 'C3'), 'C3', bare=True)
    sh('cp', '-a', C1.root, C3.root)
    same = C3.uuid() == C1.uuid()
    copies = {k: E.whereis(k) for k in all_keys}
    E.run('remote', 'add', 'same', C3.root)
    E.run('fetch', '-q', 'same')
    E.run('annex', 'merge', '-q')
    check(same and copies == {k: E.whereis(k) for k in all_keys}, 'a copy with the same UUID adds copies')
    E.run('remote', 'remove', 'same')
    C3.run('config', '--unset', 'annex.uuid')
    C3.run('annex', 'init', '-q', 'C3')
    fresh = C3.uuid()
    check(fresh != C1.uuid(), 'C3 kept C1\'s UUID')
    E.run('remote', 'add', 'c3', C3.root)
    E.run('fetch', '-q', 'c3')
    E.run('annex', 'merge', '-q')
    pre = [k for k in all_keys if fresh in E.whereis(k)]
    check(pre == [], f'C3 counts {len(pre)} keys before its fsck')
    bad_key = N.tree(N.head())['Docs/a.txt']
    path = os.path.join(C3.root, C3.run('annex', 'contentlocation', bad_key).strip())
    subprocess.run(['chmod', '-R', 'u+w', os.path.dirname(path)])
    with open(path, 'r+b') as h:
        h.write(b'X' * 16)
    fs = sh('git', 'annex', 'fsck', '-q', '--all', cwd=C3.root, check=False)
    E.run('fetch', '-q', 'c3')
    E.run('annex', 'merge', '-q')
    post = {k for k in all_keys if fresh in E.whereis(k)}
    expect = {k for k in all_keys if C1.present(k)} - {bad_key}
    check(post == expect, f'after fsck C3 holds {len(post)} keys, expected {len(expect)}')
    check(fs.returncode != 0 and bad_key not in post, 'the corrupted object counts after fsck')
    result(7, f'cp -a keeps C1\'s UUID (no extra copy); `git config --unset annex.uuid && git annex init` '
              f'gives a new one, which counts no key until `git annex fsck --all` hashes {len(post)} and '
              'refuses the corrupted one', before)

    # ---- E21.8 durability: git's default fsync, and git-annex's objects
    before = len(FAILS)
    counts = durability(os.path.join(lab, 'fsync'))
    check(counts['default'] == 0 and counts['hardened'] > 0, f'fsync counters {counts}')
    result(8, f'git fetch of loose objects: {counts["default"]} fsync by default, {counts["hardened"]} with '
              f'core.fsync=objects,reference; git-annex get: {counts["annex"]}', before)

    print('recovery recipe (RECOVERY.txt):\n  ' + '\n  '.join(RECIPE))
    print(f'E21: {sum(ok for _, ok in RESULTS)}/{len(RESULTS)} checks passed in {time.time() - t_start:.0f} s; '
          + sh('git', 'annex', 'version', '--raw').stdout.strip())
    sys.exit(1 if FAILS else 0)


def durability(root):
    """trace2 fsync counters of a small git fetch, and fsync calls of git-annex on its objects."""
    os.makedirs(root)
    src = Repo(os.path.join(root, 'src'), 'src')
    os.makedirs(src.root)
    src.run('init', '-q', '-b', 'main')
    src.run('annex', 'init', '-q', 'src')
    for i in range(3):
        src.write(f'f{i}', blob(f'f{i}'))
        src.run('add', f'f{i}')
        src.run('commit', '-q', '-m', f'c{i}')
    src.write('big', blob('big', 50000))
    src.run('annex', 'add', '-q', 'big')
    src.run('commit', '-q', '-m', 'annexed')
    counts = {}
    for label, cfg in (('default', []), ('hardened', ['-c', 'core.fsync=objects,reference',
                                                      '-c', 'core.fsyncMethod=fsync'])):
        dst = Repo(os.path.join(root, label), label, bare=True)
        os.makedirs(dst.root)
        dst.run('init', '-q', '--bare', '-b', 'main')
        trace = os.path.join(root, f'{label}.trace')
        dst.run(*cfg, 'fetch', '-q', src.root, '+refs/heads/main:refs/heads/main',
                env={'GIT_TRACE2_EVENT': trace})
        counts[label] = sum(e.get('count', 0) for e in map(json.loads, open(trace))
                            if e.get('event') == 'counter' and e.get('category') == 'fsync')
    loose = sh('git', 'count-objects', cwd=os.path.join(root, 'default')).stdout
    check(not loose.startswith('0 objects'), f'the fetch wrote no loose objects: {loose}')
    counts['annex'] = 'not measured (no strace)'
    if shutil.which('strace'):
        dst = Repo(os.path.join(root, 'annex'), 'annex', bare=True)
        sh('git', 'clone', '-q', '--bare', src.root, dst.root)
        dst.run('annex', 'init', '-q', 'annex')
        log = os.path.join(root, 'strace.txt')
        r = sh('strace', '-f', '-y', '-qq', '-e', 'signal=none', '-e', 'trace=fsync,fdatasync,syncfs', '-o', log,
               'git', 'annex', 'get', '-q', '--all', '--from', 'origin', cwd=dst.root, check=False)
        if r.returncode == 0 and dst.ok('annex', 'contentlocation', src.tree('HEAD')['big']):
            calls = [l for l in open(log) if any(f'{c}(' in l for c in ('fsync', 'fdatasync', 'syncfs'))]
            on_objects = [l for l in calls if 'annex/objects' in l]
            sqlite = [l for l in calls if 'annex/keysdb' in l]
            counts['annex'] = (f'{len(on_objects)} fsync calls on annex objects ({len(calls)} sync calls in all, '
                               f'{len(sqlite)} of them SQLite\'s on the keys database)')
            check(not on_objects, f'git-annex fsyncs its objects now: {on_objects[:2]}')
    return counts


if __name__ == '__main__':
    main()
