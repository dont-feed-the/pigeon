//! Generators of worlds, steps and crash schedules, from a hegel test case.
#![allow(dead_code)]

use hegel::TestCase;
use hegel::generators as gs;
use pigeon_core::headline::Level;
use pigeon_lab::scenario::{Handle, Scenario, Steps, all, expect, hold, not, race, wait};
use pigeon_lab::slice::{Slice, slice};
use pigeon_lab::step::{Condition, CrashPoint, Kind, Step};
use pigeon_lab::world::{Content, Custodian, File, Folder, Laptop, World};

/// An integer in `lo..=hi`.
pub fn int(tc: &TestCase, lo: i64, hi: i64) -> i64 {
    tc.draw(gs::integers::<i64>().min_value(lo).max_value(hi))
}

/// A boolean.
pub fn flag(tc: &TestCase) -> bool {
    tc.draw(gs::booleans())
}

/// One of `items`, which is not empty.
pub fn pick<T: Clone>(tc: &TestCase, items: &[T]) -> T {
    items[int(tc, 0, items.len() as i64 - 1) as usize].clone()
}

/// The generated world: the slice's, a spare custodian, a spare laptop and
/// a second folder.
pub struct Gen {
    /// The world.
    pub world: World,
    /// Laptops: the slice's and the spare.
    pub laptops: [Laptop; 2],
    /// Custodians: the slice's two and the spare.
    pub custodians: [Custodian; 3],
    /// Folders.
    pub folders: [Folder; 2],
}

/// The world of generated scenarios.
pub fn world() -> Gen {
    let Slice {
        mut world,
        l,
        c1,
        c2,
        docs,
    } = slice();
    let c3 = world.spare_custodian("backup-3");
    let l2 = world.spare_laptop("new-laptop");
    let photos = world.folder("photos");
    Gen {
        world,
        laptops: [l, l2],
        custodians: [c1, c2, c3],
        folders: [docs, photos],
    }
}

/// A file among a few names of the two folders.
pub fn file(tc: &TestCase, g: &Gen) -> File {
    pick(tc, &g.folders).file(pick(tc, &["a.txt", "b.bin", "deep/c.txt"]))
}

/// Small content.
pub fn content(tc: &TestCase) -> Content {
    if flag(tc) {
        Content::Text(pick(tc, &["one", "two", "three", ""]).to_owned())
    } else {
        Content::Random {
            seed: int(tc, 0, 3) as u64,
            len: int(tc, 0, 70_000) as u64,
        }
    }
}

/// A condition.
pub fn condition(tc: &TestCase, g: &Gen, depth: u32) -> Condition {
    let l = pick(tc, &g.laptops);
    let c = pick(tc, &g.custodians);
    let kinds = [
        Kind::Missing,
        Kind::Local,
        Kind::Published,
        Kind::Uncertain,
        Kind::Protected,
    ];
    let levels = [
        Level::Safe,
        Level::Working,
        Level::NotCheckedRecently,
        Level::NotSafeYet,
        Level::NeedsYou,
    ];
    match int(tc, 0, if depth > 0 { 10 } else { 8 }) {
        0 => l.sees(&file(tc, g), pick(tc, &kinds)),
        1 => l.headline(pick(tc, &levels)),
        2 => l.has(&file(tc, g), &content(tc)),
        3 => l.lacks(&file(tc, g)),
        4 => l.no_pointers(pick(tc, &g.folders)),
        5 => l.lists_not_shared(&file(tc, g)),
        6 => c.says("disk-full"),
        7 => c.holds(&file(tc, g)),
        8 => l.disk_within(int(tc, 1000, 1200) as u32),
        9 => not(condition(tc, g, depth - 1)),
        _ => all((0..int(tc, 0, 2))
            .map(|_| condition(tc, g, depth - 1))
            .collect()),
    }
}

/// A step that changes something (no condition).
pub fn action(tc: &TestCase, g: &Gen) -> Step {
    let l = pick(tc, &g.laptops);
    let c = pick(tc, &g.custodians);
    match int(tc, 0, 15) {
        0..=3 => l.write(&file(tc, g), content(tc)),
        4 => l.symlink(&file(tc, g), "elsewhere"),
        5 => l.free(pick(tc, &g.folders)),
        6 => l.keep(pick(tc, &g.folders)),
        7 => l.restore(&file(tc, g), int(tc, 1, 3) as u64),
        8 => {
            if flag(tc) {
                l.start()
            } else {
                c.start()
            }
        }
        9 => {
            if flag(tc) {
                l.stop()
            } else {
                c.kill()
            }
        }
        10 => l.crash(pick(tc, &CrashPoint::ALL), int(tc, 1, 2) as u32),
        11 => c.destroy(),
        12 => l.invite(c),
        13 => l.remove_computer(c),
        14 => l.recovery_sheet(),
        _ => g.laptops[1].recover_from(g.laptops[0]),
    }
}

/// Any step.
pub fn step(tc: &TestCase, g: &Gen) -> Step {
    match int(tc, 0, 9) {
        0 => expect(condition(tc, g, 1)),
        1 => wait(condition(tc, g, 1), int(tc, 1, 30) as u32),
        2 => hold(condition(tc, g, 1), int(tc, 1, 5) as u32),
        3 => race(vec![action(tc, g), action(tc, g)]),
        _ => action(tc, g),
    }
}

/// A few steps.
pub fn steps(tc: &TestCase, g: &Gen, max: i64) -> Steps {
    Steps((0..int(tc, 0, max)).map(|_| step(tc, g)).collect())
}

/// A few actions.
pub fn actions(tc: &TestCase, g: &Gen, max: i64) -> Vec<Step> {
    (0..int(tc, 0, max)).map(|_| action(tc, g)).collect()
}

/// A scenario of the generated world.
pub fn scenario(tc: &TestCase, max: i64) -> Scenario {
    let g = world();
    Scenario {
        id: "G".into(),
        title: "generated".into(),
        steps: steps(tc, &g, max),
        world: g.world,
        needs: Vec::new(),
    }
}

/// An S9 crash schedule: one to three crashes, each before a workload step.
pub fn schedule(tc: &TestCase) -> Vec<(usize, CrashPoint, u32)> {
    (0..int(tc, 1, 3))
        .map(|_| {
            (
                int(tc, 0, 8) as usize,
                pick(tc, &CrashPoint::ALL),
                int(tc, 1, 3) as u32,
            )
        })
        .collect()
}
