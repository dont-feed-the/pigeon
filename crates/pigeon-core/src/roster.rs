//! The roster, its changes and their fold (04 §4.2, §4.7, §4.8; ADR 0051).
//!
//! A change is a list of edits judged once, under its parents' rosters. The
//! roster of a commit is the fold of the changes of its ancestors, in the
//! canonical order, from the genesis roster. A change applies as a whole or
//! is skipped, except for its device removals, which always apply.
//!
//! Laws (`tests/roster.rs`, and `tests/roster_trace.rs` against the `Roster`
//! TLA+ model): the fold depends only on the set of ancestors, not on the
//! order of merges or deliveries; every folded roster is valid and keeps an
//! admin; a removed device never returns; in a linear history no change is
//! skipped and the fold equals applying each change in turn.

use std::collections::{BTreeMap, BTreeSet};

/// A person ID: lowercase ASCII, immutable.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PersonId(pub String);

/// A device ID: the hex of its ed25519 public key (ADR 0007).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(pub String);

/// A device's role (ADRs 0002, 0047).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// A person's computer: untrusted for custody.
    Laptop,
    /// A backup machine.
    Custodian,
    /// A key on paper, never online.
    Recovery,
}

/// A person of the group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Person {
    /// Display name.
    pub name: String,
    /// Whether the person is an admin.
    pub admin: bool,
}

/// A device of the group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    /// Its person.
    pub person: PersonId,
    /// Display name, unique in the roster.
    pub name: String,
    /// Its role.
    pub role: Role,
    /// Its git-annex repository UUID; none for a recovery device.
    pub annex_uuid: Option<String>,
    /// Capacity in bytes, for custodians.
    pub maxsize: Option<u64>,
    /// The hardware it shares with other custodians (ADR 0049).
    pub domain: Option<String>,
    /// Where it stands (ADR 0049).
    pub site: Option<String>,
}

/// The rule of a folder (04 §4.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rule {
    /// Only the owner changes it.
    Owned(PersonId),
    /// Anyone adds; the stewards (the admins if none) modify and delete.
    AddOnly(BTreeSet<PersonId>),
}

/// An entry of `[[folders]]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folder {
    /// The folder's path, `/`-separated.
    pub path: String,
    /// Its rule.
    pub rule: Rule,
}

/// A roster, as the fold computes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Roster {
    /// The rule set the group runs; never decreases (ADR 0048).
    pub rules: u32,
    /// Display name of the group.
    pub group: String,
    /// K: custody copies in distinct failure domains.
    pub k: u32,
    /// Declared sites the copies must span.
    pub min_sites: u32,
    /// Waiting period before a purge, in days.
    pub retention_days: u32,
    /// Persons by ID.
    pub persons: BTreeMap<PersonId, Person>,
    /// Devices by ID.
    pub devices: BTreeMap<DeviceId, Device>,
    /// Devices ever removed, with their annex UUID: never added again.
    pub removed: BTreeMap<DeviceId, Option<String>>,
    /// Folder rules.
    pub folders: Vec<Folder>,
}

/// A field that a `set` edit changes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Field {
    /// The group's name.
    Group,
    /// K.
    K,
    /// `min_sites`.
    MinSites,
    /// `retention_days`.
    RetentionDays,
    /// `rules`.
    Rules,
    /// A person's name.
    PersonName(PersonId),
    /// A person's admin flag.
    PersonAdmin(PersonId),
    /// A device's name.
    DeviceName(DeviceId),
    /// A custodian's capacity.
    DeviceMaxsize(DeviceId),
    /// A custodian's declared domain.
    DeviceDomain(DeviceId),
    /// A custodian's site.
    DeviceSite(DeviceId),
}

/// The value of a field; `Unset` for an absent optional field.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Value {
    /// Text.
    Text(String),
    /// A number.
    Number(u64),
    /// A flag.
    Flag(bool),
    /// No value.
    Unset,
}

/// One edit of a change (04 §4.8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    /// Set a field from the value its author saw to a new one.
    Set {
        /// The field.
        field: Field,
        /// The value the author saw.
        from: Value,
        /// The new value.
        to: Value,
    },
    /// Add a person.
    AddPerson(PersonId, Person),
    /// Add a device.
    AddDevice(DeviceId, Device),
    /// Add a folder rule.
    AddFolder(Folder),
    /// Remove a device: it never returns.
    RemoveDevice(DeviceId),
    /// Remove a person, the devices they have and their steward roles.
    RemovePerson(PersonId),
    /// Remove the folder rule of a path.
    RemoveFolder(String),
}

/// Why a roster is invalid (04 §4.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Invalid {
    /// No admin remains.
    NoAdmin,
    /// A device names a person who is not in the roster.
    UnknownPerson(DeviceId),
    /// Two devices share a name.
    DuplicateDeviceName(String),
    /// K or `min_sites` is zero.
    ZeroSetting,
    /// An owner or steward is not a person.
    UnknownSteward(String),
    /// A device other than a custodian has a domain or a site, or a label is empty.
    BadLabel(DeviceId),
    /// A laptop or custodian lacks an annex UUID, or a recovery device has one.
    BadUuid(DeviceId),
    /// An annex UUID belongs to two devices, a removed one included.
    UuidReused(String),
    /// A removed device is in the roster again.
    RemovedDeviceBack(DeviceId),
}

impl Roster {
    /// Check the validity rules of 04 §4.2 that one roster can show.
    pub fn validate(&self) -> Result<(), Invalid> {
        if !self.persons.values().any(|p| p.admin) {
            return Err(Invalid::NoAdmin);
        }
        if self.k == 0 || self.min_sites == 0 {
            return Err(Invalid::ZeroSetting);
        }
        let mut names = BTreeSet::new();
        let mut uuids: BTreeSet<&str> = BTreeSet::new();
        for u in self.removed.values().flatten() {
            uuids.insert(u);
        }
        for (id, d) in &self.devices {
            if !self.persons.contains_key(&d.person) {
                return Err(Invalid::UnknownPerson(id.clone()));
            }
            if self.removed.contains_key(id) {
                return Err(Invalid::RemovedDeviceBack(id.clone()));
            }
            if !names.insert(d.name.as_str()) {
                return Err(Invalid::DuplicateDeviceName(d.name.clone()));
            }
            let labels = [&d.domain, &d.site];
            let custodian = d.role == Role::Custodian;
            if labels
                .iter()
                .any(|l| l.as_ref().is_some_and(|s| s.is_empty() || !custodian))
            {
                return Err(Invalid::BadLabel(id.clone()));
            }
            match (&d.annex_uuid, d.role) {
                (None, Role::Recovery) => {}
                (Some(u), Role::Laptop | Role::Custodian) => {
                    if !uuids.insert(u) {
                        return Err(Invalid::UuidReused(u.clone()));
                    }
                }
                _ => return Err(Invalid::BadUuid(id.clone())),
            }
        }
        for f in &self.folders {
            let people: Vec<&PersonId> = match &f.rule {
                Rule::Owned(p) => vec![p],
                Rule::AddOnly(s) => s.iter().collect(),
            };
            if let Some(p) = people.into_iter().find(|p| !self.persons.contains_key(*p)) {
                return Err(Invalid::UnknownSteward(p.0.clone()));
            }
        }
        Ok(())
    }

    /// The current value of a field, `None` if its person or device is absent.
    pub fn get(&self, field: &Field) -> Option<Value> {
        let text = |s: &Option<String>| s.clone().map_or(Value::Unset, Value::Text);
        Some(match field {
            Field::Group => Value::Text(self.group.clone()),
            Field::K => Value::Number(u64::from(self.k)),
            Field::MinSites => Value::Number(u64::from(self.min_sites)),
            Field::RetentionDays => Value::Number(u64::from(self.retention_days)),
            Field::Rules => Value::Number(u64::from(self.rules)),
            Field::PersonName(p) => Value::Text(self.persons.get(p)?.name.clone()),
            Field::PersonAdmin(p) => Value::Flag(self.persons.get(p)?.admin),
            Field::DeviceName(d) => Value::Text(self.devices.get(d)?.name.clone()),
            Field::DeviceMaxsize(d) => self
                .devices
                .get(d)?
                .maxsize
                .map_or(Value::Unset, Value::Number),
            Field::DeviceDomain(d) => text(&self.devices.get(d)?.domain),
            Field::DeviceSite(d) => text(&self.devices.get(d)?.site),
        })
    }

    fn set(&mut self, field: &Field, value: &Value) -> bool {
        let small = |v: &Value| match v {
            Value::Number(n) => u32::try_from(*n).ok(),
            _ => None,
        };
        let text = |v: &Value| match v {
            Value::Text(s) => Some(Some(s.clone())),
            Value::Unset => Some(None),
            _ => None,
        };
        match (field, value) {
            (Field::Group, Value::Text(s)) => self.group = s.clone(),
            (Field::K, v) => match small(v) {
                Some(n) => self.k = n,
                None => return false,
            },
            (Field::MinSites, v) => match small(v) {
                Some(n) => self.min_sites = n,
                None => return false,
            },
            (Field::RetentionDays, v) => match small(v) {
                Some(n) => self.retention_days = n,
                None => return false,
            },
            (Field::Rules, v) => match small(v) {
                Some(n) => self.rules = n,
                None => return false,
            },
            (Field::PersonName(p), Value::Text(s)) => match self.persons.get_mut(p) {
                Some(x) => x.name = s.clone(),
                None => return false,
            },
            (Field::PersonAdmin(p), Value::Flag(b)) => match self.persons.get_mut(p) {
                Some(x) => x.admin = *b,
                None => return false,
            },
            (Field::DeviceName(d), Value::Text(s)) => match self.devices.get_mut(d) {
                Some(x) => x.name = s.clone(),
                None => return false,
            },
            (Field::DeviceMaxsize(d), v) => match (self.devices.get_mut(d), v) {
                (Some(x), Value::Number(n)) => x.maxsize = Some(*n),
                (Some(x), Value::Unset) => x.maxsize = None,
                _ => return false,
            },
            (Field::DeviceDomain(d), v) => match (self.devices.get_mut(d), text(v)) {
                (Some(x), Some(t)) => x.domain = t,
                _ => return false,
            },
            (Field::DeviceSite(d), v) => match (self.devices.get_mut(d), text(v)) {
                (Some(x), Some(t)) => x.site = t,
                _ => return false,
            },
            _ => return false,
        }
        true
    }

    /// A removal is recorded even when the device is absent, its addition
    /// skipped or not yet folded, so that it is never added afterwards.
    fn remove_device(&mut self, id: &DeviceId) {
        let uuid = self.devices.remove(id).and_then(|d| d.annex_uuid);
        let slot = self.removed.entry(id.clone()).or_insert(None);
        if uuid.is_some() {
            *slot = uuid;
        }
    }

    /// Apply one edit; `false` if it does not apply (04 §4.8).
    fn edit(&mut self, e: &Edit) -> bool {
        match e {
            Edit::Set { field, from, to } => match self.get(field) {
                Some(now) if now == *from || now == *to => self.set(field, to),
                _ => false,
            },
            Edit::AddPerson(id, p) => match self.persons.get(id) {
                Some(q) => q == p,
                None => {
                    self.persons.insert(id.clone(), p.clone());
                    true
                }
            },
            Edit::AddDevice(id, d) => match self.devices.get(id) {
                Some(x) => x == d,
                None if self.removed.contains_key(id) => false,
                None => {
                    self.devices.insert(id.clone(), d.clone());
                    true
                }
            },
            Edit::AddFolder(f) => match self.folders.iter().find(|x| x.path == f.path) {
                Some(x) => x == f,
                None => {
                    self.folders.push(f.clone());
                    true
                }
            },
            Edit::RemoveDevice(id) => {
                self.remove_device(id);
                true
            }
            Edit::RemovePerson(p) => {
                let ids: Vec<DeviceId> = self
                    .devices
                    .iter()
                    .filter(|(_, d)| d.person == *p)
                    .map(|(i, _)| i.clone())
                    .collect();
                for id in &ids {
                    self.remove_device(id);
                }
                self.persons.remove(p);
                for f in &mut self.folders {
                    if let Rule::AddOnly(s) = &mut f.rule {
                        s.remove(p);
                    }
                }
                true
            }
            Edit::RemoveFolder(path) => {
                self.folders.retain(|f| f.path != *path);
                true
            }
        }
    }
}

/// Why a change was skipped by the fold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Skip {
    /// An edit did not find the value its author saw, or its entry differs.
    Conflict(usize),
    /// The result would be invalid.
    Invalid(Invalid),
}

/// The result of applying a change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applied {
    /// The new roster.
    pub roster: Roster,
    /// Why the change was skipped, if it was.
    pub skipped: Option<Skip>,
}

/// Apply a change as a whole; if it does not apply, apply only its device
/// removals and say why (04 §4.8).
pub fn apply(roster: &Roster, change: &[Edit]) -> Applied {
    let mut next = roster.clone();
    let mut skipped = None;
    for (i, e) in change.iter().enumerate() {
        if !next.edit(e) {
            skipped = Some(Skip::Conflict(i));
            break;
        }
    }
    if skipped.is_none() {
        match next.validate() {
            Ok(()) => {
                return Applied {
                    roster: next,
                    skipped: None,
                };
            }
            Err(why) => skipped = Some(Skip::Invalid(why)),
        }
    }
    let mut kept = roster.clone();
    for e in change {
        if let Edit::RemoveDevice(id) = e {
            kept.remove_device(id);
        }
    }
    Applied {
        roster: kept,
        skipped,
    }
}

/// A commit, as the fold sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit<C> {
    /// Its accepted parents.
    pub parents: Vec<C>,
    /// Recorded author time, in seconds.
    pub time: i64,
    /// The device that signed it.
    pub signer: DeviceId,
    /// δ(c): its roster change; empty for a clean merge.
    pub change: Vec<Edit>,
}

/// The accepted commits of a repository, from the genesis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct History<C: Ord> {
    /// The genesis commit's ID.
    pub genesis: C,
    /// The genesis roster.
    pub roster: Roster,
    /// Every commit but the genesis, by ID.
    pub commits: BTreeMap<C, Commit<C>>,
}

/// The fold of a commit's history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folded<C> {
    /// R(c).
    pub roster: Roster,
    /// The skipped changes, with their reason, in canonical order.
    pub skipped: Vec<(C, Skip)>,
}

impl<C: Ord + Clone> History<C> {
    /// `c` and all its ancestors, the genesis included.
    pub fn ancestors(&self, c: &C) -> BTreeSet<C> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![c.clone()];
        while let Some(x) = stack.pop() {
            if seen.insert(x.clone())
                && let Some(commit) = self.commits.get(&x)
            {
                stack.extend(commit.parents.iter().cloned());
            }
        }
        seen
    }

    /// The commits with a change among `c` and its ancestors, in canonical
    /// order: topological, and among those whose ancestors are all placed,
    /// the smallest (time, signer, ID) first (04 §4.8).
    pub fn canonical_order(&self, c: &C) -> Vec<C> {
        let mut rest: BTreeMap<C, BTreeSet<C>> = self
            .ancestors(c)
            .into_iter()
            .filter(|x| self.commits.get(x).is_some_and(|m| !m.change.is_empty()))
            .map(|x| {
                let mut anc = self.ancestors(&x);
                anc.remove(&x);
                (x, anc)
            })
            .collect();
        let mut out = Vec::with_capacity(rest.len());
        while !rest.is_empty() {
            let next = rest
                .iter()
                .filter(|(_, anc)| !anc.iter().any(|a| rest.contains_key(a)))
                .filter_map(|(x, _)| {
                    self.commits
                        .get(x)
                        .map(|m| ((m.time, m.signer.clone(), x.clone()), x))
                })
                .min_by(|a, b| a.0.cmp(&b.0))
                .map(|(_, x)| x.clone());
            match next {
                Some(x) => {
                    rest.remove(&x);
                    out.push(x);
                }
                None => break,
            }
        }
        out
    }

    /// R(c): the fold of the changes of `c`'s history in canonical order.
    pub fn fold(&self, c: &C) -> Folded<C> {
        let mut roster = self.roster.clone();
        let mut skipped = Vec::new();
        for x in self.canonical_order(c) {
            if let Some(m) = self.commits.get(&x) {
                let a = apply(&roster, &m.change);
                roster = a.roster;
                if let Some(why) = a.skipped {
                    skipped.push((x, why));
                }
            }
        }
        Folded { roster, skipped }
    }
}

/// Whether the device `signer` may sign `change` under `roster` (04 §4.7,
/// §4.3's `.pigeon/` table): an admin changes anything; a device of person P
/// adds or removes devices of P and sets its custodians' capacity, domain and
/// site; a recovery device only adds and removes devices of its own person
/// (ADR 0047); nobody removes their own person.
pub fn may_sign(roster: &Roster, signer: &DeviceId, change: &[Edit]) -> bool {
    let Some(device) = roster.devices.get(signer) else {
        return false;
    };
    let me = &device.person;
    let recovery = device.role == Role::Recovery;
    let admin = !recovery && roster.persons.get(me).is_some_and(|p| p.admin);
    let mine = |d: &DeviceId| roster.devices.get(d).is_none_or(|x| x.person == *me);
    change.iter().all(|e| match e {
        Edit::AddDevice(_, d) => admin || d.person == *me,
        Edit::RemoveDevice(d) => admin || mine(d),
        _ if recovery => false,
        Edit::Set {
            field: Field::DeviceMaxsize(d) | Field::DeviceDomain(d) | Field::DeviceSite(d),
            ..
        } => admin || mine(d),
        Edit::RemovePerson(p) => admin && p != me,
        _ => admin,
    })
}

/// Whether `signer` may sign a clean merge of parents with these rosters:
/// a device of the roster of some parent (rule 2 of 04 §4.4, ADR 0051).
pub fn may_sign_merge(parents: &[&Roster], signer: &DeviceId) -> bool {
    parents.iter().any(|r| r.devices.contains_key(signer))
}
