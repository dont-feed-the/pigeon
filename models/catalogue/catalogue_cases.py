"""The cases of the catalogue model (models/catalogue/catalogue.py, spec 05 §5.2):
each a few devices, a genesis tree and the operations they race; the fixed
histories of the spec 05 §5.2 case table; and the laws old and naive fail."""


DAY = "2026-10-13"


class Op:
    def __init__(self, device, when, kind, *args, needs=(), conflict=None):
        self.device, self.when, self.kind, self.args = device, when, kind, args
        self.needs, self.conflict = set(needs), conflict


class Case:
    def __init__(self, name, devices, genesis, ops, shows):
        self.name, self.devices, self.genesis, self.ops, self.shows = name, devices, genesis, ops, shows
        self.names = [d for d, _ in devices]
        self.person = dict(devices)


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
        "an edit follows a concurrent rename (ADR 0072), and meets an add of the new name"),
    Case("rename-rename", TEAM, {A: "k0"}, {
        "A": Op("X", t(14, 1), "rename", A, B),
        "B": Op("Y", t(14, 2), "rename", A, "team/c.txt"),
        "C": Op("Z", t(14, 3), "set", (A, "kC"))},
        "two renames keep both names; an edit follows the earlier"),
    Case("rename-delete", TEAM, {A: "k0"}, {
        "A": Op("X", t(14, 1), "rename", A, B),
        "B": Op("Y", t(14, 2), "set", (A, None)),
        "C": Op("Z", t(14, 3), "set", (A, "kC"))},
        "a rename survives a concurrent deletion, as an edit does, and carries an edit"),
    Case("rename-chain", ALICE2, {A: "k0"}, {
        "A": Op("X", t(14, 1), "rename", A, B),
        "D": Op("X", t(14, 4), "rename", B, "team/c.txt", needs="A"),
        "B": Op("Z", t(14, 2), "set", (A, "kB"))},
        "an edit follows two renames made one after the other"),
    Case("rename-edited", TEAM, {A: "k0"}, {
        "A": Op("X", t(14, 1), "rename", A, B, "kA"),
        "B": Op("Y", t(14, 2), "set", (A, "kB"))},
        "a rename that also changed the file meets the edit at the new name: both kept"),
    Case("rename-add", TEAM, {}, {
        "A": Op("X", t(14, 1), "set", (B, "kA")),
        "B": Op("Z", t(14, 2), "set", (B, "kB")),
        "R": Op("X", t(14, 3), "rename", B, A, needs="A")},
        "an add of a name stays when another file of that name is renamed"),
    Case("rename-conflict", TEAM, {B: "k0"}, {
        "A": Op("X", t(14, 1), "set", (B, "kA")),
        "B": Op("Z", t(14, 2), "set", (B, "kB")),
        "R": Op("X", t(14, 3), "rename", B, A, needs="A")},
        "an edit follows a rename of a version it did not see: both kept at the new name"),
    Case("rename-keep", ALICE2, {}, {
        "A": Op("X", t(14, 1), "set", (B, "kA")),
        "B": Op("Y", t(14, 2), "set", (B, "kB")),
        "K": Op("Y", t(14, 5), "keep", B, "kA", needs="AB", conflict=B),
        "R": Op("X", t(14, 3), "rename", B, A, needs="A")},
        "a rename meets Keep mine: the kept version moves, the dropped one stays dropped"),
    Case("rename-folder", TEAM, {A: "k0"}, {
        "A": Op("X", t(14, 1), "rename", A, "alice/a.txt", "kA"),
        "B": Op("Y", t(14, 2), "set", (A, "kB")),
        "C": Op("Z", t(14, 3), "set", (B, "kC"), needs="AB")},
        "an edit follows a rename into a folder its author may not write: kept there as a copy"),
    Case("rename-twice", TEAM, {A: "k0"}, {
        "A": Op("X", t(14, 1), "set", (A, "kA")),
        "R": Op("X", t(14, 3), "rename", A, "team/c.txt", needs="A"),
        "S": Op("X", t(14, 4), "rename", "team/c.txt", B, needs="R"),
        "B": Op("Z", t(14, 2), "set", (A, "kB"))},
        "an edit follows two renames of a version it did not see: both kept"),
    Case("rename-spell", TEAM, {A: "k0"}, {
        "A": Op("X", t(14, 1), "set", ("team/Straße/x.txt", "kA")),
        "R": Op("Y", t(14, 2), "rename", A, "team/STRASSE/y.txt"),
        "E": Op("X", t(14, 3), "set", (A, "kE")),
        "D": Op("X", t(14, 4), "set", ("team/Straße/x.txt", None), needs="AER")},
        "an edit follows a rename into a folder spelt otherwise: the spelling shown stays"),
    Case("rename-seen", TEAM, {A: "k0"}, {
        "S": Op("Z", t(14, 1), "rename", A, B),
        "E": Op("Y", t(14, 2), "set", (A, "kE")),
        "R": Op("X", t(14, 3), "rename", A, B),
        "D": Op("X", t(14, 4), "set", (B, None), needs="RE")},
        "a deletion of an edit seen where a rename took it holds wherever another rename takes it"),
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
    ("rename-edit", "A B C X<Y X<Z", {B: "kB", f"team/b{C3}.txt": "kC"}, {B}),
    ("rename-edit", "A X<Y B", {B: "k0"}, set()),
    ("rename-rename", "A B C X<Y X<Z", {B: "kC", "team/c.txt": "k0"}, set()),
    ("rename-delete", "A B C X<Y X<Z", {B: "kC"}, set()),
    ("rename-chain", "A D B X<Z", {"team/c.txt": "kB"}, set()),
    ("rename-edited", "A B X<Y", {B: "kA", f"team/b{C1}.txt": "kB"}, {B}),
    ("rename-add", "A B R X<Z", {A: "kA", B: "kB"}, set()),
    ("rename-conflict", "A B R X<Z", {A: "kB", f"team/a (Alice, {DAY} 14h03).txt": "kA"}, {A}),
    ("rename-conflict", "A Z<X B R X<Z", {A: "kB"}, set()),
    ("rename-keep", "A B Y<X K R X<Y", {A: "kA"}, set()),
    ("rename-twice", "A R S B X<Z", {B: "kB", f"team/b (Alice, {DAY} 14h04).txt": "kA"}, {B}),
    ("rename-spell", "A R E X<Y D", {"team/Straße/y.txt": "kE"}, set()),
    ("rename-seen", "S E R X<Y D X<Z", {B: "k0"}, set()),
    ("rename-folder", "A B X<Y", {"alice/a.txt": "kA", f"alice/a{C1}.txt": "kB"}, {"alice/a.txt"}),
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


# Laws that old and naive fail, on every schedule of at most OLD_PULLS pulls (spec 05 §5.2).
# old: merge commits are changes (weaknesses a, c); naive: copies vanish (b); both: no folding (d).
OLD_PULLS = 5
MERGES_AS_CHANGES = ("algebra", "order", "seen", "tree")
EXPECTED = {
    "old": dict({c.name: MERGES_AS_CHANGES for c in CASES}, case=("names",), unicode=("names",),
                **{"rename-rename": ("algebra", "order", "tree"), "rename-chain": ("algebra", "order", "tree"),
                   "rename-edited": ("algebra",), "rename-add": ("algebra", "order", "tree"),
                   "rename-conflict": ("algebra", "order", "tree"), "rename-folder": (),
                   "rename-twice": ("algebra", "order", "tree"),
                   "rename-spell": ("algebra", "names", "order", "seen", "tree")}),
    "naive": {"add-add": ("kept", "order", "tree"), "edit-edit": ("kept", "order", "tree"),
              "case": ("names",), "unicode": ("names",), "package": ("kept", "order", "tree"),
              "supersede": ("kept", "order", "tree"), "keep-mine": ("kept", "tree"),
              "keep-both": ("kept", "order", "seen", "tree"), "rename-add": ("kept", "order", "tree"),
              "rename-conflict": ("kept", "order", "tree"), "rename-keep": ("kept", "order", "seen", "tree"),
              "rename-twice": ("kept", "order", "tree"), "rename-spell": ("names",),
              "rename-seen": ("kept", "order", "tree")},
}
