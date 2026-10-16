//! Configuring folder rules (04 §4.3): only an admitted admin computer
//! can change a rule. Replacement is portable-name aware, preserves all
//! unrelated roster state, and uses the existing signed-edit authority.

use crate::names::{fold, portable};
use crate::roster::{DeviceId, Folder, Roster, Rule, diff, may_sign};

/// Why a folder rule cannot be changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The path is not a portable user folder.
    Path,
    /// An owner or steward is not a current person.
    Person,
    /// The source roster is invalid or the change cannot be expressed.
    Roster,
    /// This signing computer may not make the change.
    Authority,
}

/// Set a direct rule, or remove it to inherit its nearest ancestor's.
/// Laws in `tests/folder_rules.rs` run 1000 cases each.
pub fn configure(
    old: &Roster,
    signer: &DeviceId,
    path: &str,
    rule: Option<Rule>,
) -> Result<Roster, Refusal> {
    old.validate().map_err(|_| Refusal::Roster)?;
    let folded = fold(path);
    if portable(path).is_err()
        || folded == ".pigeon"
        || folded.starts_with(".pigeon/")
        || folded == ".gitattributes"
    {
        return Err(Refusal::Path);
    }
    // Rules version 1 matches literal path spelling. Do not silently
    // move a legacy scope to a case/Unicode alias while replacing it.
    if old
        .folders
        .iter()
        .any(|f| fold(&f.path) == folded && f.path != path)
    {
        return Err(Refusal::Path);
    }
    let people: Vec<_> = match &rule {
        None | Some(Rule::PerPerson) => Vec::new(),
        Some(Rule::Owned(person)) => vec![person],
        Some(Rule::AddOnly(stewards)) => stewards.iter().collect(),
    };
    if people.iter().any(|p| !old.persons.contains_key(*p)) {
        return Err(Refusal::Person);
    }
    // Even a no-op reset is an admin action, and paper keys cannot edit rules.
    if !old.devices.get(signer).is_some_and(|device| {
        device.role != crate::roster::Role::Recovery
            && old.persons.get(&device.person).is_some_and(|p| p.admin)
    }) {
        return Err(Refusal::Authority);
    }
    let mut new = old.clone();
    // Per-person folders need rules 2 (ADR 0048): the same signed change
    // raises them, which the caller has the person confirm.
    if rule == Some(Rule::PerPerson) {
        new.rules = new.rules.max(2);
    }
    let position = new
        .folders
        .iter()
        .position(|folder| fold(&folder.path) == folded)
        .unwrap_or(new.folders.len());
    new.folders.retain(|folder| fold(&folder.path) != folded);
    if let Some(rule) = rule {
        new.folders.insert(
            position.min(new.folders.len()),
            Folder {
                path: path.into(),
                rule,
            },
        );
    }
    new.validate().map_err(|_| Refusal::Roster)?;
    let change = diff(old, &new).ok_or(Refusal::Roster)?;
    if !may_sign(old, signer, &change) {
        return Err(Refusal::Authority);
    }
    Ok(new)
}
