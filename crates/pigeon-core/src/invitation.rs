//! Admission by the group's invitation (09 §9.6, ADR 0075). Whoever proves
//! the invitation's secret joins, under the name they give: as a new person,
//! or, once they agree to, as the person of the roster with that name, with
//! that person's rights. The secret is checked before; this decides the
//! roster change.
//!
//! Laws (`tests/invitation.rs`, 1000 cases each): a name the roster lacks
//! adds one person, not an admin, and the device; a name it has is never
//! taken without consent, and with it the device joins that person; a device
//! of the roster is welcomed again, without an edit, only with its role and
//! annex UUID; a removed device is never admitted; every addition applies
//! without a skip.

use crate::roster::{Device, DeviceId, Edit, Person, PersonId, Role, Roster, apply};

/// What a device that proved the invitation asks for.
#[derive(Clone, Copy, Debug)]
pub struct Arrival<'a> {
    /// Its device ID, which the connection authenticated.
    pub device: &'a DeviceId,
    /// The name its person gives.
    pub person: &'a str,
    /// Whether it may join the person of the roster with that name.
    pub existing: bool,
    /// Its name in the roster, made unique if another device has it.
    pub name: &'a str,
    /// A laptop or a custodian.
    pub role: Role,
    /// Its git-annex UUID in the group.
    pub annex_uuid: &'a str,
    /// The space a custodian offers, in bytes.
    pub capacity: Option<u64>,
}

/// What the roster makes of an arrival.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Admission {
    /// The change that adds it.
    Add(Vec<Edit>),
    /// It is already a device of the roster, with this role and UUID.
    Again,
    /// Its name is this person's, and it did not agree to join them.
    PersonExists(PersonId),
    /// Nothing it asks can be granted.
    Refused,
}

/// The admission of `a` under `roster`.
pub fn admit(roster: &Roster, a: &Arrival<'_>) -> Admission {
    let name = a.person.trim();
    if a.role == Role::Recovery
        || name.is_empty()
        || a.name.trim().is_empty()
        || roster.removed.contains_key(a.device)
    {
        return Admission::Refused;
    }
    if let Some(d) = roster.devices.get(a.device) {
        return if d.role == a.role && d.annex_uuid.as_deref() == Some(a.annex_uuid) {
            Admission::Again
        } else {
            Admission::Refused
        };
    }
    let mut change = Vec::new();
    let person = match roster.person_named(name) {
        Some(p) if a.existing => p,
        Some(p) => return Admission::PersonExists(p),
        None => {
            let id = roster.fresh_person_id(name);
            let p = Person {
                name: name.to_owned(),
                admin: false,
            };
            change.push(Edit::AddPerson(id.clone(), p));
            id
        }
    };
    change.push(Edit::AddDevice(
        a.device.clone(),
        Device {
            person,
            name: roster.fresh_device_name(a.name.trim()),
            role: a.role,
            annex_uuid: Some(a.annex_uuid.to_owned()),
            maxsize: a.capacity.filter(|_| a.role == Role::Custodian),
        },
    ));
    if apply(roster, &change).skipped.is_some() {
        return Admission::Refused;
    }
    Admission::Add(change)
}
