//! The scenarios of the safety slice, S1–S11 (13 §13.12), as EDSL programs.
//!
//! Every scenario has one laptop and two custodians, K = 2, and the folder
//! `docs`; some add a spare device. Deadlines are generous: they bound a
//! wait on a CI runner, not the time pigeon should take. S9 is a family:
//! [`s9`] inserts a crash schedule into one workload, [`s9_point`] crashes
//! at one named point, [`all`] lists one S9 per point.

use pigeon_core::headline::Level;

use crate::model::MIB;
use crate::scenario::{Handle, Need, Scenario, Steps, all as every, expect, hold, not, race, wait};
use crate::step::{CrashPoint, Kind, Step};
use crate::world::{Content, Custodian, File, Folder, Laptop, World};

/// The slice's world: one laptop, two custodians, K = 2, folder `docs`.
pub struct Slice {
    /// The world, to which a scenario may add spare devices.
    pub world: World,
    /// The laptop.
    pub l: Laptop,
    /// The first custodian.
    pub c1: Custodian,
    /// The second custodian.
    pub c2: Custodian,
    /// The folder.
    pub docs: Folder,
}

/// A fresh slice world.
pub fn slice() -> Slice {
    let mut world = World::new("slice", 2);
    let l = world.laptop("laptop");
    let c1 = world.custodian("backup-1");
    let c2 = world.custodian("backup-2");
    let docs = world.folder("docs");
    Slice {
        world,
        l,
        c1,
        c2,
        docs,
    }
}

fn scenario(id: &str, title: &str, world: World, steps: Vec<Step>, needs: Vec<Need>) -> Scenario {
    Scenario {
        id: id.to_owned(),
        title: title.to_owned(),
        world,
        steps: Steps(steps),
        needs,
    }
}

fn random(seed: u64, mib: u64) -> Content {
    Content::Random {
        seed,
        len: mib * MIB,
    }
}

/// S1: local → published → protected, each from an observation; a symbolic
/// link listed as not shared.
pub fn s1() -> Scenario {
    let Slice {
        world,
        l,
        c1,
        c2,
        docs,
    } = slice();
    let a = docs.file("notes.txt");
    let link = docs.file("link-to-notes");
    let steps = vec![
        c1.stop(),
        c2.stop(),
        l.write_text(&a, "first words"),
        wait(l.sees(&a, Kind::Local), 60),
        c1.start(),
        wait(l.sees(&a, Kind::Published), 120),
        c2.start(),
        wait(l.sees(&a, Kind::Protected), 120),
        l.symlink(&link, "notes.txt"),
        wait(l.lists_not_shared(&link), 60),
        wait(l.headline(Level::Safe), 60),
    ];
    scenario(
        "S1",
        "local, then published, then protected; a link is not shared",
        world,
        steps,
        vec![],
    )
}

/// S2: `free` leaves a file that is not protected in place, then frees it
/// without a pointer once it is.
pub fn s2() -> Scenario {
    let Slice {
        world, l, c2, docs, ..
    } = slice();
    let a = docs.file("draft.txt");
    let steps = vec![
        c2.stop(),
        l.write_text(&a, "a draft"),
        wait(l.sees(&a, Kind::Published), 120),
        l.free(docs),
        hold(
            every(vec![l.has_text(&a, "a draft"), l.no_pointers(docs)]),
            20,
        ),
        c2.start(),
        wait(l.sees(&a, Kind::Protected), 120),
        wait(l.lacks(&a), 60),
        expect(l.no_pointers(docs)),
        expect(l.sees(&a, Kind::Protected)),
    ];
    scenario(
        "S2",
        "free waits for protection, then frees without a pointer",
        world,
        steps,
        vec![],
    )
}

/// S3: `keep` and `restore` give back the same bytes.
pub fn s3() -> Scenario {
    let Slice { world, l, docs, .. } = slice();
    let a = docs.file("photo.raw");
    let v1 = random(3, 3);
    let steps = vec![
        l.write(&a, v1.clone()),
        wait(l.sees(&a, Kind::Protected), 120),
        l.free(docs),
        wait(l.lacks(&a), 60),
        l.keep(docs),
        wait(l.has(&a, &v1), 120),
        l.write_text(&a, "second version"),
        wait(l.sees(&a, Kind::Protected), 120),
        l.restore(&a, 1),
        wait(l.has(&a, &v1), 120),
        wait(l.sees(&a, Kind::Protected), 120),
    ];
    scenario(
        "S3",
        "keep and restore give back the same bytes",
        world,
        steps,
        vec![],
    )
}

/// S4: an offline custodian: *published* only; *protected* without action
/// once it returns.
pub fn s4() -> Scenario {
    let Slice {
        world, l, c2, docs, ..
    } = slice();
    let a = docs.file("letter.txt");
    let steps = vec![
        c2.stop(),
        l.write_text(&a, "dear group"),
        wait(l.sees(&a, Kind::Published), 120),
        hold(l.sees(&a, Kind::Published), 20),
        c2.start(),
        wait(l.sees(&a, Kind::Protected), 180),
    ];
    scenario(
        "S4",
        "an offline custodian: published only, protected when it returns",
        world,
        steps,
        vec![],
    )
}

/// S5: a 1 GiB transfer killed on either side resumes; no partial object
/// counts.
pub fn s5() -> Scenario {
    let Slice {
        world,
        l,
        c1,
        c2,
        docs,
    } = slice();
    let f = docs.file("video.bin");
    let steps = vec![
        c2.stop(),
        l.write(&f, random(5, 1024)),
        wait(c1.receiving(&f), 300),
        c1.kill(),
        expect(not(c1.holds(&f))),
        c1.start(),
        wait(c1.holds(&f), 900),
        c2.start(),
        wait(c2.receiving(&f), 300),
        l.kill(),
        expect(not(c2.holds(&f))),
        l.start(),
        wait(l.sees(&f, Kind::Protected), 900),
    ];
    scenario(
        "S5",
        "a killed 1 GiB transfer resumes; no partial object counts",
        world,
        steps,
        vec![Need::BigFiles],
    )
}

/// S6: a save racing `keep`'s install wins (spec 07 §7.5).
pub fn s6() -> Scenario {
    let Slice { world, l, docs, .. } = slice();
    let a = docs.file("report.txt");
    let steps = vec![
        l.write(&a, random(6, 8)),
        wait(l.sees(&a, Kind::Protected), 120),
        l.free(docs),
        wait(l.lacks(&a), 60),
        race(vec![l.keep(docs), l.write_text(&a, "my edit")]),
        wait(
            every(vec![l.has_text(&a, "my edit"), l.sees(&a, Kind::Protected)]),
            120,
        ),
        hold(l.has_text(&a, "my edit"), 10),
    ];
    scenario(
        "S6",
        "a save racing keep's install wins",
        world,
        steps,
        vec![],
    )
}

/// S7: a custodian's disk destroyed: not safe; `remove-computer`, a new
/// custodian, repair.
pub fn s7() -> Scenario {
    let Slice {
        mut world,
        l,
        c1,
        docs,
        ..
    } = slice();
    let c3 = world.spare_custodian("backup-3");
    let (a, b) = (docs.file("a.txt"), docs.file("b.bin"));
    let steps = vec![
        l.write_text(&a, "alpha"),
        l.write(&b, random(7, 2)),
        wait(
            every(vec![
                l.sees(&a, Kind::Protected),
                l.sees(&b, Kind::Protected),
            ]),
            120,
        ),
        c1.destroy(),
        wait(l.sees(&a, Kind::Published), 120),
        wait(l.headline(Level::NeedsYou), 60),
        l.remove_computer(c1),
        l.invite(c3),
        wait(
            every(vec![
                l.sees(&a, Kind::Protected),
                l.sees(&b, Kind::Protected),
            ]),
            300,
        ),
        wait(l.headline(Level::Safe), 60),
    ];
    scenario(
        "S7",
        "a destroyed custodian: not safe; replaced and repaired",
        world,
        steps,
        vec![Need::SizedDisks],
    )
}

/// S8: a full custodian refuses admission and says why; a full laptop
/// refuses a restore before it starts.
pub fn s8() -> Scenario {
    let mut world = World::new("slice", 2);
    // Each about 0.7 GB above its device's `annex.diskreserve` (03 §3.4),
    // so that a fill, not the reserve alone, makes it full.
    let l = world.laptop_with_disk("laptop", 2048);
    let c1 = world.custodian_with_disk("backup-1", 5632);
    let _c2 = world.custodian("backup-2");
    let docs = world.folder("docs");
    let big = docs.file("big.bin");
    let body = random(8, 32);
    let steps = vec![
        c1.fill(8),
        l.write(&big, body.clone()),
        wait(c1.says("disk-full"), 180),
        hold(l.sees(&big, Kind::Published), 10),
        c1.unfill(),
        wait(l.sees(&big, Kind::Protected), 300),
        l.free(docs),
        wait(l.lacks(&big), 60),
        l.fill(8),
        l.restore(&big, 1).refused("disk-full"),
        expect(l.lacks(&big)),
        l.unfill(),
        l.keep(docs),
        wait(l.has(&big, &body), 120),
    ];
    scenario(
        "S8",
        "full disks refuse, say why, and change nothing",
        world,
        steps,
        vec![Need::SizedDisks],
    )
}

/// The S9 workload: capture, protection, free, keep, an edit.
pub fn s9_workload(s: &Slice) -> (Vec<Step>, Vec<(File, Content)>) {
    let (l, docs) = (s.l, s.docs);
    let (a, b) = (docs.file("a.bin"), docs.file("b.txt"));
    let (va, vb, va2) = (
        random(9, 2),
        Content::Text("bee".into()),
        Content::Text("a, again".into()),
    );
    let steps = vec![
        l.write(&a, va.clone()),
        l.write(&b, vb.clone()),
        wait(
            every(vec![
                l.sees(&a, Kind::Protected),
                l.sees(&b, Kind::Protected),
            ]),
            180,
        ),
        l.free(docs),
        wait(every(vec![l.lacks(&a), l.lacks(&b)]), 120),
        l.keep(docs),
        wait(every(vec![l.has(&a, &va), l.has(&b, &vb)]), 180),
        l.write(&a, va2.clone()),
        wait(l.sees(&a, Kind::Protected), 180),
    ];
    (steps, vec![(a, va2), (b, vb)])
}

/// S9 with a crash schedule: before workload step `i`, restart the laptop
/// to crash at the `n`-th crossing of `point`. The end is checked as if no
/// crash happened: nothing lost or duplicated, no pointer.
pub fn s9(id: &str, schedule: &[(usize, CrashPoint, u32)]) -> Scenario {
    let s = slice();
    let (work, end) = s9_workload(&s);
    let mut plan = schedule.to_vec();
    plan.sort_by_key(|p| p.0);
    let mut steps = Vec::new();
    for (i, step) in work.into_iter().enumerate() {
        for (_, point, n) in plan.iter().filter(|p| p.0 == i) {
            steps.push(s.l.crash(*point, *n));
        }
        steps.push(step);
    }
    let mut finals: Vec<_> = end.iter().map(|(f, c)| s.l.has(f, c)).collect();
    finals.extend(end.iter().map(|(f, _)| s.l.sees(f, Kind::Protected)));
    steps.push(wait(every(finals), 300));
    steps.push(expect(s.l.no_pointers(s.docs)));
    scenario(
        id,
        "a crash at named points loses and duplicates nothing",
        s.world,
        steps,
        vec![],
    )
}

/// The workload step before which a crash at `point` is armed.
pub fn s9_step(point: CrashPoint) -> usize {
    match point {
        CrashPoint::CaptureBeforeRef => 0,
        CrashPoint::FreeAfterBegin
        | CrashPoint::FreeAfterMoveAside
        | CrashPoint::FreeAfterProved
        | CrashPoint::FreeAfterIndex => 3,
        CrashPoint::KeepAfterBegin | CrashPoint::KeepAfterRenameIn => 5,
    }
}

/// S9 crashing once at `point`, where its loop crosses it.
pub fn s9_point(point: CrashPoint) -> Scenario {
    s9(
        &format!("S9-{}", point.name()),
        &[(s9_step(point), point, 1)],
    )
}

/// S10: a fresh laptop recovers the group from the custodians with the
/// recovery sheet.
pub fn s10() -> Scenario {
    let Slice {
        mut world, l, docs, ..
    } = slice();
    let fresh = world.spare_laptop("new-laptop");
    let (a, b) = (docs.file("a.txt"), docs.file("deep/b.bin"));
    let vb = random(10, 2);
    let steps = vec![
        l.write_text(&a, "keep me"),
        l.write(&b, vb.clone()),
        wait(
            every(vec![
                l.sees(&a, Kind::Protected),
                l.sees(&b, Kind::Protected),
            ]),
            120,
        ),
        l.recovery_sheet(),
        l.kill(),
        fresh.recover_from(l),
        wait(
            every(vec![fresh.has_text(&a, "keep me"), fresh.has(&b, &vb)]),
            300,
        ),
        wait(
            every(vec![
                fresh.sees(&a, Kind::Protected),
                fresh.sees(&b, Kind::Protected),
            ]),
            120,
        ),
    ];
    scenario(
        "S10",
        "a fresh laptop recovers from the custodians",
        world,
        steps,
        vec![],
    )
}

/// S11: steady and peak disk use per device.
pub fn s11() -> Scenario {
    let Slice {
        world,
        l,
        c1,
        c2,
        docs,
    } = slice();
    let files: Vec<File> = (0..4).map(|i| docs.file(&format!("f{i}.bin"))).collect();
    let mut steps: Vec<Step> = files
        .iter()
        .enumerate()
        .map(|(i, f)| l.write(f, random(110 + i as u64, 4)))
        .collect();
    let protected = every(files.iter().map(|f| l.sees(f, Kind::Protected)).collect());
    steps.extend([
        wait(protected.clone(), 180),
        l.measure("steady"),
        c1.measure("steady"),
        c2.measure("steady"),
        expect(l.disk_within(1100)),
        l.write(&files[0], random(120, 4)),
        wait(protected, 180),
        l.measure("after an edit"),
        l.free(docs),
        wait(every(files.iter().map(|f| l.lacks(f)).collect()), 120),
        l.measure("freed"),
    ]);
    scenario(
        "S11",
        "disk use per device, steady and after free",
        world,
        steps,
        vec![],
    )
}

/// The scenarios that do not pass yet against pigeond on both runners,
/// with what their last runs there showed (models-full.yml, run
/// 38592753952, at 5d9e740; 13 §13.12). `pigeon-lab run` reports each as
/// blocked without running it, and runs it with `--with-blocked`, where a
/// pass says to unmark it and a failure does not fail the run. An id `S9`
/// stands for every S9 point. Each one but S5, S7 and S8, which need sized
/// disks or a 1 GiB file, passes with `--isolation dirs` at bcece9a.
pub const BLOCKED: &[(&str, &str)] = &[
    (
        "S3",
        "pigeon restore answered not-in-this-build (both runners)",
    ),
    (
        "S5",
        "the 1 GiB file is not protected 900 s after the laptop's restart \
         (published on Linux, local on macOS): a restarted device listened on a new port",
    ),
    (
        "S7",
        "the laptop still says protected 120 s after backup-1's disk is \
         destroyed (both runners)",
    ),
    (
        "S8",
        "a full custodian says nothing: no disk-full in status or doctor within 180 s \
         (both runners)",
    ),
    ("S9", S9),
    (
        "S10",
        "pigeon recovery-sheet answered not-in-this-build (both runners)",
    ),
    (
        "S11",
        "the laptop's disk holds 2.07 × the content kept (34.7 MB for 16.8 MB) on \
         both runners: each file and its annex object (13 §13.11 allows 1.1 ×)",
    ),
];

const S9: &str = "a crash at a point of free or keep fell inside the request, which \
     answered daemon-not-running (both runners); after a crash before capture's ref, \
     the files were still published after 180 s";

/// Why the scenario `id` is blocked, if it is.
pub fn blocked(id: &str) -> Option<&'static str> {
    BLOCKED
        .iter()
        .find(|(b, _)| *b == id || (*b == "S9" && id.starts_with("S9-")))
        .map(|(_, why)| *why)
}

/// Every scenario of the slice, S9 once per named point.
pub fn all() -> Vec<Scenario> {
    let mut out = vec![s1(), s2(), s3(), s4(), s5(), s6(), s7(), s8()];
    out.extend(CrashPoint::ALL.iter().map(|p| s9_point(*p)));
    out.extend([s10(), s11()]);
    out
}
