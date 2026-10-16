//! `.pigeon/roster.toml` (04 §4.2) as text, to and from pigeon-core's
//! [`Roster`]. `maxsize` is a number of bytes.
//!
//! The schema is closed (ADR 0048): another `format`, a key the schema does
//! not define, or a role or folder rule this release does not know makes the
//! file [`Unread::Unknown`]; anything else that does not fit is
//! [`Unread::Malformed`].
//!
//! Laws (`tests/codecs.rs`): parsing a rendered roster gives it back; the
//! golden files of `fixtures/formats/roster/` read as their directory says
//! (`tests/formats.rs`).

use std::collections::BTreeMap;

use pigeon_core::authority::Unread;
use pigeon_core::roster::{DeviceId, Folder, Person, PersonId, Role, Roster, Rule};
use serde::{Deserialize, Serialize};
use toml_edit::{Item, TableLike};

/// The format this release reads and writes.
pub const FORMAT: u32 = 2;

const TOP: &[&str] = &[
    "format",
    "rules",
    "group",
    "invitation",
    "k",
    "retention_days",
    "relay",
    "telemetry_device",
    "persons",
    "devices",
    "removed",
    "folders",
];
const PERSON: &[&str] = &["name", "admin"];
const DEVICE: &[&str] = &["person", "name", "role", "annex_uuid", "maxsize"];
const FOLDER: &[&str] = &["path", "rule", "owner", "stewards"];

/// A roster file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RosterDoc {
    /// The roster.
    pub roster: Roster,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileT {
    format: u32,
    rules: u32,
    group: String,
    invitation: String,
    k: u32,
    #[serde(default = "thirty")]
    retention_days: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    relay: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    telemetry_device: Option<String>,
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
#[serde(deny_unknown_fields)]
struct PersonT {
    name: String,
    #[serde(default)]
    admin: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceT {
    person: String,
    name: String,
    role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    annex_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    maxsize: Option<u64>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FolderT {
    path: String,
    rule: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    owner: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    stewards: Vec<String>,
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

/// The first key of `t` outside `known`, as `at.key`.
fn extra(t: &dyn TableLike, at: &str, known: &[&str]) -> Option<String> {
    t.iter()
        .map(|(k, _)| k)
        .find(|k| !known.contains(k))
        .map(|k| format!("{at}{k}"))
}

/// The first key of `doc` that the schema does not define.
fn unknown_key(doc: &toml_edit::DocumentMut) -> Option<String> {
    let top = doc.as_table();
    if let Some(k) = extra(top, "", TOP) {
        return Some(k);
    }
    for (name, known) in [("persons", PERSON), ("devices", DEVICE)] {
        let Some(t) = top.get(name).and_then(Item::as_table_like) else {
            continue;
        };
        for (id, item) in t.iter() {
            if let Some(entry) = item.as_table_like()
                && let Some(k) = extra(entry, &format!("{name}.{id}."), known)
            {
                return Some(k);
            }
        }
    }
    let folders: Vec<&dyn TableLike> = match top.get("folders") {
        Some(Item::ArrayOfTables(a)) => a.iter().map(|t| t as &dyn TableLike).collect(),
        Some(Item::Value(toml_edit::Value::Array(a))) => a
            .iter()
            .filter_map(|v| v.as_inline_table().map(|t| t as &dyn TableLike))
            .collect(),
        _ => Vec::new(),
    };
    folders
        .into_iter()
        .find_map(|t| extra(t, "folders.", FOLDER))
}

/// Parses a roster file.
pub fn parse(text: &str) -> Result<RosterDoc, Unread> {
    let malformed = |why: String| Err(Unread::Malformed(why));
    let Ok(doc) = text.parse::<toml_edit::DocumentMut>() else {
        return malformed("not TOML".to_owned());
    };
    match doc.get("format").and_then(Item::as_integer) {
        Some(n) if n == i64::from(FORMAT) => {}
        Some(n) => return Err(Unread::Unknown(format!("format {n}"))),
        None => return malformed("no format".to_owned()),
    }
    if let Some(k) = unknown_key(&doc) {
        return Err(Unread::Unknown(format!("key {k}")));
    }
    let f: FileT = match toml_edit::de::from_str(text) {
        Ok(f) => f,
        Err(e) => return malformed(e.message().to_owned()),
    };
    let mut devices = BTreeMap::new();
    for (id, d) in f.devices {
        let role =
            parse_role(&d.role).ok_or_else(|| Unread::Unknown(format!("role {}", d.role)))?;
        devices.insert(
            DeviceId(id),
            pigeon_core::roster::Device {
                person: PersonId(d.person),
                name: d.name,
                role,
                annex_uuid: d.annex_uuid,
                maxsize: d.maxsize,
            },
        );
    }
    let mut folders = Vec::new();
    for x in f.folders {
        let rule = match (x.rule.as_str(), x.owner) {
            ("owned", Some(o)) => Rule::Owned(PersonId(o)),
            ("add-only", None) => Rule::AddOnly(x.stewards.into_iter().map(PersonId).collect()),
            ("owned" | "add-only", _) => return malformed(format!("folder {} owner", x.path)),
            (r, _) => return Err(Unread::Unknown(format!("folder rule {r}"))),
        };
        folders.push(Folder { path: x.path, rule });
    }
    let roster = Roster {
        rules: f.rules,
        group: f.group,
        invitation: f.invitation,
        k: f.k,
        retention_days: f.retention_days,
        relay: f.relay,
        telemetry_device: f.telemetry_device.map(DeviceId),
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
    Ok(RosterDoc { roster })
}

/// Renders a roster file.
pub fn render(doc: &RosterDoc) -> Result<String, String> {
    let r = &doc.roster;
    let f = FileT {
        format: FORMAT,
        rules: r.rules,
        group: r.group.clone(),
        invitation: r.invitation.clone(),
        k: r.k,
        retention_days: r.retention_days,
        relay: r.relay.clone(),
        telemetry_device: r.telemetry_device.as_ref().map(|d| d.0.clone()),
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
                (
                    id.0.clone(),
                    DeviceT {
                        person: d.person.0.clone(),
                        name: d.name.clone(),
                        role: role_name(d.role).to_owned(),
                        annex_uuid: d.annex_uuid.clone(),
                        maxsize: d.maxsize,
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

/// Human-readable diagnostics must not disclose the permanent join secret.
/// This copy is never persisted as a roster or used to judge signed history.
pub(crate) fn diagnostic(doc: &RosterDoc) -> Result<String, String> {
    let mut public = doc.clone();
    public.roster.invitation = "[redacted]".into();
    render(&public)
}
