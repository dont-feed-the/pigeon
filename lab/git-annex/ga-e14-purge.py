#!/usr/bin/env python3
"""E14 (issues #4, #5; ADR 0042): a purge releases references, not keys, and
content goes only after every custodian's seal.

A reference implementation of spec 08 §8.6 (references, coverage by paths, base
and named suggestions, seals in signed-status-like refs, the released set) runs
against real git and git-annex: laptops L and L2, bare custodians C1 and C2,
numcopies = mincopies = 2, custodians wanting `sizebalanced=custodian:2`.
Transfers are chosen key by key with `matchexpression --key`. Asserts:
- issue #4's cases: two paths sharing a key, current content sharing a key with
  purged history, and a suggestion sharing a key with accepted content all keep
  the key; keys with only covered references are released;
- a restore reaching one custodian before its seal holds the purge back at a
  custodian that has not merged it, then keeps the restored key;
- released keys are never fetched again and `drop --force --key` leaves no copy;
- a laptop returning from offline drops its covered copies and re-uploads
  content it re-added meanwhile: a new retention generation;
- git-annex contracts: matchexpression agrees with git-annex's own choices;
  `get --auto --all` and `drop --auto --key` are refused; `satisfy --all` drops
  from a remote. Exits non-zero on failure. Environment: LAB (directory).
"""
import os, shutil, subprocess, sys, tomllib
from datetime import datetime, timedelta, timezone

os.environ['PATH'] = os.path.expanduser('~/git-annex.linux') + ':' + os.environ['PATH']
CUSTODIANS = ['C1', 'C2']
WANTED = 'sizebalanced=custodian:2'
RETENTION_DAYS = 30
FAILS = []


def fail(msg):
    FAILS.append(msg)
    print('FAIL:', msg, flush=True)


def check(cond, msg):
    if not cond:
        fail(msg)


def content(label, size=3000):
    return (f'{label} ' * size).encode()[:size]


def iso(t):
    return t.strftime('%Y-%m-%dT%H:%M:%SZ')


class Repo:
    """A device's repository and pigeon's view of it."""

    def __init__(self, root, name, bare=False):
        self.root, self.name, self.bare = root, name, bare
        self.trees, self.blobs = {}, {}

    def run(self, *args, check=True, stdin=None):
        r = subprocess.run(['git', '-c', 'user.name=lab', '-c', 'user.email=lab@pigeon', *args],
                           cwd=self.root, capture_output=True, text=True, input=stdin)
        if check and r.returncode:
            raise RuntimeError(f'{self.name}: git {" ".join(args)}: {r.stderr.strip()}')
        return r.stdout

    def ok(self, *args):
        return subprocess.run(['git', *args], cwd=self.root, capture_output=True).returncode == 0

    def rev(self, ref):
        out = subprocess.run(['git', 'rev-parse', '-q', '--verify', ref + '^{commit}'], cwd=self.root,
                             capture_output=True, text=True)
        return out.stdout.strip() or None

    def head(self):
        return self.rev('refs/heads/main')

    def is_ancestor(self, a, b):
        return bool(a and b) and self.ok('merge-base', '--is-ancestor', a, b)

    def pigeon_dir(self):
        d = os.path.join(self.root, 'pigeon' if self.bare else '.git/pigeon')
        os.makedirs(d, exist_ok=True)
        return d

    # ---- laptops: files and commits
    def write(self, rel, data):
        p = os.path.join(self.root, rel)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        if os.path.lexists(p):               # a locked annexed file: a symlink to a read-only object
            os.unlink(p)
        with open(p, 'wb') as h:
            h.write(data)

    def commit(self, msg, add=(), remove=(), plain=()):
        for rel in remove:
            self.run('rm', '-q', '--', rel)
        if add:
            self.run('annex', 'add', '-q', '--', *add)
        if plain:
            self.run('add', '--', *plain)
        self.run('commit', '-q', '-m', msg)
        return self.rev('HEAD')

    # ---- reading history
    def tree(self, commit):
        """path -> annex key of the annexed files in commit's tree."""
        if commit not in self.trees:
            entries = []
            for item in self.run('ls-tree', '-r', '-z', '--full-tree', commit).split('\0'):
                if item:
                    meta, path = item.split('\t', 1)
                    entries.append((meta.split()[2], path))
            need = sorted({b for b, _ in entries if b not in self.blobs})
            if need:
                out = subprocess.run(['git', 'cat-file', '--batch'], cwd=self.root, capture_output=True,
                                     input=''.join(b + '\n' for b in need).encode()).stdout
                i = 0
                for b in need:
                    header_end = out.index(b'\n', i)
                    size = int(out[i:header_end].split()[2])
                    data = out[header_end + 1:header_end + 1 + size].decode(errors='replace')
                    i = header_end + 1 + size + 1
                    self.blobs[b] = data.strip().rsplit('/', 1)[1] if 'annex/objects/' in data else None
            self.trees[commit] = {p: self.blobs[b] for b, p in entries if self.blobs[b]}
        return self.trees[commit]

    def proposals(self):
        out = self.run('for-each-ref', '--format=%(objectname)', 'refs/pigeon/proposals/')
        return out.split()

    def references(self, with_proposals=True):
        """(commit, path, key, in_main) for every commit that introduces key at path."""
        tips = [self.head()] + (self.proposals() if with_proposals else [])
        main = set(self.run('rev-list', self.head()).split())
        refs = []
        for line in self.run('rev-list', '--parents', *tips).splitlines():
            c, *parents = line.split()
            mine, theirs = self.tree(c), [self.tree(q) for q in parents]
            for p, k in mine.items():
                if all(t.get(p) != k for t in theirs):
                    refs.append((c, p, k, c in main))
        return refs

    def file_at(self, path, commit=None):
        r = subprocess.run(['git', 'show', f'{commit or "refs/heads/main"}:{path}'], cwd=self.root,
                           capture_output=True)
        return r.stdout if r.returncode == 0 else None

    def decisions(self):
        """The active, valid purge decisions of main (ADR 0042, spec 08 §8.6)."""
        roster = tomllib.loads(self.file_at('.pigeon/roster.toml').decode())
        out = []
        for item in self.run('ls-tree', '--name-only', 'refs/heads/main', '.pigeon/decisions/').split():
            if not os.path.basename(item).startswith('purge-'):
                continue
            d = tomllib.loads(self.file_at(item).decode())
            added = self.run('log', '-1', '--format=%cI', '--diff-filter=A', 'refs/heads/main', '--', item)
            d['id'] = os.path.basename(item)[:-5]
            d['committed'] = datetime.fromisoformat(added.strip())
            d['valid'] = d['retention_end'] >= d['committed'] + timedelta(days=roster['retention_days'])
            out.append(d)
        return out

    def seals(self):
        """custodian -> {decision id: seal}, from the status refs this device knows."""
        out = {}
        for c in CUSTODIANS:
            data = self.file_at('status.toml', f'refs/pigeon/status/{c}')
            out[c] = {s['decision']: s for s in tomllib.loads(data.decode()).get('purge', [])} if data else {}
        return out

    # ---- the released set
    def covers(self, d, ref, main_ancestors):
        c, p, k, in_main = ref
        if not any(p == x or (x.endswith('/') and p.startswith(x)) for x in d['paths']):
            return False
        if d.get('keys') and k not in d['keys']:
            return False
        if in_main:
            return c in main_ancestors(d['base'])
        named = d.get('proposals', [])
        return any(self.is_ancestor(c, tip) for tip in named if self.rev(tip))

    def released(self, decisions=None, seals=None, with_proposals=True):
        decisions = [d for d in (decisions or self.decisions()) if d['valid']]
        seals = self.seals() if seals is None else seals
        main, cache = self.head(), {}

        def ancestors(base):
            if base not in cache:
                cache[base] = set(self.run('rev-list', base).split()) if self.rev(base) else set()
            return cache[base]
        sealed = [d for d in decisions
                  if all(d['id'] in seals[c] and self.is_ancestor(seals[c][d['id']]['main'], main)
                         for c in CUSTODIANS)]
        held = {k for d in sealed for c in CUSTODIANS for k in seals[c][d['id']].get('held', [])}
        by_key = {}
        for ref in self.references(with_proposals):
            by_key.setdefault(ref[2], []).append(ref)
        current = set(self.tree(main).values())
        out = {k for k, refs in by_key.items()
               if all(any(self.covers(d, r, ancestors) for d in sealed) for r in refs)}
        check(not out & current, f'{self.name}: a current key is released')
        return out - held - current

    def keys(self, with_proposals=True):
        """keys() of spec 05 §5.1: keys with a reference, minus the released ones."""
        return {r[2] for r in self.references(with_proposals)} - self.released(with_proposals=with_proposals)

    # ---- custody
    def present(self, key):
        return self.ok('annex', 'contentlocation', key)

    def wants(self, key, expr=WANTED):
        return self.ok('annex', 'matchexpression', f'--key={key}', expr)

    def place(self):
        """Per-key downloads of a custodian (spec 08 §8.3): keys() it lacks and wants."""
        got = []
        for k in sorted(self.keys()):
            if not self.present(k) and self.wants(k) and self.ok('annex', 'get', '--key', k):
                got.append(k)
        return got

    def drop_released(self):
        gone = []
        for k in sorted(self.released()):
            if self.present(k):
                self.run('annex', 'drop', '--force', '--key', k)
                gone.append(k)
        return gone

    # ---- exchanging history
    def fetch(self, remote, main=True, status=True, proposals=True):
        specs = ['+refs/heads/git-annex:refs/remotes/%s/git-annex' % remote]
        if main:
            specs.append('+refs/heads/main:refs/remotes/%s/main' % remote)
        if proposals:
            specs.append('+refs/pigeon/proposals/*:refs/pigeon/proposals/*')
        if status:
            specs.append('+refs/pigeon/status/*:refs/remotes/%s/status/*' % remote)
        self.run('fetch', '-q', remote, *specs)
        self.run('annex', 'merge', '-q')
        if status:
            for c in CUSTODIANS:
                theirs = self.rev(f'refs/remotes/{remote}/status/{c}')
                mine = self.rev(f'refs/pigeon/status/{c}')
                if c != self.name and theirs and (not mine or self.status_at(theirs) > self.status_at(mine)):
                    self.run('update-ref', f'refs/pigeon/status/{c}', theirs)
        if main:
            self.merge(self.rev(f'refs/remotes/{remote}/main'))

    def status_at(self, commit):
        return tomllib.loads(self.file_at('status.toml', commit).decode())['at']

    def merge(self, other):
        cur = self.head()
        if self.is_ancestor(other, cur):
            return
        if not self.bare:
            self.run('merge', '-q', '--no-edit', other)
        elif self.is_ancestor(cur, other):
            self.run('update-ref', 'refs/heads/main', other)
        else:
            raise RuntimeError(f'{self.name}: custodians only fast-forward in this lab')
        if cur != self.head() and self.decisions():
            self.note_seen()

    # ---- seals (custodians)
    def note_seen(self, now=None):
        path = os.path.join(self.pigeon_dir(), 'purges.toml')
        seen = tomllib.load(open(path, 'rb')).get('seen', {}) if os.path.exists(path) else {}
        for d in self.decisions():
            seen.setdefault(d['id'], now or datetime.now(timezone.utc))
        with open(path, 'w') as h:
            h.write('format = 1\n[seen]\n' + ''.join(f'"{k}" = {iso(v)}\n' for k, v in seen.items()))
        return seen

    def seal(self, now):
        """Seal every active decision whose waiting period ended on this clock."""
        seen, main = self.note_seen(), self.head()
        mine = self.seals()[self.name]
        refs = self.references()
        for d in self.decisions():
            if not d['valid'] or d['id'] in mine:
                continue
            if now < d['retention_end'] or now < seen[d['id']] + timedelta(days=RETENTION_DAYS):
                continue
            covered = {r[2] for r in refs if r[3] and self.covers(d, r, lambda b: set(
                self.run('rev-list', b).split()))}
            outside = {r[2] for r in refs if not r[3] and not any(
                self.covers(e, r, None) for e in self.decisions() if e['valid'])}
            mine[d['id']] = {'decision': d['id'], 'main': main, 'held': sorted(covered & outside)}
        body = f'format = 1\ndevice = "{self.name}"\nat = {iso(now)}\nmain = "{main}"\n'
        for s in mine.values():
            body += (f'\n[[purge]]\ndecision = "{s["decision"]}"\nmain = "{s["main"]}"\n'
                     f'held = [{", ".join(chr(34) + k + chr(34) for k in s["held"])}]\n')
        blob = self.run('hash-object', '-w', '--stdin', stdin=body).strip()
        tree = self.run('mktree', stdin=f'100644 blob {blob}\tstatus.toml\n').strip()
        commit = self.run('commit-tree', tree, '-m', f'status of {self.name}').strip()
        self.run('update-ref', f'refs/pigeon/status/{self.name}', commit)
        return sorted(mine)


def setup(lab):
    L = Repo(os.path.join(lab, 'L'), 'L')
    os.makedirs(L.root)
    L.run('init', '-q', '-b', 'main')
    L.run('annex', 'init', '-q', 'L')
    L.run('annex', 'numcopies', '2')
    L.run('annex', 'mincopies', '2')
    L.write('.pigeon/roster.toml', f'format = 1\nk = 2\nretention_days = {RETENTION_DAYS}\n'.encode())
    return L


def decision(paths, base, end, keys=(), proposals=()):
    q = lambda xs: '[' + ', '.join(f'"{x}"' for x in xs) + ']'
    return (f'format = 1\nkind = "purge"\npaths = {q(paths)}\nbase = "{base}"\nkeys = {q(keys)}\n'
            f'proposals = {q(proposals)}\nrequested_by = "ines"\nretention_end = {iso(end)}\n').encode()


def main():
    lab = os.environ.get('LAB', os.path.expanduser('~/ga-lab14'))
    if os.path.exists(lab):
        subprocess.run(['chmod', '-R', 'u+w', lab])
        shutil.rmtree(lab)
    os.makedirs(lab)
    X, V, U, Y1, Y2, Z, W = (content(n) for n in ('X', 'V', 'U', 'Y1', 'Y2', 'Z', 'W'))
    L = setup(lab)
    for rel, data in [('Photos/a.jpg', X), ('Shared/a-copy.jpg', X), ('Photos/b.jpg', V),
                      ('Photos/c.jpg', U), ('Docs/x.txt', Y1), ('Old/only.bin', Z),
                      ('Recettes/gateau.docx', W)]:
        L.write(rel, data)
    L.commit('A1', add=['Photos', 'Shared', 'Docs', 'Old', 'Recettes'], plain=['.pigeon'])
    L.write('Docs/x.txt', Y2)
    L.write('Docs/y.txt', Y1)
    a2 = L.commit('A2: edit x.txt, y.txt shares its old key, delete Old/only.bin',
                  add=['Docs'], remove=['Old/only.bin'])
    key = {n: L.tree(L.rev('HEAD~1'))[p] for n, p in [('X', 'Photos/a.jpg'), ('V', 'Photos/b.jpg'),
           ('U', 'Photos/c.jpg'), ('Y1', 'Docs/x.txt'), ('Z', 'Old/only.bin'),
           ('W', 'Recettes/gateau.docx')]}
    key['Y2'] = L.tree(a2)['Docs/x.txt']
    name = {v: k for k, v in key.items()}
    names = lambda ks: sorted(name[k] for k in ks)

    repos = {'L': L}
    for c in CUSTODIANS:
        r = Repo(os.path.join(lab, c), c, bare=True)
        subprocess.run(['git', 'clone', '-q', '--bare', L.root, r.root], check=True)
        r.run('annex', 'init', '-q', c)
        r.run('annex', 'group', '.', 'custodian')
        r.run('annex', 'wanted', '.', WANTED)
        r.run('annex', 'maxsize', 'here', '1000000000')
        repos[c] = r
    L2 = Repo(os.path.join(lab, 'L2'), 'L2')
    subprocess.run(['git', 'clone', '-q', L.root, L2.root], check=True)
    L2.run('annex', 'init', '-q', 'L2')
    repos['L2'] = L2
    for r in repos.values():
        r.run('remote', 'remove', 'origin', check=False)
        for other in repos.values():
            if other is not r:
                r.run('remote', 'add', other.name, other.root)
    for r in repos.values():
        for other in ('L', 'C1', 'C2', 'L2'):
            if other != r.name:
                r.fetch(other)

    # L2 suggests tarte.docx with the content of the accepted gateau.docx
    L2.run('annex', 'get', '-q', '--key', key['W'])
    L2.run('checkout', '-q', '-b', 'suggestion')
    L2.write('Recettes/tarte.docx', W)
    P = L2.commit('suggestion: tarte.docx', add=['Recettes'])
    L2.run('checkout', '-q', 'main')
    L2.run('update-ref', f'refs/pigeon/proposals/L2/{P}', P)
    for c in CUSTODIANS:
        repos[c].fetch('L')
        repos[c].fetch('L2')
        got = repos[c].place()
        check(sorted(got) == sorted(key.values()), f'{c}: per-key placement fetched {names(got)}')
        check(all(repos[c].wants(k) for k in key.values()), f'{c}: {WANTED} does not want every key')
    print(f'placement: C1 and C2 each fetched {len(key)} keys by key, history and the suggestion included')

    # L2 keeps copies of V and Z, then goes offline and re-adds V in another folder
    for n in ('V', 'Z'):
        L2.run('annex', 'get', '-q', '--key', key[n])
    L2.write('Archive/b-again.jpg', V)
    L2.commit('offline: Archive/b-again.jpg', add=['Archive'])

    # the admin's laptop L requests the purges (deleting current paths in the same commit)
    t0 = datetime.now(timezone.utc).replace(microsecond=0)
    end = t0 + timedelta(days=RETENTION_DAYS, minutes=1)
    L.write('.pigeon/decisions/purge-photos.toml', decision(['Photos/'], a2, end))
    L.write('.pigeon/decisions/purge-xold.toml', decision(['Docs/x.txt'], a2, end, keys=[key['Y1']]))
    L.write('.pigeon/decisions/purge-gateau.toml', decision(['Recettes/gateau.docx'], a2, end))
    L.write('.pigeon/decisions/purge-old-bad.toml', decision(['Old/'], a2, t0 + timedelta(days=1)))
    a3 = L.commit('A3: erase Photos/, old x.txt, gateau.docx', plain=['.pigeon'],
                  remove=['Photos/a.jpg', 'Photos/b.jpg', 'Photos/c.jpg', 'Recettes/gateau.docx'])
    check({d['id']: d['valid'] for d in L.decisions()}['purge-old-bad'] is False,
          'a decision with a 1-day retention is valid')
    L.write('.pigeon/decisions/purge-old.toml', decision(['Old/'], a3, end))
    L.commit('A4: erase Old/', plain=['.pigeon'])
    for c in CUSTODIANS:
        repos[c].fetch('L')
    L.write('Photos/c.jpg', U)                   # a restore during the waiting period
    L.commit('A5: restore Photos/c.jpg', add=['Photos'])
    repos['C2'].fetch('L')                       # only C2 receives the restore before sealing

    for c in CUSTODIANS:
        early = repos[c].seal(t0 + timedelta(days=RETENTION_DAYS - 1))
        check(early == [], f'{c}: sealed before the waiting period ended: {early}')
    sealed = {c: repos[c].seal(end + timedelta(hours=1)) for c in CUSTODIANS}
    check(all(s == ['purge-gateau', 'purge-old', 'purge-photos', 'purge-xold'] for s in sealed.values()),
          f'seals {sealed}')
    C1, C2 = repos['C1'], repos['C2']
    C1.fetch('C2', main=False)
    r_c1 = C1.released()
    check(r_c1 == set(), f'C1 released {names(r_c1)} before merging the main C2 sealed')
    trusting = {c: {d: dict(s, main=C1.head()) for d, s in C1.seals()[c].items()} for c in CUSTODIANS}
    lost = names(C1.released(seals=trusting))
    check('U' in lost, f'trusting seals without their main, C1 would release {lost}, not the restored U')
    L.fetch('C1', main=False, proposals=False)
    L.fetch('C2', main=False, proposals=False)
    check(L.proposals() == [], 'L knows the suggestion; held is untested')
    r_l = L.released()
    check(names(r_l) == ['V', 'Z'], f'L released {names(r_l)}, expected V, Z')
    no_held = {c: {d: dict(s, held=[]) for d, s in L.seals()[c].items()} for c in CUSTODIANS}
    check(key['W'] in L.released(seals=no_held),
          'without held, L (unaware of the suggestion) would keep W anyway')
    C1.fetch('C2')
    for r in (C1, C2):
        if r is C2:
            C2.fetch('C1', main=False)
        got = r.released()
        check(names(got) == ['V', 'Z'], f'{r.name} released {names(got)}, expected V, Z')
    named = [dict(d, paths=['Recettes/'], proposals=[P]) if d['id'] == 'purge-gateau' else d
             for d in C1.decisions()]
    nheld = {c: {d: dict(s, held=[]) for d, s in C1.seals()[c].items()} for c in CUSTODIANS}
    check(key['W'] in C1.released(decisions=named, seals=nheld),
          'naming the suggestion does not release W')
    print(f'released: V (Photos/b.jpg only) and Z (Old/ only) among {len(L.references())} references; '
          'kept: X (Shared/a-copy.jpg), Y1 (Docs/y.txt), W (suggestion, held by the seals), '
          f'U (restored before C2 sealed; C1 trusting seals alone would release {lost})')

    # drops, and no re-fetch although L2 (offline) still holds V and Z
    check(C1.wants(key['V']), 'git-annex alone no longer wants V: the per-key filter is untested')
    for r in (C1, C2, L):
        gone = r.drop_released()
        check(names(gone) == ['V', 'Z'], f'{r.name} dropped {names(gone)}')
        check(r.place() == [] if r.bare else True, f'{r.name} fetched a released key again')
    holders = {n: [r.name for r in repos.values() if r.present(key[n])] for n in ('V', 'Z')}
    check(holders == {'V': ['L2'], 'Z': ['L2']}, f'holders after the drops: {holders}')

    # L2 returns: it merges, drops Z, keeps V (a new reference), and custodians fetch V again
    L2.fetch('L')
    L2.fetch('C1', main=False)
    L2.fetch('C2', main=False)
    r_l2 = L2.released()
    check(names(r_l2) == ['Z'], f'L2 released {names(r_l2)}, expected Z')
    check(names(L2.drop_released()) == ['Z'], 'L2 did not drop Z')
    regen = {}
    for r in (C1, C2):
        r.fetch('L2')
        check(names(r.released()) == ['Z'], f'{r.name} released {names(r.released())} after L2 returned')
        regen[r.name] = names(r.place())
    check(regen == {'C1': ['V'], 'C2': ['V']}, f'new generation fetched {regen}')
    holders = {n: sorted(r.name for r in repos.values() if r.present(key[n])) for n in ('V', 'Z')}
    check(holders == {'V': ['C1', 'C2', 'L2'], 'Z': []}, f'holders at the end: {holders}')
    back = [n for n in ('V', 'Z') if C1.present(key[n])]
    check(back == ['V'], f'came back: {back}')
    print(f'returning laptop: dropped Z; {back} re-added offline came back to C1 and C2, a new retention '
          'generation ("Erased content came back"); Z has no copy left')

    contracts(os.path.join(lab, 'contracts'))
    print('FAILED' if FAILS else 'ok: E14 passed')
    sys.exit(1 if FAILS else 0)


def contracts(root):
    """git-annex behaviours that ADR 0042 relies on or avoids."""
    os.makedirs(root)
    A = Repo(os.path.join(root, 'A'), 'A')
    os.makedirs(A.root)
    A.run('init', '-q', '-b', 'main')
    A.run('annex', 'init', '-q', 'A')
    for i in range(8):
        A.write(f'f{i}', content(f'f{i}', 1000 + 100 * i))
    A.commit('files', add=[f'f{i}' for i in range(8)])
    for i in range(3):
        A.write(f'f{i}', content(f'f{i} v2', 1000 + 100 * i))
    A.commit('edits: history keys', add=[f'f{i}' for i in range(3)])
    all_keys = sorted({k for c in A.run('rev-list', 'main').split() for k in A.tree(c).values()})
    # balanced over two bare custodians: matchexpression against git-annex's own transfers
    picks = {}
    for c, expr in (('B1', 'balanced=custodian:1'), ('B2', 'balanced=custodian:1')):
        r = Repo(os.path.join(root, c), c, bare=True)
        subprocess.run(['git', 'clone', '-q', '--bare', A.root, r.root], check=True)
        r.run('annex', 'init', '-q', c)
        r.run('annex', 'group', '.', 'custodian')
        r.run('annex', 'wanted', '.', expr)
        picks[c] = r
    for r in picks.values():
        for o in picks.values():
            if o is not r:
                r.run('remote', 'add', o.name, o.root)
        r.run('fetch', '-q', '--all')
        r.run('annex', 'merge', '-q')
    agree, split = 0, []
    for r in picks.values():
        verdict = {k for k in all_keys if r.wants(k, 'balanced=custodian:1')}
        r.run('annex', 'satisfy', '--all', 'origin', check=False)
        got = {k for k in all_keys if r.present(k)}
        check(verdict == got, f'{r.name}: matchexpression {len(verdict)} keys, satisfy fetched {len(got)}')
        agree += len(verdict)
        split.append(len(verdict))
    check(agree == len(all_keys), f'balanced=custodian:1 split {agree} of {len(all_keys)} keys')
    # sizebalanced in a non-bare custodian: matchexpression against find --want-get
    N = Repo(os.path.join(root, 'N'), 'N')
    subprocess.run(['git', 'clone', '-q', A.root, N.root], check=True)
    N.run('annex', 'init', '-q', 'N')
    N.run('annex', 'group', '.', 'custodian')
    N.run('annex', 'wanted', '.', 'sizebalanced=custodian:1')
    for o in picks.values():
        N.run('remote', 'add', o.name, o.root)
    N.run('fetch', '-q', '--all')
    N.run('annex', 'merge', '-q')
    sb = []
    for step in ('before', 'after'):
        files = N.run('annex', 'find', '--want-get', '--format=${key}\\n').split()
        sb.append(len(files))
        verdict = [k for k in sorted(set(N.tree('HEAD').values())) if N.wants(k, 'sizebalanced=custodian:1')]
        check(sorted(files) == verdict, f'N {step}: find --want-get {len(files)}, matchexpression {len(verdict)}')
        N.run('annex', 'get', '-q', 'f3', 'f4')
    # refusals
    for args in (('get', '--auto', '--all'), ('drop', '--auto', '--key', all_keys[0])):
        r = subprocess.run(['git', 'annex', *args], cwd=N.root, capture_output=True, text=True)
        check(r.returncode != 0 and 'Cannot use --auto' in r.stderr, f'git annex {" ".join(args)} is accepted now')
    # satisfy --all drops from a remote that does not want the content
    S = Repo(os.path.join(root, 'S'), 'S')
    subprocess.run(['git', 'clone', '-q', A.root, S.root], check=True)
    S.run('annex', 'init', '-q', 'S')
    S.run('annex', 'wanted', '.', 'anything')
    S.run('annex', 'wanted', 'origin', 'nothing')
    S.run('annex', 'satisfy', '--all', check=False)
    left = [k for k in all_keys if A.present(k)]
    check(left == [], f'satisfy --all left {len(left)} of {len(all_keys)} keys on the remote')
    print(f'contracts: matchexpression agrees with satisfy (balanced, {len(all_keys)} keys incl. history, '
          f'split {split[0]} + {split[1]}) and '
          f'with find --want-get (sizebalanced, {sb[0]} then {sb[1]} keys); --auto refused with --all and --key; '
          f'satisfy --all dropped {len(all_keys) - len(left)} keys from a remote')


if __name__ == '__main__':
    main()
