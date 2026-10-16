//! The roster of a refounded group (09 §9.9, ADR 0047): one surviving
//! computer's person becomes the sole admin. Folder paths and rule kinds
//! survive; owned folders belong to that person, and absent stewards are
//! removed (an empty set means the admins). This is a new group's authority.
//!
//! Laws (`tests/refound.rs`, 1000 cases each): the result is valid and has
//! only the refounder; paths, rule kinds and group settings survive; paper
//! keys and unknown devices cannot refound; refounding is idempotent.

use std::collections::BTreeMap;

use crate::roster::{DeviceId, Role, Roster, Rule};

/// A new group's roster, preserving the old group's folder boundaries.
pub fn roster(old: &Roster, me: &DeviceId) -> Option<Roster> {
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
            };
            folder
        })
        .collect();
    let new = Roster {
        persons: BTreeMap::from([(owner, person)]),
        devices: BTreeMap::from([(me.clone(), device)]),
        removed: BTreeMap::new(),
        folders,
        ..old.clone()
    };
    new.validate().ok()?;
    Some(new)
}
