//! Local folder-choice persistence (08 §8.5). Legacy lists mean Free;
//! format 1 adds explicit Keep exceptions without expanding into file lists.

use crate::group::Group;
use pigeon_core::keep_targets::Targets;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    format: u32,
    targets: BTreeMap<String, bool>,
}

fn parse(text: &str) -> Option<Targets> {
    if let Ok(old) = serde_json::from_str::<BTreeSet<String>>(text) {
        return Some(Targets(old.into_iter().map(|p| (p, true)).collect()));
    }
    let file: File = serde_json::from_str(text).ok()?;
    (file.format == 1).then_some(Targets(file.targets))
}

pub(crate) fn read(g: &Group) -> Targets {
    std::fs::read_to_string(g.root().join("freed.json"))
        .ok()
        .and_then(|text| parse(&text))
        .unwrap_or_default()
}

pub(crate) fn write(g: &Group, targets: Targets) -> Result<(), String> {
    let bytes = serde_json::to_vec(&File {
        format: 1,
        targets: targets.0,
    })
    .map_err(|e| e.to_string())?;
    pigeon_sys::fs::write_atomic(&g.root().join("freed.json"), &bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_choices_and_nested_exceptions_survive_reload() {
        let old = parse(r#"["Photos"]"#).unwrap_or_default();
        assert!(old.is_freed("Photos/2019/a.jpg"));
        let new = parse(r#"{"format":1,"targets":{"Photos":true,"Photos/2019":false}}"#)
            .unwrap_or_default();
        assert!(!new.is_freed("Photos/2019/new.jpg"));
        assert!(new.is_freed("Photos/2020/new.jpg"));
        assert!(parse(r#"{"format":2,"targets":{"Photos":true}}"#).is_none());
    }
}
