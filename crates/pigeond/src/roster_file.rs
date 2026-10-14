//! `.pigeon/roster.toml` (04 §4.2) as text, to and from pigeon-core's
//! [`Roster`], with what the slice adds to each custodian: its fingerprints
//! and host fingerprint, so that every member computes the same failure
//! domains (13 §13.12, v0 of ADR 0046's facts). `maxsize` is a number of bytes.
//!
//! Law (`tests/codecs.rs`): parsing a rendered roster gives it back.

use std::collections::{BTreeMap, BTreeSet};

use pigeon_core::roster::{DeviceId, Folder, Person, PersonId, Role, Roster, Rule};
use serde::{Deserialize, Serialize};

/// What a custodian says of its hardware, beside the roster's fields.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    /// Fingerprints of its storage.
    pub fingerprints: BTreeSet<String>,
    /// Fingerprint of its host.
    pub host: Option<String>,
}

/// A roster file: the roster and the facts of its devices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RosterDoc {
    /// The roster.
    pub roster: Roster,
    /// The facts, by device.
    pub facts: BTreeMap<DeviceId, Facts>,
}

#[derive(Serialize, Deserialize)]
struct FileT {
    format: u32,
    rules: u32,
    group: String,
    k: u32,
    #[serde(default = "one")]
    min_sites: u32,
    #[serde(default = "thirty")]
    retention_days: u32,
    #[serde(default)]
    persons: BTreeMap<String, PersonT>,
    #[serde(default)]
    devices: BTreeMap<String, DeviceT>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    removed: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    folders: Vec<FolderT>,
}

#[derive(Serialize, Deserialize)]
struct PersonT {
    name: String,
    #[serde(default)]
    admin: bool,
}

#[derive(Serialize, Deserialize)]
struct DeviceT {
    person: String,
    name: String,
    role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    annex_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    maxsize: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    domain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    site: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    fingerprints: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    host: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct FolderT {
    path: String,
    rule: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    owner: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    stewards: Vec<String>,
}

fn one() -> u32 {
    1
}

fn thirty() -> u32 {
    30
}

fn role_name(r: Role) -> &'static str {
    match r {
        Role::Laptop => "laptop",
        Role::Custodian => "custodian",
        Role::Recovery => "recovery",
    }
}

/// The role of a roster name.
pub fn parse_role(s: &str) -> Option<Role> {
    match s {
        "laptop" => Some(Role::Laptop),
        "custodian" => Some(Role::Custodian),
        "recovery" => Some(Role::Recovery),
        _ => None,
    }
}

/// Parses a roster file.
pub fn parse(text: &str) -> Result<RosterDoc, String> {
    let f: FileT = toml_edit::de::from_str(text).map_err(|e| e.to_string())?;
    if f.format != 1 {
        return Err(format!("roster format {}", f.format));
    }
    let mut facts = BTreeMap::new();
    let mut devices = BTreeMap::new();
    for (id, d) in f.devices {
        let role = parse_role(&d.role).ok_or_else(|| format!("role {}", d.role))?;
        let id = DeviceId(id);
        if !d.fingerprints.is_empty() || d.host.is_some() {
            facts.insert(
                id.clone(),
                Facts {
                    fingerprints: d.fingerprints.into_iter().collect(),
                    host: d.host,
                },
            );
        }
        devices.insert(
            id,
            pigeon_core::roster::Device {
                person: PersonId(d.person),
                name: d.name,
                role,
                annex_uuid: d.annex_uuid,
                maxsize: d.maxsize,
                domain: d.domain,
                site: d.site,
            },
        );
    }
    let mut folders = Vec::new();
    for x in f.folders {
        let rule = match (x.rule.as_str(), x.owner) {
            ("owned", Some(o)) => Rule::Owned(PersonId(o)),
            ("add-only", None) => Rule::AddOnly(x.stewards.into_iter().map(PersonId).collect()),
            (r, _) => return Err(format!("folder rule {r}")),
        };
        folders.push(Folder { path: x.path, rule });
    }
    let roster = Roster {
        rules: f.rules,
        group: f.group,
        k: f.k,
        min_sites: f.min_sites,
        retention_days: f.retention_days,
        persons: f
            .persons
            .into_iter()
            .map(|(id, p)| {
                (
                    PersonId(id),
                    Person {
                        name: p.name,
                        admin: p.admin,
                    },
                )
            })
            .collect(),
        devices,
        removed: f
            .removed
            .into_iter()
            .map(|(id, u)| (DeviceId(id), (!u.is_empty()).then_some(u)))
            .collect(),
        folders,
    };
    Ok(RosterDoc { roster, facts })
}

/// Renders a roster file.
pub fn render(doc: &RosterDoc) -> Result<String, String> {
    let r = &doc.roster;
    let f = FileT {
        format: 1,
        rules: r.rules,
        group: r.group.clone(),
        k: r.k,
        min_sites: r.min_sites,
        retention_days: r.retention_days,
        persons: r
            .persons
            .iter()
            .map(|(id, p)| {
                (
                    id.0.clone(),
                    PersonT {
                        name: p.name.clone(),
                        admin: p.admin,
                    },
                )
            })
            .collect(),
        devices: r
            .devices
            .iter()
            .map(|(id, d)| {
                let facts = doc.facts.get(id).cloned().unwrap_or_default();
                (
                    id.0.clone(),
                    DeviceT {
                        person: d.person.0.clone(),
                        name: d.name.clone(),
                        role: role_name(d.role).to_owned(),
                        annex_uuid: d.annex_uuid.clone(),
                        maxsize: d.maxsize,
                        domain: d.domain.clone(),
                        site: d.site.clone(),
                        fingerprints: facts.fingerprints.into_iter().collect(),
                        host: facts.host,
                    },
                )
            })
            .collect(),
        removed: r
            .removed
            .iter()
            .map(|(id, u)| (id.0.clone(), u.clone().unwrap_or_default()))
            .collect(),
        folders: r
            .folders
            .iter()
            .map(|x| match &x.rule {
                Rule::Owned(p) => FolderT {
                    path: x.path.clone(),
                    rule: "owned".to_owned(),
                    owner: Some(p.0.clone()),
                    stewards: Vec::new(),
                },
                Rule::AddOnly(s) => FolderT {
                    path: x.path.clone(),
                    rule: "add-only".to_owned(),
                    owner: None,
                    stewards: s.iter().map(|p| p.0.clone()).collect(),
                },
            })
            .collect(),
    };
    toml_edit::ser::to_string_pretty(&f).map_err(|e| e.to_string())
}
