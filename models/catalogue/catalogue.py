#!/usr/bin/env python3
"""Reference model of the logical catalogue and its merge (spec 05 §5.2).

catalogue(S) is a pure function of a set S of accepted commits, in three modes:
new is spec 05 §5.2 (contributions, conflict events, folded slots, renames);
old is §5.2 before it (heads over every change, merge commits included); naive
is the obvious repair of old (heads over contributions only, copies derived).
Each case of catalogue_cases.py runs every schedule of its contributions and of
pulls between three devices (orders, pairings, fast-forwards, duplicate merges
and deliveries, split and rejoin): new reaches every state; old and naive stop
at OLD_PULLS pulls. Laws: order (the catalogue depends only on which
contribution saw which), tree (tree(c) = catalogue(anc*(c)); a device shows
the tree of its main), kept (a key leaves only through a contribution that had
it), seen (no version no contribution saw comes back), names (portable,
injective after folding), algebra (merge idempotent, commutative,
associative), authority (clean merges change nothing; a copy is allowed to its
author where it is or was written). new passes them all and EXAMPLES; old and
naive fail as EXPECTED says. --dump DIR writes the commits for catalogue_oracle.rs.
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

from catalogue_cases import CASES, DAY, EXAMPLES, EXPECTED, OLD_PULLS


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
    __slots__ = ("id", "parents", "tree", "device", "person", "who", "when", "op", "resolve", "seq",
                 "renames")

    def __init__(self, cid, parents, tree, device, person, who, when, op, resolve, seq, renames=()):
        self.id, self.parents, self.tree, self.device = cid, parents, tree, device
        self.person, self.who, self.when, self.op = person, who, when, op
        self.resolve, self.seq, self.renames = resolve, seq, renames


class Event:
    """A contribution, or a conflict event; writes maps slot -> (value, origin); renames
    maps a slot the contribution renamed to (its new slot, whether the content is unchanged)."""
    __slots__ = ("id", "past", "writes", "resolves", "conflict", "renames")

    def __init__(self, eid, past, writes, resolves=frozenset(), conflict=False, renames=None):
        self.id, self.past, self.writes, self.resolves = eid, past, writes, resolves
        self.conflict, self.renames = conflict, renames or {}


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
        tree, shown, unsettled, heads, eff, _, via = self.present(state)
        old, new = units(tree), units(c.tree)
        delta = [x for x in sorted(old.keys() | new.keys()) if old.get(x) != new.get(x)]
        resolves = frozenset(fold(p) for p in c.resolve  # Keep both, from whoever may change p
                             if fold(p) in old and may(c.person, p, old[fold(p)]))
        if not delta and not resolves:
            return (True, False, state, tree)  # a clean merge: no change, any member signs it
        allowed = all(may(c.person, (new.get(x) or old.get(x))[0], old.get(x)) for x in delta)
        if clashes(c.tree) or not allowed:
            return (False, False, state, tree)  # a proposal (spec 04 §4.5)
        frozen = frozenset(self.freeze(x, heads[x], shown[x], via) for x in unsettled)
        writes = {x: (new.get(x), c.id) for x in delta}
        # A rename (Pigeon-Renamed, ADR 0072) counts when it deletes a file of the base and
        # adds one the base lacks, packages aside; it is pure when the content did not change.
        # It keeps the versions it moved: the events heading the old slot with a version, a
        # pure rename standing for the versions it moved in turn.
        def moved(x):
            return frozenset().union(*(next((h for y, p, h in e.renames.values() if y == x and p), {e.id})
                                       for e in heads.get(x, ()) if eff[e.id][x][0]))
        renames = {fold(a): (fold(b), new[fold(b)][1] == old[fold(a)][1], moved(fold(a)))
                   for a, b in c.renames
                   if unit(a) == a and unit(b) == b and fold(a) in old and fold(a) not in new
                   and fold(b) in new and fold(b) not in old
                   and not isinstance(old[fold(a)][1], frozenset) and not isinstance(new[fold(b)][1], frozenset)}
        self.events[c.id] = Event(c.id, state | frozen, writes, resolves, renames=renames)
        return (True, True, state | frozen | {c.id}, tree)

    def freeze(self, x, hs, shown, via):
        """The conflict event that keeps the versions of slot x where they were shown: it saw
        their writers, and the renames that moved them there (ADR 0072)."""
        writes = {fold(path): ((path, v[1]), o) for v, path, o in shown}
        writes.setdefault(x, (None, None))
        saw = frozenset(e.id for e in hs).union(*(via.get((e.id, x), (frozenset(),))[0] for e in hs))
        eid = ("v", x, saw, frozenset((y, w[0]) for y, w in writes.items()))
        if eid not in self.events:
            past = frozenset().union(*({i} | self.events[i].past for i in saw))
            self.events[eid] = Event(eid, past, writes, conflict=conflicted(shown))
            for value, o in writes.values():
                # allowed where it is, or, if a rename moved it, where its author wrote it (ADR 0072)
                wrote = next((v[0] for v, _ in self.events[o].writes.values() if v and v[1] == value[1]),
                             value[0]) if value else None
                if value and not (may(self.run.commits[o].person, value[0], None)
                                  or may(self.run.commits[o].person, wrote, None)):
                    self.unauthorised.append((value[0], o))
        return eid

    def knew(self, e, x):
        """Whether event e saw a rename of slot x: its x is a new file (ADR 0072)."""
        return any(x in self.events[p].renames for p in e.past)

    def effective(self, state):
        """Each event's writes in a state, renames followed (ADR 0072): a write of a slot
        that a contribution renamed, which neither saw the other, goes to the new slot (the
        earliest such rename by representative order, then the next, until none), unless the
        event writes that slot itself or saw an event write it, or saw a rename of the slot
        it is at; a rename's deletion of the slot it renamed stays. Answers the writes by event,
        the renames each moved write followed, and where each conflict event's frozen slot is."""
        evs = {eid: self.events[eid] for eid in state}
        renames = sorted((self.order(eid), eid, x, y) for eid, e in evs.items()
                         for x, (y, _, _) in e.renames.items())

        def conc(a, b):
            return a.id != b.id and a.id not in b.past and b.id not in a.past

        def saw(e, y):  # an event e has seen wrote y: y is another file to e
            return any(y in self.events[p].writes for p in e.past)

        knew = self.knew

        def shared(r, e, x, used):  # r and e build on one version of x
            return any(w in e.past or w in used for w in r.past if x in self.events[w].writes)

        def target(e, x, within=None):  # among the renames of a past, else of the state
            used, seen = [], {x}
            while x not in e.renames and not knew(e, x):
                nxt = next(((rid, y) for _, rid, rx, y in renames if rx == x and (within is None or rid in within)
                            and conc(evs[rid], e) and shared(evs[rid], e, x, used)), None)
                if nxt is None or nxt[1] in seen or nxt[1] in e.writes or saw(e, nxt[1]):
                    return x, used
                used.append(nxt[0])
                x = nxt[1]
                seen.add(x)
            return x, used

        eff, via, fslot = {}, {}, {}
        for eid, e in evs.items():
            w = {}
            for x, (v, o) in e.writes.items():
                y, used = target(e, x)
                if used:
                    v = v and (evs[used[-1]].writes[y][0][0], v[1])
                    via[eid, y] = (frozenset(used), x)
                w[y] = (v, o)
                if isinstance(eid, tuple) and x == eid[1]:
                    fslot[eid] = y
            eff[eid] = w
        return eff, via, fslot, target

    def heads(self, state):
        """Step 2 with renames (ADR 0072): the writers of each slot that nothing replaced. An
        event replaces a write it saw if it wrote the slot where that write was in its own
        view: where the renames it saw took it. So a version people saw and changed never
        comes back, wherever later renames take it. Then a pure rename's write of its new
        slot yields to a version that followed it there and saw every version it moved, so
        that a concurrent edit wins over a mere move and loses nothing."""
        eff, via, fslot, target = self.effective(state)
        evs = [self.events[eid] for eid in state]
        none = (frozenset(), None)

        def over(w, e, s):  # w saw e's write of slot s, and wrote where it saw it
            return e.id in w.past and target(e, s, w.past)[0] in w.writes

        def heads_of(ws, x):
            return [e for e in ws if not any(over(w, e, via.get((e.id, x), none)[1] or x)
                                             for w in evs if w is not e)]

        writers = defaultdict(list)
        for eid in state:
            for x in eff[eid]:
                writers[x].append(self.events[eid])
        heads = {x: heads_of(ws, x) for x, ws in writers.items()}
        for x, hs in list(heads.items()):
            def yields(r, e):  # e moved here through r and saw every version r moved
                return r.id in via.get((e.id, x), none)[0] and eff[e.id][x][0] is not None and any(
                    y == x and pure and seen <= e.past for y, pure, seen in r.renames.values())
            gone = [r for r in hs if any(yields(r, e) for e in hs)]
            if gone:
                writers[x] = [e for e in writers[x] if e not in gone]
                for e in gone:
                    eff[e.id] = {y: w for y, w in eff[e.id].items() if y != x}
                heads[x] = heads_of(writers[x], x)
        return eff, via, fslot, {x: hs for x, hs in heads.items() if hs}

    def present(self, state):
        """Tree of the catalogue of a state, where each version is shown, unsettled slots."""
        if state in self.memo:
            return self.memo[state]
        eff, via, fslot, heads = self.heads(state)
        versions = []
        for x, hs in heads.items():
            best = {}  # one version per content: names that differ only in case agree
            for e in hs:
                v, o = eff[e.id][x]
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
        copied = {(fslot.get(eid, eid[1]), o) for eid in state if isinstance(eid, tuple)
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
        self.memo[state] = (tree, shown, unsettled, heads, eff, fslot, via)
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
        _, shown, unsettled, _, eff, fslot, _ = self.present(state)
        out = {x for x in unsettled if conflicted(shown[x])}
        for eid in state:
            ev = self.events[eid]
            if ev.conflict:
                x = fslot.get(eid, eid[1])
                if not any(eid in e.past and (e.resolves & {x} or eff[e.id].keys() & eff[eid].keys())
                           for e in map(self.events.get, state) if not isinstance(e.id, tuple)):
                    out.add(x)
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

    def commit(self, cid, parents, tree, device, person, who, when, op=None, resolve=(), renames=()):
        if cid not in self.commits:
            past = frozenset().union(*(self.anc(p) for p in parents))
            seq = sum(1 for a in past if self.commits[a].device == device and self.commits[a].op)
            self.commits[cid] = Commit(cid, parents, tree, device, person, who, when, op, resolve,
                                       seq + (1 if op else 0), renames)
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
        if op.kind == "rename" and op.args[0] not in self.commits[t].tree:
            return False  # nothing to rename on this device's main
        return not clashes(self.edit(st, name)[0])  # else capture holds the save back

    def act(self, st, name):
        devs, done = st
        i = self.case.names.index(self.case.ops[name].device)
        s, t = devs[i]
        view, parents, who, when, resolve, renames = self.edit(st, name)
        op = self.case.ops[name]
        cid = self.commit(hid("op", name, parents), parents, view, op.device, self.case.person[op.device],
                          who, when, name, resolve, renames)
        tip = t if op.kind == "propose" else cid
        return (devs[:i] + ((s | {cid}, tip),) + devs[i + 1:], done | {(name, cid)})

    def edit(self, st, name):
        """The tree that op name saves on its device's main, with its parents and label."""
        devs, done = st
        op = self.case.ops[name]
        s, t = devs[self.case.names.index(op.device)]
        view, parents = dict(self.commits[t].tree), (t,)
        person = self.case.person[op.device]
        who, when, resolve, renames, a = person, op.when, (), (), op.args
        if op.kind in ("set", "propose"):
            for path, key in a:
                drop(view, path) if key is None else view.__setitem__(path, key)
        elif op.kind == "rename":  # capture saw a[0] go and a[1] come alike (ADR 0072)
            view[a[1]] = view.pop(a[0]) if len(a) == 2 else (view.pop(a[0]), a[2])[1]
            renames = ((a[0], a[1]),)
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
        return view, parents, who, when, resolve, renames


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


def dump(ch, path):
    """Every commit of a case's run, parents first, with the oracle's verdict."""
    run, eng = ch.run, ch.eng
    with open(path, "w", encoding="utf-8") as out:
        for cid, c in run.commits.items():
            rec = {"id": cid, "parents": list(c.parents), "tree": c.tree, "device": c.device,
                   "person": c.person, "who": c.who, "when": c.when, "resolve": list(c.resolve),
                   "renames": [list(r) for r in c.renames],
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
