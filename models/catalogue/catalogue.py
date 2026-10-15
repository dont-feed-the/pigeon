#!/usr/bin/env python3
"""Reference model of the logical catalogue and its merge (spec 05 §5.2).

catalogue(S) is a pure function of a set S of accepted commits, in three modes:
new is spec 05 §5.2 (contributions, conflict events, folded slots); old is spec 05
§5.2 before it (heads over every change, merge commits included); naive is the
obvious repair of old (heads over contributions only, copies derived).
For each case the checker runs every schedule of the contributions and of
pulls between three devices: orders, pairings, fast-forwards, duplicate merges
and deliveries, split and rejoin. new reaches every state (there are finitely
many); old and naive stop at OLD_PULLS pulls, enough to show how they fail.
Laws: order (the final catalogue depends only on which contribution saw which,
never on contact order or merge identities), tree (tree(c) = catalogue(anc*(c))
for every commit; a device shows the tree of its main), kept (a key leaves the
catalogue only through a contribution that had it), seen (no version that no
contribution saw comes back), names (portable, injective after folding),
algebra (merge idempotent, commutative, associative), authority (clean merges
change nothing; each copy is allowed to its author). new must pass them all,
and EXAMPLES; old and naive must fail exactly as EXPECTED says. --dump DIR
writes each case's commits (mode new) for pigeon-core's catalogue_oracle.rs.
Usage: catalogue.py [--mode M] [--case C] [--pulls N] [--dump DIR] [-v]
"""

import argparse
import hashlib
import json
import os
import re
import sys
import time
import unicodedata
from collections import defaultdict, deque

DAY = "2026-10-13"


def hid(*parts):
    return hashlib.sha1(repr(parts).encode()).hexdigest()[:16]


def fold(s):
    """Portable folding (spec 05 §5.4): NFC, then Unicode full case folding."""
    return unicodedata.normalize("NFC", unicodedata.normalize("NFC", s).casefold())


def unit(path):
    """The outermost package above path (spec 07 §7.9), or path itself."""
    parts = path.split("/")
    for i in range(len(parts) - 1):
        if parts[i].casefold().endswith(".pages"):
            return "/".join(parts[: i + 1])
    return path


def units(tree, key=fold):
    """key(unit) -> (unit, content); a package's content is the set of its files."""
    groups = defaultdict(dict)
    for path, k in tree.items():
        u = unit(path)
        groups[u][path[len(u) + 1:]] = k
    return {key(u): (u, f[""] if "" in f else frozenset(f.items())) for u, f in groups.items()}


def contents(tree):
    return {content for _, content in units(tree, key=str).values()}


def expand(entries):
    tree = {}
    for path, content in entries:
        if isinstance(content, frozenset):
            tree.update((f"{path}/{rel}", k) for rel, k in content)
        else:
            tree[path] = content
    return tree


def split_name(path):
    head, _, last = path.rpartition("/")
    dot = last.rfind(".")
    stem, ext = (last[:dot], last[dot:]) if dot > 0 else (last, "")
    return (head + "/" if head else ""), stem, ext


def conflict_name(path, who, when, n):
    """Spec 05 §5.3: dir/stem (Person, 2026-10-13 14h02).ext; ' 2', ' 3' inside."""
    head, stem, ext = split_name(path)
    count = f" {n}" if n > 1 else ""
    return f"{head}{stem} ({who}, {DAY} {when // 60:02d}h{when % 60:02d}{count}){ext}"


def copies_of(tree, path):
    """The units of tree named as conflict copies of path, compared after folding."""
    head, stem, ext = split_name(fold(path))
    pat = re.compile(re.escape(head + stem + " (") + r".*\)" + re.escape(ext) + "$")
    return sorted(u for u, _ in units(tree).values() if pat.match(fold(u)))


def clashes(tree):
    """Folded names that two spellings, or a file and a folder, share."""
    seen = defaultdict(set)
    for path in tree:
        parts = path.split("/")
        for i in range(len(parts)):
            prefix = "/".join(parts[: i + 1])
            seen[fold(prefix)].add((prefix, i == len(parts) - 1))
    return sorted(f for f, s in seen.items() if len(s) > 1)


def drop(tree, u):
    for p in [p for p in tree if p == u or p.startswith(u + "/")]:
        del tree[p]


# Folder rules (spec 04 §4.3): the root is add-only with the admins as stewards.
FOLDERS = {"team": ("stewards", {"Alice", "Bob", "Paul"}), "alice": ("owner", "Alice")}
ADMINS = {"Alice"}


def may(person, path, before):
    """Spec 04 §4.3 for one unit: an add if before is None, else a modify or delete."""
    parts = fold(path).split("/")
    kind, who = "stewards", ADMINS
    for i in range(len(parts) - 1, 0, -1):
        if "/".join(parts[:i]) in FOLDERS:
            kind, who = FOLDERS["/".join(parts[:i])]
            break
    if kind == "owner":
        return person == who
    return before is None or person in who


class Commit:
    __slots__ = ("id", "parents", "tree", "device", "person", "who", "when", "op", "resolve", "seq")

    def __init__(self, cid, parents, tree, device, person, who, when, op, resolve, seq):
        self.id, self.parents, self.tree, self.device = cid, parents, tree, device
        self.person, self.who, self.when, self.op = person, who, when, op
        self.resolve, self.seq = resolve, seq


class Event:
    """A contribution, or a conflict event; writes maps slot -> (value, origin)."""
    __slots__ = ("id", "past", "writes", "resolves", "conflict")

    def __init__(self, eid, past, writes, resolves=frozenset(), conflict=False):
        self.id, self.past, self.writes, self.resolves = eid, past, writes, resolves
        self.conflict = conflict


class New:
    """spec 05 §5.2: the catalogue of the contributions and of the conflict events they saw."""

    def __init__(self, run):
        self.run, self.events, self.info, self.memo = run, {}, {}, {}
        self.unauthorised = []

    def order(self, o):
        """Representative order: recorded time, device, the device's count, commit ID."""
        c = self.run.commits[o]
        return (c.when, c.device, c.seq, c.id)

    def judge(self, cid):
        """(accepted, contribution, state, base tree) of a commit (spec 04 §4.4)."""
        if cid not in self.info:
            c = self.run.commits[cid]
            if not c.parents:
                self.events[cid] = Event(cid, frozenset(), {x: (v, cid) for x, v in units(c.tree).items()})
                self.info[cid] = (True, True, frozenset([cid]), {})
            else:
                acc = [p for p in c.parents if self.judge(p)[0]]
                state = frozenset().union(*(self.judge(p)[2] for p in acc))
                self.info[cid] = self.extend(c, state) if acc else (False, False, state, {})
        return self.info[cid]

    def extend(self, c, state):
        tree, shown, unsettled, heads = self.present(state)
        old, new = units(tree), units(c.tree)
        delta = [x for x in sorted(old.keys() | new.keys()) if old.get(x) != new.get(x)]
        resolves = frozenset(fold(p) for p in c.resolve  # Keep both, from whoever may change p
                             if fold(p) in old and may(c.person, p, old[fold(p)]))
        if not delta and not resolves:
            return (True, False, state, tree)  # a clean merge: no change, any member signs it
        allowed = all(may(c.person, (new.get(x) or old.get(x))[0], old.get(x)) for x in delta)
        if clashes(c.tree) or not allowed:
            return (False, False, state, tree)  # a proposal (spec 04 §4.5)
        frozen = frozenset(self.freeze(x, heads[x], shown[x]) for x in unsettled)
        writes = {x: (new.get(x), c.id) for x in delta}
        self.events[c.id] = Event(c.id, state | frozen, writes, resolves)
        return (True, True, state | frozen | {c.id}, tree)

    def freeze(self, x, hs, shown):
        """The conflict event that keeps the versions of slot x where they were shown."""
        writes = {fold(path): ((path, v[1]), o) for v, path, o in shown}
        writes.setdefault(x, (None, None))
        eid = ("v", x, frozenset(e.id for e in hs), frozenset((y, w[0]) for y, w in writes.items()))
        if eid not in self.events:
            past = frozenset().union(*({e.id} | e.past for e in hs))
            self.events[eid] = Event(eid, past, writes, conflict=conflicted(shown))
            for value, o in writes.values():
                if value and not may(self.run.commits[o].person, value[0], None):
                    self.unauthorised.append((value[0], o))
        return eid

    def present(self, state):
        """Tree of the catalogue of a state, where each version is shown, unsettled slots."""
        if state in self.memo:
            return self.memo[state]
        writers = defaultdict(list)
        for eid in state:
            for x in self.events[eid].writes:
                writers[x].append(self.events[eid])
        heads = {x: [e for e in ws if not any(e.id in w.past for w in ws)] for x, ws in writers.items()}
        versions = []
        for x, hs in heads.items():
            best = {}  # one version per content: names that differ only in case agree
            for e in hs:
                v, o = e.writes[x]
                if v is not None and (v[1] not in best or self.order(o) < self.order(best[v[1]][1])):
                    best[v[1]] = (v, o)
            versions += [(self.order(o), x, repr(v), v, o) for v, o in best.values()]
        versions.sort()
        spell, folders = {}, set()
        for _, x, _, (path, content), _ in versions:
            parts = path.split("/")
            for i in range(len(parts) - 1):
                spell.setdefault(fold("/".join(parts[: i + 1])), parts[i])
            if isinstance(content, frozenset):
                folders.add(x)
        folders |= set(spell)

        def respell(path):
            parts = path.split("/")
            folders_ = [spell[fold("/".join(parts[: i + 1]))] for i in range(len(parts) - 1)]
            return "/".join(folders_ + parts[-1:])

        # The versions a conflict event placed at one of their slot's copies (step 5).
        copied = {(eid[1], o) for eid in state if isinstance(eid, tuple)
                  for y, (v, o) in self.events[eid].writes.items() if y != eid[1] and v}
        placed, shown, later = set(), defaultdict(list), []
        for _, x, _, v, o in versions:
            path = respell(v[0])
            if x in placed or (x in folders and not isinstance(v[1], frozenset)):
                later.append((x, v, o, path))
            else:
                placed.add(x)
                shown[x].append((v, path, o))
        for x, v, o, path in later:
            if (x, o) in copied:
                continue  # shown at the copy a conflict event gave it
            c, n = self.run.commits[o], 1
            while fold(conflict_name(path, c.who, c.when, n)) in placed | folders:
                n += 1
            placed.add(fold(conflict_name(path, c.who, c.when, n)))
            shown[x].append((v, conflict_name(path, c.who, c.when, n), o))
        unsettled = sorted(x for x, s in shown.items() if len(s) > 1 or s[0][1] != s[0][0][0])  # moved
        tree = expand((path, v[1]) for s in shown.values() for v, path, _ in s)
        self.memo[state] = (tree, shown, unsettled, heads)
        return self.memo[state]

    def accepted(self, cid):
        return self.judge(cid)[0]

    def contribution(self, cid):
        return self.judge(cid)[1]

    def cat(self, cids):
        return self.present(frozenset().union(*(self.judge(c)[2] for c in cids if self.judge(c)[0])))[0]

    def base(self, cid):
        return self.judge(cid)[3]

    def notices(self, cids):
        """Open conflicts: not yet seen by a contribution, or seen and not yet resolved."""
        state = frozenset().union(*(self.judge(c)[2] for c in cids if self.judge(c)[0]))
        shown = self.present(state)[1]
        out = {x for x in self.present(state)[2] if conflicted(shown[x])}
        for eid in state:
            ev = self.events[eid]
            if ev.conflict:
                if not any(eid in e.past and (e.resolves & {eid[1]} or e.writes.keys() & ev.writes.keys())
                           for e in map(self.events.get, state) if not isinstance(e.id, tuple)):
                    out.add(eid[1])
        return out


def conflicted(shown):
    """Several versions, or one under another name: a conflict people are told of."""
    return len(shown) > 1 or shown[0][1].rpartition("/")[2] != shown[0][0][0].rpartition("/")[2]


class Old:
    """Spec 05 §5.2 before spec 05 §5.2; with naive, merge commits are not changes."""

    def __init__(self, run, naive=False):
        self.run, self.naive, self.ok, self.memo, self.acc = run, naive, {}, {}, {}
        self.unauthorised = []

    def judge(self, cid):
        if cid not in self.ok:
            c = self.run.commits[cid]
            acc = [p for p in c.parents if self.judge(p)]
            if not c.parents:
                self.ok[cid] = True
            elif not acc:
                self.ok[cid] = False
            else:
                base, new = units(self.base(cid), key=str), units(c.tree, key=str)
                delta = [u for u in base.keys() | new.keys() if base.get(u) != new.get(u)]
                self.ok[cid] = all(may(c.person, u, base.get(u)) for u in delta)
        return self.ok[cid]

    accepted = judge

    def closure(self, cid):
        if cid not in self.acc:
            c = self.run.commits[cid]
            s = frozenset().union(*(self.closure(p) for p in c.parents if self.judge(p)))
            self.acc[cid] = s | {cid} if self.judge(cid) else s
        return self.acc[cid]

    def contribution(self, cid):
        c = self.run.commits[cid]
        return self.judge(cid) and (c.op is not None or not c.parents)

    def cat(self, cids):
        return self.catalogue(frozenset().union(*(self.closure(c) for c in cids if self.judge(c))))

    def base(self, cid):
        c = self.run.commits[cid]
        return self.catalogue(frozenset().union(*(self.closure(p) for p in c.parents if self.judge(p))))

    def catalogue(self, S):
        if S in self.memo:
            return self.memo[S]
        commits, val = self.run.commits, self.run.exact
        changes = defaultdict(list)
        for c in S:
            cm = commits[c]
            if self.naive and cm.op is None and cm.parents:
                continue
            pv = [val(p) for p in cm.parents]
            for u in set(val(c)).union(*pv):
                if not pv or any(val(c).get(u) != q.get(u) for q in pv):
                    changes[u].append(c)
        out, rest = {}, []
        for u in sorted(changes):
            ch = changes[u]
            hs = [c for c in ch if not any(c != d and c in self.run.anc(d) for d in ch)]
            vs = sorted({val(h).get(u) for h in hs} - {None}, key=lambda v: self.rep(ch, u, v))
            if vs:
                out[u] = vs[0][1]
                rest += [(u, v) for v in vs[1:]]
        for u, v in rest:
            c, n = commits[self.rep(changes[u], u, v)[-1]], 1
            while conflict_name(u, c.who, c.when, n) in out:
                n += 1
            out[conflict_name(u, c.who, c.when, n)] = v[1]
        self.memo[S] = expand(out.items())
        return self.memo[S]

    def rep(self, ch, u, v):
        """The earliest non-merge commit that introduced v at u (any commit if none)."""
        commits = self.run.commits
        cands = [c for c in ch if self.run.exact(c).get(u) == v]
        plain = [c for c in cands if len(commits[c].parents) <= 1] or cands
        return min((commits[c].when, commits[c].device, c) for c in plain)


class Op:
    def __init__(self, device, when, kind, *args, needs=(), conflict=None):
        self.device, self.when, self.kind, self.args = device, when, kind, args
        self.needs, self.conflict = set(needs), conflict


class Case:
    def __init__(self, name, devices, genesis, ops, shows):
        self.name, self.devices, self.genesis, self.ops, self.shows = name, devices, genesis, ops, shows
        self.names = [d for d, _ in devices]
        self.person = dict(devices)


class Run:
    """One case in one mode: the commit graph that every schedule shares."""

    def __init__(self, case, mode):
        self.case, self.mode, self.commits, self.ancs, self.vals = case, mode, {}, {}, {}
        self.commits["g"] = Commit("g", (), dict(case.genesis), "G", "Alice", "Alice", 0, None, (), 0)
        self.engine = New(self) if mode == "new" else Old(self, naive=mode == "naive")

    def anc(self, cid):
        if cid not in self.ancs:
            self.ancs[cid] = frozenset([cid]).union(*(self.anc(p) for p in self.commits[cid].parents))
        return self.ancs[cid]

    def exact(self, cid):
        if cid not in self.vals:
            self.vals[cid] = units(self.commits[cid].tree, key=str)
        return self.vals[cid]

    def commit(self, cid, parents, tree, device, person, who, when, op=None, resolve=()):
        if cid not in self.commits:
            past = frozenset().union(*(self.anc(p) for p in parents))
            seq = sum(1 for a in past if self.commits[a].device == device and self.commits[a].op)
            self.commits[cid] = Commit(cid, parents, tree, device, person, who, when, op, resolve,
                                       seq + (1 if op else 0))
        return cid

    def merge(self, device, a, b):
        """A merge commit whose tree is the catalogue of its parents (spec 05 §5.2)."""
        person = self.case.person.get(device, "Alice")
        tree = self.engine.cat((a, b))
        return self.commit(hid("merge", device, a, b), (a, b), tree, device, person, person, 0)

    def pull(self, st, i, j):
        """Device i fetches everything j has, then fast-forwards, merges or records j's tip."""
        devs, done = st
        (si, ti), (sj, tj) = devs[i], devs[j]
        s, t = si | sj, ti
        if tj in self.anc(ti):
            pass
        elif ti in self.anc(tj):
            t = tj
        elif any(self.engine.contribution(c) for c in self.anc(tj) - self.anc(ti)):
            t = self.merge(self.case.names[i], ti, tj)
            s = s | {t}
        return (devs[:i] + ((s, t),) + devs[i + 1:], done)

    def enabled(self, st, name):
        devs, done = st
        op = self.case.ops[name]
        if any(n == name for n, _ in done):
            return False
        s, t = devs[self.case.names.index(op.device)]
        if not op.needs <= {n for n, c in done if c in s}:
            return False
        if op.conflict and not copies_of(self.commits[t].tree, op.conflict):
            return False
        return not clashes(self.edit(st, name)[0])  # else capture holds the save back

    def act(self, st, name):
        devs, done = st
        i = self.case.names.index(self.case.ops[name].device)
        s, t = devs[i]
        view, parents, who, when, resolve = self.edit(st, name)
        op = self.case.ops[name]
        cid = self.commit(hid("op", name, parents), parents, view, op.device, self.case.person[op.device],
                          who, when, name, resolve)
        tip = t if op.kind == "propose" else cid
        return (devs[:i] + ((s | {cid}, tip),) + devs[i + 1:], done | {(name, cid)})

    def edit(self, st, name):
        """The tree that op name saves on its device's main, with its parents and label."""
        devs, done = st
        op = self.case.ops[name]
        s, t = devs[self.case.names.index(op.device)]
        view, parents = dict(self.commits[t].tree), (t,)
        person = self.case.person[op.device]
        who, when, resolve, a = person, op.when, (), op.args
        if op.kind in ("set", "propose"):
            for path, key in a:
                drop(view, path) if key is None else view.__setitem__(path, key)
        elif op.kind == "rename":
            view[a[1]] = view.pop(a[0])
        elif op.kind == "keep":  # Keep mine / Keep Paul's: one version, no copies
            for u in copies_of(view, a[0]) + [u for x, (u, _) in units(view).items() if x == fold(a[0])]:
                drop(view, u)
            view[a[0]] = a[1]
        elif op.kind == "keepboth":  # Keep both: a signed change that only resolves
            resolve = (a[0],)
        elif op.kind == "edit_copy":
            for u in copies_of(view, a[0]):
                if view.get(u) == a[1]:
                    view[u] = a[2]
        elif op.kind == "adopt":  # the decision merge of spec 04 §4.5
            pc = next(c for n, c in done if n == a[0])
            p = self.commits[pc]
            parents, who, when = (t, pc), p.who, p.when
            before = self.commits[p.parents[0]].tree
            for path in sorted(before.keys() | p.tree.keys()):
                if before.get(path) == p.tree.get(path):
                    continue
                if view.get(path) == before.get(path):
                    drop(view, path) if p.tree.get(path) is None else view.__setitem__(path, p.tree[path])
                elif p.tree.get(path) is not None:  # changed meanwhile: keep both
                    n = 1
                    while conflict_name(path, who, when, n) in view:
                        n += 1
                    view[conflict_name(path, who, when, n)] = p.tree[path]
        return view, parents, who, when, resolve


class Checker:
    """Every schedule of one case in one mode, and the laws on every state reached."""

    def __init__(self, case, mode, pulls):
        self.case, self.run, self.pulls = case, Run(case, mode), pulls
        self.eng = self.run.engine
        self.fails, self.finals, self.memo, self.checked = {}, {}, {}, set()
        self.pred, self.states, self.schedules = {}, 0, 0

    def fail(self, law, st, text):
        if law not in self.fails:
            self.fails[law] = f"{text}\n    schedule: {self.schedule(st)}"

    def schedule(self, st):
        steps = []
        while self.pred.get(st):
            st, step = self.pred[st]
            steps.append(step)
        return " ".join(reversed(steps)) or "(start)"

    def explore(self):
        n = len(self.case.names)
        start = (tuple((frozenset(["g"]), "g") for _ in range(n)), frozenset())
        dist, seen, queue = {start: 0}, set(), deque([start])
        self.pred[start] = None
        while queue:
            st = queue.popleft()
            if st in seen:
                continue
            seen.add(st)
            self.check(st)
            k = dist[st]
            moves = [(self.run.act(st, name), name, 0) for name in sorted(self.case.ops)
                     if self.run.enabled(st, name)]
            if self.pulls is None or k < self.pulls:
                moves += [(self.run.pull(st, i, j), f"{a}<{b}", 1) for i, a in enumerate(self.case.names)
                          for j, b in enumerate(self.case.names) if i != j]
            for nxt, step, cost in moves:
                if nxt != st and (nxt not in dist or dist[nxt] > k + cost):
                    dist[nxt], self.pred[nxt] = k + cost, (st, step)
                    queue.appendleft(nxt) if cost == 0 else queue.append(nxt)
        self.states = len(seen)
        for value, o in self.eng.unauthorised:
            self.fail("authority", None, f"copy {value} is not allowed to {self.run.commits[o].person}")

    def check(self, st):
        self.check_state(st)
        if len(st[1]) == len(self.case.ops):
            self.final(st)

    def check_state(self, st):
        devs, done = st
        for s, _ in devs:
            for cid in s - self.checked:
                self.checked.add(cid)
                self.check_commit(st, cid)
        for s, t in devs:
            if (s, t) not in self.memo:
                self.memo[s, t] = True
                self.check_device(st, s, t)

    def check_commit(self, st, cid):
        run, eng, c = self.run, self.eng, self.run.commits[cid]
        want = not (c.op and self.case.ops[c.op].kind == "propose")
        accepted = eng.accepted(cid)
        if accepted != want:
            self.fail("authority", st, f"commit {c.op or 'merge'} accepted={accepted}, expected {want}")
        if c.op is None and c.parents and eng.contribution(cid):
            self.fail("authority", st, f"merge by {c.device} changes the catalogue")
        if accepted and eng.cat([cid]) != c.tree:
            self.fail("tree", st, f"tree({c.op or 'merge by ' + c.device}) = {show(c.tree)}\n"
                      f"    catalogue(anc*) = {show(eng.cat([cid]))}")

    def check_device(self, st, s, t):
        run, eng = self.run, self.eng
        cat = eng.cat(s)
        if cat != run.commits[t].tree:
            self.fail("tree", st, f"a device shows {show(cat)}\n"
                      f"    but its main is {show(run.commits[t].tree)}")
        if clashes(cat):
            self.fail("names", st, f"{show(cat)} is not portable: {clashes(cat)}")
        contribs = [c for c in s if eng.contribution(c)]
        cur = contents(cat)
        removed = set()
        for e in contribs:
            removed |= contents(eng.base(e)) - contents(run.commits[e].tree)
        for c in contribs:
            for k in contents(run.commits[c].tree) - contents(eng.base(c)) - cur - removed:
                self.fail("kept", st, f"{k} of {run.commits[c].op or 'genesis'} left {show(cat)}")
        tops = [c for c in contribs if not any(c != d and c in run.anc(d) for d in contribs)]
        seen = set().union(*(contents(run.commits[f].tree) for f in tops))
        for k in cur - seen:
            ends = sorted(run.commits[f].op or "genesis" for f in tops)
            self.fail("seen", st, f"{k} is in {show(cat)} but in no tree of {ends}")

    def final(self, st):
        run = self.run
        self.schedules += 1
        devs, done = st
        end = st
        for j in range(1, len(devs)):
            end = run.pull(end, 0, j)
        for j in range(1, len(devs)):
            end = run.pull(end, j, 0)
        self.pred.setdefault(end, (st, "sync"))
        self.check_state(end)
        trees = [run.commits[t].tree for _, t in end[0]]
        if any(tr != trees[0] for tr in trees):
            self.fail("order", st, f"devices differ after a full sync: {[show(t) for t in trees]}")
        key = frozenset((n, frozenset(m for m, d in done if d != c and d in run.anc(c))) for n, c in done)
        if key in self.finals and self.finals[key][0] != trees[0]:
            other = self.finals[key][1]
            self.fail("order", st, f"same contributions, same views, two catalogues:\n"
                      f"    {show(trees[0])}\n    {show(self.finals[key][0])}\n"
                      f"    other schedule: {self.schedule(other)}")
        self.finals.setdefault(key, (trees[0], st))
        tips = [t for _, t in devs]
        if len(set(tips)) == 3:
            self.algebra(st, *tips)

    def algebra(self, st, a, b, c):
        run = self.run
        tree = lambda x: run.commits[x].tree
        ab, ba = run.merge("H", a, b), run.merge("H", b, a)
        if tree(ab) != tree(ba):
            self.fail("algebra", st, f"merge(a, b) = {show(tree(ab))} but merge(b, a) = {show(tree(ba))}")
        left, right = run.merge("H", ab, c), run.merge("H", a, run.merge("H", b, c))
        if tree(left) != tree(right):
            self.fail("algebra", st, f"merge(merge(a, b), c) = {show(tree(left))}\n"
                      f"    merge(a, merge(b, c)) = {show(tree(right))}")
        for again in (run.merge("H", ab, ba), run.merge("H", ab, a)):
            if tree(again) != tree(ab):
                self.fail("algebra", st, f"merging merge(a, b) again gives {show(tree(again))},"
                          f" not {show(tree(ab))}")


def show(tree):
    return "{" + ", ".join(f"{p}: {k}" for p, k in sorted(tree.items())) + "}"


def t(h, m):
    return 60 * h + m


TEAM = (("X", "Alice"), ("Y", "Bob"), ("Z", "Paul"))
ALICE2 = (("X", "Alice"), ("Y", "Alice"), ("Z", "Bob"))
A, B, P = "team/a.txt", "team/b.txt", "team/R.pages"
CASES = [
    Case("add-add", TEAM, {}, {
        "A": Op("X", t(14, 1), "set", (A, "kA")),
        "B": Op("Y", t(14, 2), "set", (A, "kB")),
        "C": Op("Z", t(14, 2), "set", (A, "kC"))},
        "three adds of one name: the first keeps it, the others get copies"),
    Case("edit-edit", ALICE2, {A: "k0"}, {
        "A": Op("X", t(14, 2), "set", (A, "kA")),
        "B": Op("Y", t(14, 2), "set", (A, "kB")),
        "C": Op("Z", t(14, 1), "set", (A, "kC"))},
        "three edits; Alice's two copies of one minute are told apart by ' 2'"),
    Case("edit-delete", TEAM, {A: "k0", B: "k1"}, {
        "A": Op("X", t(14, 1), "set", (A, "kA")),
        "B": Op("Y", t(14, 2), "set", (A, None), (B, "kB")),
        "C": Op("Z", t(14, 3), "set", (B, None))},
        "on two paths, a modification survives a concurrent deletion"),
    Case("rename-edit", TEAM, {A: "k0"}, {
        "A": Op("X", t(14, 1), "rename", A, B),
        "B": Op("Y", t(14, 2), "set", (A, "kB")),
        "C": Op("Z", t(14, 3), "set", (B, "kC"))},
        "no rename tracking: the edit stays at the old name, the rename adds the new one"),
    Case("case", TEAM, {}, {
        "A": Op("X", t(14, 1), "set", ("team/Photo.jpg", "kA")),
        "B": Op("Y", t(14, 2), "set", ("team/photo.jpg", "kB")),
        "C": Op("Z", t(14, 3), "set", ("team/PHOTO.jpg", "kA"))},
        "names equal after folding are one slot; one content under two spellings is one version"),
    Case("unicode", TEAM, {}, {
        "A": Op("X", t(14, 1), "set", ("team/Straße/a.txt", "kA")),
        "B": Op("Y", t(14, 2), "set", ("team/STRASSE/b.txt", "kB")),
        "C": Op("Z", t(14, 3), "set", ("team/strasse", "kC"))},
        "full case folding: one folder under the first spelling; a file gives way to a folder"),
    Case("package", TEAM, {P + "/index.xml": "k0", P + "/data/p.bin": "k1"}, {
        "A": Op("X", t(14, 1), "set", (P + "/index.xml", "kA")),
        "B": Op("Y", t(14, 2), "set", (P + "/data/p.bin", "kB")),
        "C": Op("Z", t(14, 3), "set", (P, None))},
        "a package is one slot: two whole packages, never a mix; the deletion yields"),
    Case("supersede", TEAM, {A: "k0"}, {
        "A": Op("X", t(14, 1), "set", (A, "kA")),
        "B": Op("Y", t(14, 2), "set", (A, "kB")),
        "C": Op("Z", t(14, 3), "set", (A, "kC")),
        "D": Op("X", t(14, 4), "set", (A, "kD"), needs="AB")},
        "an edit made after seeing a conflict replaces the name and keeps the copies it saw"),
    Case("keep-mine", TEAM, {A: "k0"}, {
        "A": Op("X", t(14, 1), "set", (A, "kA")),
        "B": Op("Y", t(14, 2), "set", (A, "kB")),
        "K": Op("Y", t(14, 5), "keep", A, "kB", needs="AB", conflict=A),
        "E": Op("X", t(14, 6), "set", (A, "kE"), needs="AB")},
        "Keep mine races a new edit of the name"),
    Case("keep-both", TEAM, {A: "k0"}, {
        "A": Op("X", t(14, 1), "set", (A, "kA")),
        "B": Op("Y", t(14, 2), "set", (A, "kB")),
        "K": Op("X", t(14, 5), "keepboth", A, needs="AB", conflict=A),
        "E": Op("Y", t(14, 6), "edit_copy", A, "kB", "kE", needs="AB", conflict=A),
        "C": Op("Z", t(14, 7), "set", (A, "kC"), needs="AB")},
        "Keep both races an edit of the copy and an edit of the name"),
    Case("decision", (("X", "Alice"), ("Y", "Alice"), ("Z", "Paul")), {"alice/a.txt": "k0", B: "k1"}, {
        "P": Op("Z", t(14, 1), "propose", ("alice/a.txt", "kP")),
        "D": Op("X", t(14, 3), "adopt", "P", needs="P"),
        "E": Op("Y", t(14, 2), "set", ("alice/a.txt", "kE")),
        "F": Op("Z", t(14, 4), "set", (B, "kF"))},
        "Alice adopts Paul's suggestion while her phone edits the file"),
]

# Fixed histories (spec 05 §5.2 case table): steps, device X's tree, open conflicts.
C1, C2, C3 = (f" (Bob, {DAY} 14h02)", f" (Paul, {DAY} 14h02)", f" (Paul, {DAY} 14h03)")
EXAMPLES = [
    ("add-add", "A B C X<Y X<Z", {A: "kA", f"team/a{C1}.txt": "kB", f"team/a{C2}.txt": "kC"}, {A}),
    ("edit-edit", "A B C X<Y X<Z", {A: "kC", f"team/a (Alice, {DAY} 14h02).txt": "kA",
                                    f"team/a (Alice, {DAY} 14h02 2).txt": "kB"}, {A}),
    ("edit-delete", "A B C X<Y X<Z", {A: "kA", B: "kB"}, set()),
    ("rename-edit", "A B C X<Y X<Z", {A: "kB", B: "k0", f"team/b{C3}.txt": "kC"}, {B}),
    ("case", "A B C X<Y X<Z", {"team/Photo.jpg": "kA", f"team/photo{C1}.jpg": "kB"}, {"team/photo.jpg"}),
    ("unicode", "A B C X<Y X<Z", {"team/Straße/a.txt": "kA", "team/Straße/b.txt": "kB",
                                  f"team/strasse{C3}": "kC"}, {"team/strasse"}),
    ("package", "A B C X<Y X<Z", {P + "/index.xml": "kA", P + "/data/p.bin": "k1",
                                  f"team/R{C1}.pages/index.xml": "k0", f"team/R{C1}.pages/data/p.bin": "kB"},
     {"team/r.pages"}),
    ("supersede", "A B C Z<X X<Y D X<Z", {A: "kC", f"team/a{C1}.txt": "kB",
                                          f"team/a (Alice, {DAY} 14h04).txt": "kD"}, {A}),
    ("keep-mine", "A B Y<X K X<Y", {A: "kB"}, set()),
    ("keep-mine", "A B Y<X X<Y K E X<Y", {A: "kB", f"team/a (Alice, {DAY} 14h06).txt": "kE"}, {A}),
    ("keep-both", "A B X<Y", {A: "kA", f"team/a{C1}.txt": "kB"}, {A}),
    ("keep-both", "A B X<Y K", {A: "kA", f"team/a{C1}.txt": "kB"}, set()),
    ("keep-both", "A B X<Y Y<X E X<Y", {A: "kA", f"team/a{C1}.txt": "kE"}, set()),
    ("decision", "P E F X<Z D X<Y", {"alice/a.txt": "kP", f"alice/a (Alice, {DAY} 14h02).txt": "kE",
                                     B: "kF"}, {"alice/a.txt"}),
    ("decision", "P E F X<Y X<Z D", {"alice/a.txt": "kE", f"alice/a (Paul, {DAY} 14h01).txt": "kP",
                                     B: "kF"}, set()),
]


def examples():
    """Replay each fixed history in the new mode; return the mismatches."""
    bad = []
    for name, steps, want, notices in EXAMPLES:
        case = next(c for c in CASES if c.name == name)
        run = Run(case, "new")
        st = (tuple((frozenset(["g"]), "g") for _ in case.names), frozenset())
        for step in steps.split():
            if "<" in step:
                st = run.pull(st, case.names.index(step[0]), case.names.index(step[2]))
            elif run.enabled(st, step):
                st = run.act(st, step)
            else:
                bad.append(f"{name} {steps}: {step} is not enabled")
        s, tip = st[0][0]
        got, open_ = run.commits[tip].tree, run.engine.notices(s)
        if got != want or open_ != {fold(x) for x in notices}:
            bad.append(f"{name} {steps}: {show(got)} open {sorted(open_)}")
    return bad


# Laws that old and naive fail, on every schedule of at most OLD_PULLS pulls (spec 05 §5.2).
# old: merge commits are changes (weaknesses a, c); naive: copies vanish (b); both: no folding (d).
OLD_PULLS = 5
MERGES_AS_CHANGES = ("algebra", "order", "seen", "tree")
EXPECTED = {
    "old": dict({c.name: MERGES_AS_CHANGES for c in CASES}, case=("names",), unicode=("names",)),
    "naive": {"add-add": ("kept", "order", "tree"), "edit-edit": ("kept", "order", "tree"),
              "case": ("names",), "unicode": ("names",), "package": ("kept", "order", "tree"),
              "supersede": ("kept", "order", "tree"), "keep-mine": ("kept", "tree"),
              "keep-both": ("kept", "order", "seen", "tree")},
}


def dump(ch, path):
    """Every commit of a case's run, parents first, with the oracle's verdict."""
    run, eng = ch.run, ch.eng
    with open(path, "w", encoding="utf-8") as out:
        for cid, c in run.commits.items():
            rec = {"id": cid, "parents": list(c.parents), "tree": c.tree, "device": c.device,
                   "person": c.person, "who": c.who, "when": c.when, "resolve": list(c.resolve),
                   "accepted": eng.accepted(cid), "contribution": eng.contribution(cid)}
            if rec["accepted"]:
                rec["open"] = sorted(eng.notices([cid]))
            out.write(json.dumps(rec, ensure_ascii=False, sort_keys=True) + "\n")


def main():
    ap = argparse.ArgumentParser(description="Check the catalogue laws on every schedule.")
    ap.add_argument("--mode", choices=("new", "old", "naive"), action="append")
    ap.add_argument("--case", action="append", choices=[c.name for c in CASES])
    ap.add_argument("--pulls", type=int, help=f"bound on pulls (default: none, {OLD_PULLS} for old, naive)")
    ap.add_argument("--dump", metavar="DIR", help="write each case's commits, mode new, to DIR")
    ap.add_argument("-v", action="store_true", help="print a counterexample for each failed law")
    args = ap.parse_args()
    bad = examples()
    for line in bad:
        print("BAD example", line)
    for mode in args.mode or ("new", "old", "naive"):
        for case in CASES:
            if args.case and case.name not in args.case:
                continue
            start = time.time()
            ch = Checker(case, mode, args.pulls if args.pulls or mode == "new" else OLD_PULLS)
            ch.explore()
            if args.dump and mode == "new":
                os.makedirs(args.dump, exist_ok=True)
                dump(ch, os.path.join(args.dump, case.name + ".ndjson"))
            got = set(ch.fails)
            want = set() if mode == "new" else set(EXPECTED[mode].get(case.name, ()))
            ok = got == want if args.pulls is None or mode == "new" else True
            bad += [case.name] if not ok else []
            print(f"{'ok ' if ok else 'BAD'} {mode:5} {case.name:11} {ch.states:7} states"
                  f" {ch.schedules:7} ends"
                  f" {time.time() - start:5.1f}s  fails: {' '.join(sorted(got)) or '-'}"
                  + ("" if ok else f" (expected: {' '.join(sorted(want)) or '-'})"))
            if args.v:
                print(f"  ({case.shows})")
                for law in sorted(ch.fails):
                    print(f"  {law}: {ch.fails[law]}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
