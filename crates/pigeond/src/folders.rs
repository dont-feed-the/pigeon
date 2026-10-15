//! The answer to `Folders` (ux.md §6.2): the group's top-level folders,
//! each with its files, those not safe yet, its least safe state, and
//! whether its space is freed on this computer (08 §8.5). Files at the top
//! of the group's folder belong to no folder and are not listed.

use std::collections::BTreeMap;

use pigeon_core::headline::Level;
use pigeon_proto::Response;
use pigeon_proto::api::Folder;

use crate::group::Group;

/// The top-level folder of `path`, if it is in one.
fn top(path: &str) -> Option<&str> {
    path.split_once('/').map(|(t, _)| t)
}

/// The group's folders, as `me` sees them.
pub async fn folders(g: &Group, me: &str) -> Response {
    let observed = crate::status::observe(g, me, "").await;
    let targets = crate::evict::targets(g);
    let mut by: BTreeMap<&str, Vec<&crate::status::Observed>> = BTreeMap::new();
    for o in &observed {
        if let Some(t) = top(&o.path) {
            by.entry(t).or_default().push(o);
        }
    }
    let n = |x: usize| u64::try_from(x).unwrap_or(u64::MAX);
    let folders = by
        .into_iter()
        .map(|(path, files)| Folder {
            path: path.to_owned(),
            files: n(files.len()),
            not_safe: n(files.iter().filter(|o| o.level != Level::Safe).count()),
            state: crate::status::worst(files.iter().map(|o| &o.state)),
            freed: targets.iter().any(|t| crate::evict::under(path, t)),
        })
        .collect();
    let root = g
        .worktree
        .as_ref()
        .map(|w| w.to_string_lossy().into_owned())
        .unwrap_or_default();
    Response::Folders { root, folders }
}

#[cfg(test)]
mod tests {
    use super::top;

    #[test]
    fn a_path_belongs_to_its_first_folder() {
        assert_eq!(top("Photos/2019/a.jpg"), Some("Photos"));
        assert_eq!(top("Photos/a.jpg"), Some("Photos"));
        assert_eq!(top("budget.xlsx"), None);
    }
}
