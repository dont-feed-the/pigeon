//! Local kept-folder download ordering (15 §15.2, 11 §11.2). These
//! preferences order requests only; current retention and transport checks
//! remain responsible for authority. Shared keys take the highest priority
//! of their kept paths and are requested once.

use std::collections::BTreeMap;

use pigeon_core::queue::{self, Work};
use pigeon_core::time::Time;
use serde::Deserialize;

use crate::group::Group;

/// Download preferences keyed by top-level folder name.
pub type Priorities = BTreeMap<String, Priority>;

/// Local configuration spelling of the existing pure queue priority.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    /// Before normal and low folders.
    High,
    /// The default for unlisted folders.
    Normal,
    /// After high and normal folders.
    Low,
}

impl Priority {
    fn queue(self) -> queue::Priority {
        match self {
            Self::High => queue::Priority::High,
            Self::Normal => queue::Priority::Normal,
            Self::Low => queue::Priority::Low,
        }
    }
}

/// Invalid names reject the complete settings reading.
pub(crate) fn validate(priorities: &Priorities) -> Result<(), String> {
    for folder in priorities.keys() {
        if folder.is_empty()
            || matches!(folder.as_str(), "." | "..")
            || folder.contains(['/', '\0'])
        {
            return Err(format!(
                "groups.priority: {folder:?} must be a top-level folder name"
            ));
        }
    }
    Ok(())
}

/// Plan current kept keys without fetching content. Cold immutable history
/// readings happen outside the worktree lock; callers bind the plan to the
/// repository revision and revalidate authority before each request.
pub async fn items(g: &Group, priorities: &Priorities) -> Vec<Work> {
    let keys = {
        let _busy = g.busy.lock().await;
        let Some(main) = g.git.rev("refs/heads/main").await else {
            return Vec::new();
        };
        let Ok(keys) = g.git.keys(&main).await else {
            return Vec::new();
        };
        let targets = crate::shared_overlay::targets(g);
        keys.into_iter()
            .filter(|(path, _)| !targets.is_freed(path))
            .collect::<Vec<_>>()
    };
    let dates = match crate::queue_history::introductions(g).await {
        Ok(dates) => dates,
        Err(error) => {
            tracing::debug!(group = %g.id, %error, "download-dates-unreadable");
            Default::default()
        }
    };
    let mut by_key = BTreeMap::<String, Work>::new();
    for (path, key) in keys {
        // Root files have normal priority; configured names denote folders.
        let priority = path
            .split_once('/')
            .and_then(|(folder, _)| priorities.get(folder))
            .copied()
            .unwrap_or(Priority::Normal)
            .queue();
        let work = Work::Download {
            changed: dates.get(&key).copied().unwrap_or(Time(0)),
            key: key.clone(),
            priority,
        };
        by_key
            .entry(key)
            .and_modify(|old| {
                if queue::compare(&work, old).is_lt() {
                    *old = work.clone();
                }
            })
            .or_insert(work);
    }
    let mut result = by_key.into_values().collect::<Vec<_>>();
    queue::order(&mut result);
    result
}

/// Apply an already validated complete reading, preserving equal snapshots
/// so periodic reloads do not interrupt unchanged queues.
pub(crate) fn configure(d: &crate::daemon::Daemon, config: &crate::config::Config) {
    let mut changed = false;
    for g in d.groups() {
        let priorities = config
            .groups
            .iter()
            .find(|entry| entry.id == g.id)
            .map(|entry| entry.priority.clone())
            .unwrap_or_default();
        let mut known = g.lock();
        if *known.download_priorities != priorities {
            known.download_priorities = std::sync::Arc::new(priorities);
            changed = true;
        }
    }
    if changed {
        d.wake.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::unwrap_used, reason = "configuration preservation fixture")]
    fn priorities_validate_whole_readings_and_survive_identity_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let body = "format = 1\n[[groups]]\nid = 'old'\npath = '/unchanged'\n# my local order\nfuture = 42\n[groups.priority]\nPhotos = 'high'\nArchives = 'low'\n";
        std::fs::write(&path, body).unwrap();
        let config = crate::config::load(&path).unwrap();
        assert_eq!(config.groups[0].priority["Photos"], Priority::High);
        let entry = crate::config::GroupEntry {
            id: "new".into(),
            path: "/unchanged".into(),
            priority: Default::default(),
        };
        crate::config::replace_group(&path, "old", &entry).unwrap();
        let loaded = crate::config::load(&path).unwrap();
        assert_eq!(loaded.groups[0].id, "new");
        assert_eq!(loaded.groups[0].priority, config.groups[0].priority);
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.contains("# my local order") && after.contains("future = 42"));
        for invalid in [
            body.replace("'high'", "'urgent'"),
            body.replace("Photos =", "'Photos/2026' ="),
            body.replace("Photos =", "'..' ="),
            body.replace("Photos =", "'' ="),
        ] {
            std::fs::write(&path, invalid).unwrap();
            assert!(crate::config::load(&path).is_err());
        }
        let added = dir.path().join("added.toml");
        crate::config::add_group(&added, &config.groups[0]).unwrap();
        assert_eq!(crate::config::load(&added).unwrap().groups, config.groups);
    }
}

#[cfg(test)]
#[path = "download_queue_runtime_tests.rs"]
mod runtime_tests;
