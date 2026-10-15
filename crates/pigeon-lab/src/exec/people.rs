//! The process executor's steps for several people (M2): a spare laptop
//! joins through an invitation, an admin accepts or declines a suggestion,
//! and the conditions on conflict copies (05 §5.3), suggestions (04 §4.5)
//! and the roster's machines.
//!
//! A laptop is in the group once its folder is a repository: the creator's
//! from `create`, a spare's from its join or recovery. An invitation for a
//! person none of whose laptops is in the group adds the person.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use pigeon_core::names::is_copy_of;

use super::{Exec, cli, observe};
use crate::run::Verdict;
use crate::step::{Condition, Outcome};
use crate::world::{Device, File, Laptop, World};

/// Whether `listed`, a path a response names, is `path`.
fn same(listed: &str, path: &str) -> bool {
    listed == path || listed.ends_with(&format!("/{path}"))
}

impl Exec {
    fn in_group(&self, l: Laptop) -> bool {
        self.devs[l.0 as usize].tree.join(".git").exists()
    }

    /// `by` invites `l`'s person, as an admin if the world makes them one
    /// and they are new, and `l` joins in its folder under its own name.
    pub(super) fn join_laptop(
        &mut self,
        by: Laptop,
        l: Laptop,
        w: &World,
    ) -> Result<Outcome, String> {
        let person = w.person_of(l).ok_or("the world names no persons")?.clone();
        let theirs = w.laptops[l.0 as usize].person;
        let known = (0..w.laptops.len())
            .any(|j| w.laptops[j].person == theirs && self.in_group(Laptop(j as u8)));
        let ask = cli::invite_person(&person.name, person.admin && !known);
        let invite = self.pigeon(Device::Laptop(by), &ask)?;
        if !invite.ok {
            return Ok(invite.outcome());
        }
        let code = invite
            .json
            .as_ref()
            .and_then(|v| cli::find_string(v, "pgn1-"))
            .ok_or_else(|| format!("no invitation in {:?}", invite.stdout))?;
        let d = Device::Laptop(l);
        self.start(d)?;
        let tree = self.devs[l.0 as usize].tree.clone();
        Ok(self
            .pigeon(d, &cli::join_laptop(&code, &tree, w.name(d)))?
            .outcome())
    }

    /// The suggestions `on` lists: their IDs and paths, oldest first.
    fn suggestions(&self, on: Laptop) -> Result<Vec<(String, Vec<String>)>, String> {
        let a = self.pigeon(Device::Laptop(on), &cli::plain("suggestions"))?;
        let v = a.json.ok_or_else(|| {
            format!(
                "suggestions --json printed {:?} {:?}",
                a.stdout,
                a.stderr.trim()
            )
        })?;
        Ok(cli::suggestions(&v))
    }

    /// `pigeon accept` or `pigeon decline` on `on` for the newest suggestion
    /// that changes `file`.
    pub(super) fn decide(
        &mut self,
        on: Laptop,
        file: &File,
        adopt: bool,
        w: &World,
    ) -> Result<Outcome, String> {
        let path = w.path(file);
        let id = self
            .suggestions(on)?
            .into_iter()
            .rev()
            .find(|(_, paths)| paths.iter().any(|p| same(p, &path)))
            .map(|(id, _)| id);
        let Some(id) = id else {
            return Ok(Outcome::Refused("no-such-suggestion".into()));
        };
        let verb = if adopt { "accept" } else { "decline" };
        Ok(self
            .pigeon(Device::Laptop(on), &cli::decide(verb, &id))?
            .outcome())
    }

    /// `pigeon resolve FILE --take-group` on `on`.
    pub(super) fn take_group(
        &mut self,
        on: Laptop,
        file: &File,
        w: &World,
    ) -> Result<Outcome, String> {
        let path = self.path(on, file, w);
        Ok(self
            .pigeon(Device::Laptop(on), &cli::take_group(&path))?
            .outcome())
    }

    /// The file and its conflict copies on `on`'s disk, by name, with the
    /// SHA-256 of each.
    fn copies(&self, on: Laptop, file: &File, w: &World) -> BTreeMap<String, String> {
        let path = self.path(on, file, w);
        let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
            return BTreeMap::new();
        };
        let name = name.to_string_lossy().into_owned();
        // The file is away for an instant while an install moves it.
        let _ = observe::sha256_settled(&path);
        fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                if n != name && !is_copy_of(&n, &name) {
                    return None;
                }
                Some((n, observe::sha256_settled(&e.path())?))
            })
            .collect()
    }

    /// The conditions of several people.
    pub(super) fn people_condition(&mut self, c: &Condition, w: &World) -> Verdict {
        let yes = |b: bool, why: String| if b { Verdict::Pass } else { Verdict::Fail(why) };
        match c {
            Condition::Versions { on, file, contents } => {
                let got = self.copies(*on, file, w);
                let want: BTreeSet<String> = contents.iter().map(|c| self.sha(c)).collect();
                let have: BTreeSet<String> = got.values().cloned().collect();
                yes(have == want, format!("copies {got:?}"))
            }
            Condition::Kept { on, file, content } => {
                let want = self.sha(content);
                let got = self.copies(*on, file, w);
                yes(got.values().any(|s| *s == want), format!("copies {got:?}"))
            }
            Condition::Suggested { on, file } => match self.suggestions(*on) {
                Err(e) => Verdict::Fail(e),
                Ok(list) => {
                    let path = w.path(file);
                    let found = list.iter().any(|(_, ps)| ps.iter().any(|p| same(p, &path)));
                    yes(found, format!("suggestions: {list:?}"))
                }
            },
            Condition::Knows { on, device } => {
                match self.pigeon(Device::Laptop(*on), &cli::plain("machines")) {
                    Err(e) => Verdict::Fail(e),
                    Ok(a) => {
                        let names: Vec<String> = a
                            .json
                            .as_ref()
                            .map(cli::machines)
                            .unwrap_or_default()
                            .into_iter()
                            .map(|(n, _)| n)
                            .collect();
                        yes(
                            names.iter().any(|n| n == w.name(*device)),
                            format!("machines: {names:?}"),
                        )
                    }
                }
            }
            _ => Verdict::Fail("not a condition of several people".into()),
        }
    }
}
