//! The scenarios of M2, several people and laptops (roadmap M2), S14–S16,
//! as EDSL programs, with the steps and conditions they add.
//!
//! The family: Ana, an admin, creates the group on `ana-1`; her second
//! laptop `ana-2` and Bob's `bob-1` join later; two custodians, K = 2, the
//! folder `docs` under the default root rule, add-only with the admins as
//! stewards (04 §4.5). S14: files flow everywhere and Bob's change of
//! Ana's file is a suggestion she accepts. S15: a declined suggestion
//! stays Bob's, and concurrent edits keep both versions, never overwriting
//! (ADR 0034). S16: two roster changes made apart converge (ADR 0051).

use crate::scenario::{Handle, Scenario, Steps, all as every, expect, hold, not, wait};
use crate::step::{Condition, Kind, LaptopOp, Outcome, Step};
use crate::world::{Content, Custodian, Device, File, Folder, Laptop, World};

impl Laptop {
    fn asks(self, op: LaptopOp) -> Step {
        Step::Laptop {
            on: self,
            op,
            expect: Outcome::Done,
        }
    }
    /// `pigeon invite [--admin] PERSON` for `l`'s person; `l` joins.
    pub fn invites_laptop(self, l: Laptop) -> Step {
        self.asks(LaptopOp::Join { laptop: l })
    }
    /// `pigeon accept` for the newest suggestion changing `file`.
    pub fn accept(self, file: &File) -> Step {
        self.asks(LaptopOp::Accept { file: file.clone() })
    }
    /// `pigeon decline` for the newest suggestion changing `file`.
    pub fn decline(self, file: &File) -> Step {
        self.asks(LaptopOp::Decline { file: file.clone() })
    }
    /// `pigeon resolve FILE --take-group`.
    pub fn take_group(self, file: &File) -> Step {
        self.asks(LaptopOp::TakeGroup { file: file.clone() })
    }
    /// `file` and its conflict copies hold exactly `texts`, as a set.
    pub fn shows(self, file: &File, texts: &[&str]) -> Condition {
        Condition::Versions {
            on: self,
            file: file.clone(),
            contents: texts
                .iter()
                .map(|t| Content::Text((*t).to_owned()))
                .collect(),
        }
    }
    /// `file` or one of its conflict copies holds `text`.
    pub fn keeps_text(self, file: &File, text: &str) -> Condition {
        Condition::Kept {
            on: self,
            file: file.clone(),
            content: Content::Text(text.to_owned()),
        }
    }
    /// `pigeon suggestions` lists one changing `file`.
    pub fn lists_suggestion(self, file: &File) -> Condition {
        Condition::Suggested {
            on: self,
            file: file.clone(),
        }
    }
    /// `pigeon machines` lists `d`.
    pub fn knows(self, d: impl Into<Device>) -> Condition {
        Condition::Knows {
            on: self,
            device: d.into(),
        }
    }
}

/// The family's world.
pub struct Family {
    /// The world, to which a scenario may add people and laptops.
    pub world: World,
    /// Ana's first laptop, which creates the group.
    pub a1: Laptop,
    /// Ana's second laptop, spare.
    pub a2: Laptop,
    /// Bob's laptop, spare.
    pub b1: Laptop,
    /// The first custodian.
    pub c1: Custodian,
    /// The second custodian.
    pub c2: Custodian,
    /// The folder.
    pub docs: Folder,
}

/// A fresh family world.
pub fn family() -> Family {
    let mut world = World::new("family", 2);
    let ana = world.person("ana", true);
    let bob = world.person("bob", false);
    let a1 = world.laptop_of("ana-1", ana);
    let a2 = world.spare_laptop_of("ana-2", ana);
    let b1 = world.spare_laptop_of("bob-1", bob);
    let c1 = world.custodian("backup-1");
    let c2 = world.custodian("backup-2");
    let docs = world.folder("docs");
    Family {
        world,
        a1,
        a2,
        b1,
        c1,
        c2,
        docs,
    }
}

/// How long a change may take to reach every laptop: five daemons on one
/// busy runner, each fetching from its peers.
const FLOW: u32 = 300;

fn scenario(id: &str, title: &str, world: World, steps: Vec<Step>) -> Scenario {
    Scenario {
        id: id.to_owned(),
        title: title.to_owned(),
        world,
        steps: Steps(steps),
        needs: vec![],
    }
}

/// Ana's two laptops and Bob's join, and each gets `files` with `texts`.
fn gather(f: &Family, files: &[(&File, &str)]) -> Vec<Step> {
    let (a1, a2, b1) = (f.a1, f.a2, f.b1);
    let mut steps: Vec<Step> = files.iter().map(|(x, t)| a1.write_text(x, t)).collect();
    steps.push(wait(
        every(
            files
                .iter()
                .map(|(x, _)| a1.sees(x, Kind::Protected))
                .collect(),
        ),
        180,
    ));
    steps.push(a1.invites_laptop(a2));
    steps.push(a1.invites_laptop(b1));
    let got = [a2, b1]
        .iter()
        .flat_map(|l| files.iter().map(|(x, t)| l.has_text(x, t)))
        .collect();
    steps.push(wait(every(got), FLOW));
    steps
}

/// S14: two people, three laptops: files flow everywhere; Bob's change of
/// Ana's file is a suggestion, which he may not accept and she does.
pub fn s14() -> Scenario {
    let f = family();
    let (a1, a2, b1) = (f.a1, f.a2, f.b1);
    let (plan, note) = (f.docs.file("plan.txt"), f.docs.file("bob.txt"));
    let mut steps = gather(&f, &[(&plan, "first plan")]);
    steps.extend([
        b1.write_text(&note, "from bob"),
        wait(
            every(vec![
                a1.has_text(&note, "from bob"),
                a2.has_text(&note, "from bob"),
                a1.sees(&note, Kind::Protected),
            ]),
            FLOW,
        ),
        a2.write_text(&plan, "second plan"),
        wait(
            every(vec![
                a1.has_text(&plan, "second plan"),
                b1.has_text(&plan, "second plan"),
            ]),
            FLOW,
        ),
        b1.write_text(&plan, "bob's plan"),
        wait(a1.lists_suggestion(&plan), FLOW),
        hold(
            every(vec![
                a1.has_text(&plan, "second plan"),
                a2.has_text(&plan, "second plan"),
                b1.has_text(&plan, "bob's plan"),
            ]),
            10,
        ),
        b1.accept(&plan).refused("not-allowed"),
        a1.accept(&plan),
        wait(
            every(vec![
                a1.has_text(&plan, "bob's plan"),
                a2.has_text(&plan, "bob's plan"),
                not(a1.lists_suggestion(&plan)),
            ]),
            FLOW,
        ),
        wait(a1.sees(&plan, Kind::Protected), FLOW),
        expect(b1.has_text(&plan, "bob's plan")),
    ]);
    scenario(
        "S14",
        "two people, three laptops: files flow; a suggestion is accepted",
        f.world,
        steps,
    )
}

/// S15: a declined suggestion stays Bob's alone until he takes the group's
/// version (07 §7.6); edits made apart on Ana's two laptops both stay, one
/// under the name and one as its conflict copy, and neither laptop's file
/// is overwritten (ADR 0034).
pub fn s15() -> Scenario {
    let f = family();
    let (a1, a2, b1) = (f.a1, f.a2, f.b1);
    let (a, b) = (f.docs.file("a.txt"), f.docs.file("notes.txt"));
    let (e1, e2) = ("ana-1's edit", "ana-2's edit");
    let mut steps = gather(&f, &[(&a, "first"), (&b, "ana's notes")]);
    steps.extend([
        b1.write_text(&b, "bob's notes"),
        wait(a1.lists_suggestion(&b), FLOW),
        a1.decline(&b),
        wait(not(a1.lists_suggestion(&b)), 120),
        hold(
            every(vec![
                a1.has_text(&b, "ana's notes"),
                a2.has_text(&b, "ana's notes"),
                b1.has_text(&b, "bob's notes"),
            ]),
            20,
        ),
        b1.take_group(&b),
        wait(b1.has_text(&b, "ana's notes"), 60),
        b1.take_group(&b).refused("not-diverging"),
        a2.stop(),
        a1.write_text(&a, e1),
        wait(b1.has_text(&a, e1), FLOW),
        a2.write_text(&a, e2),
        a2.start(),
        wait(
            every(vec![
                b1.shows(&a, &[e1, e2]),
                a1.keeps_text(&a, e1),
                a2.keeps_text(&a, e2),
            ]),
            FLOW,
        ),
        hold(
            every(vec![
                b1.shows(&a, &[e1, e2]),
                a1.keeps_text(&a, e1),
                a2.keeps_text(&a, e2),
            ]),
            20,
        ),
    ]);
    scenario(
        "S15",
        "a declined suggestion stays its author's; concurrent edits keep both",
        f.world,
        steps,
    )
}

/// S16: Ana's two laptops each admit a new person while they cannot reach
/// each other, so the roster forks; once all run, every laptop knows every
/// device and the newcomers' files reach everyone (ADR 0051).
pub fn s16() -> Scenario {
    let mut f = family();
    let cleo = f.world.person("cleo", false);
    let k1 = f.world.spare_laptop_of("cleo-1", cleo);
    let (a1, a2, b1, c1, c2) = (f.a1, f.a2, f.b1, f.c1, f.c2);
    let docs = f.docs;
    let (a, b, c) = (docs.file("a.txt"), docs.file("b.txt"), docs.file("c.txt"));
    let laptops = [a1, a2, b1, k1];
    let devices: Vec<Device> = laptops
        .iter()
        .map(|l| Device::Laptop(*l))
        .chain([c1.into(), c2.into()])
        .collect();
    let knows = laptops
        .iter()
        .flat_map(|l| devices.iter().map(|d| l.knows(*d)))
        .collect();
    let has = |x: &File, t: &str| every(laptops.iter().map(|l| l.has_text(x, t)).collect());
    let steps = vec![
        a1.write_text(&a, "from ana"),
        wait(a1.sees(&a, Kind::Protected), 180),
        a1.invites_laptop(a2),
        wait(a2.has_text(&a, "from ana"), FLOW),
        // Ana's second laptop admits Bob while her first is off…
        a1.stop(),
        a2.invites_laptop(b1),
        wait(b1.has_text(&a, "from ana"), FLOW),
        // …and her first admits Cleo while every other device is off.
        a2.stop(),
        b1.stop(),
        c1.stop(),
        c2.stop(),
        a1.start(),
        a1.invites_laptop(k1),
        a2.start(),
        b1.start(),
        c1.start(),
        c2.start(),
        wait(every(knows), FLOW),
        b1.write_text(&b, "from bob"),
        k1.write_text(&c, "from cleo"),
        wait(
            every(vec![
                has(&a, "from ana"),
                has(&b, "from bob"),
                has(&c, "from cleo"),
            ]),
            FLOW,
        ),
        wait(
            every(vec![
                a1.sees(&b, Kind::Protected),
                a1.sees(&c, Kind::Protected),
            ]),
            FLOW,
        ),
    ];
    scenario(
        "S16",
        "two roster changes made apart converge",
        f.world,
        steps,
    )
}

/// The scenarios of M2.
pub fn all() -> Vec<Scenario> {
    vec![s14(), s15(), s16()]
}
