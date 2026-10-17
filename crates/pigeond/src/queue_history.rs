//! Introduction dates for protection ordering (15 §15.2). Reuse immutable
//! reference births: inheriting a pointer in a merge is not a new version.
//! The latest retained birth of shared bytes supplies their ordering hint.
//! This cache never grants content authority or proves a custody copy.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use pigeon_core::time::Time;

use crate::{
    git::{Error, Git},
    group::Group,
    purge_index::Index,
};

/// One group's incremental, in-memory scheduling metadata. Failed readings
/// do not replace a complete cached tip set; no data is written to Git.
#[derive(Debug, Default)]
pub(crate) struct Cache {
    index: Index,
    times: BTreeMap<String, Time>,
    tips: BTreeSet<String>,
    hints: Arc<BTreeMap<String, Time>>,
}

fn invalid(detail: &str) -> Error {
    Error::Parse {
        argv: vec!["protection introduction times".into()],
        detail: detail.into(),
    }
}

impl Cache {
    async fn read(
        &mut self,
        git: &Git,
        tips: &[String],
    ) -> Result<Arc<BTreeMap<String, Time>>, Error> {
        let wanted: BTreeSet<String> = tips.iter().cloned().collect();
        if wanted == self.tips {
            return Ok(self.hints.clone());
        }
        if wanted.is_empty() {
            self.tips.clear();
            self.hints = Arc::default();
            return Ok(self.hints.clone());
        }
        self.index.extend(git, tips).await?;
        let mut reachable = BTreeSet::new();
        for tip in &wanted {
            reachable.extend(
                self.index
                    .ancestors(tip)
                    .ok_or_else(|| invalid("incomplete retained ancestry"))?,
            );
        }
        let missing: BTreeSet<String> = self
            .index
            .references
            .iter()
            .filter(|r| reachable.contains(&r.birth) && !self.times.contains_key(&r.birth))
            .map(|r| r.birth.clone())
            .collect();
        let missing: Vec<String> = missing.into_iter().collect();
        for chunk in missing.chunks(256) {
            let objects = git.objects(chunk).await?;
            if objects.len() != chunk.len() {
                return Err(invalid("incomplete birth-commit batch"));
            }
            let mut dates = Vec::new();
            for (id, object) in chunk.iter().zip(objects) {
                let Some((kind, raw)) = object else {
                    return Err(invalid("missing birth commit"));
                };
                if kind != "commit" {
                    return Err(invalid("reference birth is not a commit"));
                }
                let (seconds, _) = pigeon_core::authority::author_time(&raw)
                    .ok_or_else(|| invalid("unreadable birth date"))?;
                dates.push((id.clone(), Time(seconds)));
            }
            self.times.extend(dates);
        }
        let mut hints = BTreeMap::<String, Time>::new();
        for reference in &self.index.references {
            if !reachable.contains(&reference.birth) {
                continue;
            }
            let at = self
                .times
                .get(&reference.birth)
                .copied()
                .ok_or_else(|| invalid("missing cached birth date"))?;
            hints
                .entry(reference.key.clone())
                .and_modify(|prior| *prior = (*prior).max(at))
                .or_insert(at);
        }
        self.tips = wanted;
        self.hints = Arc::new(hints);
        Ok(self.hints.clone())
    }
}

/// Use the current retained namespaces, including proposals and archives.
/// A changed reading can change order, never the retained keys a worker
/// must independently revalidate before requesting bytes.
pub(crate) async fn introductions(g: &Group) -> Result<Arc<BTreeMap<String, Time>>, Error> {
    let snapshot = crate::retention::snapshot(g).await?;
    let cache = g.lock().protection_dates.clone();
    cache.lock().await.read(&g.git, &snapshot.tips).await
}
