//! Local folder-rule controls (04 §4.3, 10 §10.1). IDs select people
//! unambiguously; renderers show their names. The daemon checks authority.
use serde::{Deserialize, Serialize};

/// A requested direct rule, or inheritance from the nearest ancestor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Change {
    /// Only this person may change the folder.
    Owned {
        /// The person's ID or unique name.
        person: String,
    },
    /// Everyone adds; these people decide changes (empty means admins).
    AddOnly {
        /// Person IDs or unique names.
        stewards: Vec<String>,
    },
    /// Each folder in it named by a person's ID belongs to that person.
    PerPerson,
    /// Remove the direct rule.
    Inherit,
}

/// A selectable current person.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    /// Stable selection value.
    pub id: String,
    /// Human name.
    pub name: String,
    /// Whether this is the requester.
    pub this_person: bool,
    /// Admins decide changes for an add-only rule with no explicit stewards.
    pub admin: bool,
}

/// One explicit or resolved rule.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    /// Relative folder, empty for the group's default.
    pub path: String,
    /// An owner, else an add-only rule.
    pub owner: Option<String>,
    /// Explicit stewards, or current admins for the default rule.
    pub stewards: Vec<String>,
    /// Whether this folder has its own entry rather than inheritance.
    pub direct: bool,
    /// Each folder in it named by a person's ID belongs to that person.
    #[serde(default)]
    pub per_person: bool,
}

/// Folder rules and the current people they refer to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// Real repository or working root used for local requests.
    pub root: String,
    /// One resolved rule for a requested path, else all direct rules.
    pub rules: Vec<Rule>,
    /// Current people.
    pub people: Vec<Person>,
    /// Whether this computer can change rules.
    pub can_edit: bool,
}
