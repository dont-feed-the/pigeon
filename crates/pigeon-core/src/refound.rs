//! The roster of a refounded group (09 §9.9, ADR 0047): one surviving
//! computer's person becomes the sole admin. Folder paths and rule kinds
//! survive; owned folders belong to that person, and absent stewards are
//! removed (an empty set means the admins). This is a new group's authority.
//!
//! Laws (`tests/refound.rs`, 1000 cases each): the result is valid and has
//! only the refounder; paths, rule kinds and group settings survive; paper
//! keys and unknown devices cannot refound; refounding is idempotent.
//! Signed archive-link laws are in `tests/refound_archive.rs` (1000 each).

use std::collections::BTreeMap;

use crate::roster::{DeviceId, Role, Roster, Rule};

/// A signed root's explicit link to an old group's immutable history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Archive {
    /// The old genesis.
    pub group: String,
    /// The exact old tip named by the root.
    pub main: String,
}

/// Reads one unambiguous refounding trailer from a correctly signed root.
/// This authenticates the link, not the old history's authority. Callers
/// must also check its canonical ref and genesis before following it.
pub fn archive(raw: &[u8]) -> Option<Archive> {
    crate::sig::verify(raw).ok()?;
    if !crate::authority::parents(raw).is_empty() {
        return None;
    }
    let text = std::str::from_utf8(raw).ok()?;
    let (_, message) = text.split_once("\n\n")?;
    let mut links = message
        .lines()
        .filter_map(|line| line.strip_prefix("Pigeon-Refound:"));
    let line = links.next()?;
    if links.next().is_some() {
        return None;
    }
    let mut words = line.split_whitespace();
    let (group, main) = (words.next()?, words.next()?);
    let oid = |s: &str| {
        s.len() == 40
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    (words.next().is_none() && oid(group) && oid(main)).then(|| Archive {
        group: group.into(),
        main: main.into(),
    })
}

/// A new group's roster, preserving the old group's folder boundaries,
/// with its own `invitation`: the old one stays the old group's.
pub fn roster(old: &Roster, me: &DeviceId, invitation: &str) -> Option<Roster> {
    let device = old.devices.get(me)?.clone();
    if device.role == Role::Recovery {
        return None;
    }
    let owner = device.person.clone();
    let mut person = old.persons.get(&owner)?.clone();
    person.admin = true;
    let folders = old
        .folders
        .iter()
        .cloned()
        .map(|mut folder| {
            folder.rule = match folder.rule {
                Rule::Owned(_) => Rule::Owned(owner.clone()),
                Rule::AddOnly(mut stewards) => {
                    stewards.retain(|person| person == &owner);
                    Rule::AddOnly(stewards)
                }
                Rule::PerPerson => Rule::PerPerson,
            };
            folder
        })
        .collect();
    let new = Roster {
        // A new authority domain must prepare its own erasure quorum;
        // old capability votes are bound to the archived genesis.
        rules: if old.folders.iter().any(|f| f.rule == Rule::PerPerson) {
            2
        } else {
            1
        },
        persons: BTreeMap::from([(owner, person)]),
        devices: BTreeMap::from([(me.clone(), device)]),
        removed: BTreeMap::new(),
        folders,
        invitation: invitation.to_owned(),
        ..old.clone()
    };
    new.validate().ok()?;
    Some(new)
}
