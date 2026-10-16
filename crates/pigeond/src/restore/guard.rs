//! Restore keeps the filesystem baseline from before its cold scan. Re-reading
//! an index after another round captured a newer save must not authorize its
//! replacement. Only an already completed exact restore can bypass that baseline.

use std::path::Path;

use pigeon_sys::fs::{Stat, lstat};

pub(super) struct Before {
    pub stat: Option<Stat>,
    pub key: Option<String>,
}

impl Before {
    pub async fn saved(
        g: &crate::group::Group,
        me: &str,
        rel: &str,
        current: Option<&str>,
        wanted: &str,
    ) -> Option<Self> {
        let file = g.worktree.as_ref()?.join(rel);
        let before = Self::read(&file, current, wanted)?;
        if before.clear(&file) {
            return Some(before);
        }
        // A member's captured save stays in its signed proposal while the
        // index stays on main. Verify that retained entry before allowing
        // an explicit restore to replace those already saved local bytes.
        let table = crate::conflicts_file::read(g)?;
        let Some(divergence) = table.paths.get(rel) else {
            return Some(before);
        };
        if divergence.kind != crate::conflicts_file::Kind::Suggestion
            || !crate::suggest::unchanged(g.worktree.as_ref()?, &table, rel, false)
        {
            return Some(before);
        }
        let (Some(commit), Some(mine)) = (&divergence.proposal, &divergence.mine) else {
            return Some(before);
        };
        if g.git
            .rev(&crate::proposal::name(me, commit))
            .await
            .as_deref()
            != Some(commit.as_str())
        {
            return Some(before);
        }
        let tree = g.git.tree(commit).await.ok()?;
        let key = crate::git::pointer_key(&g.git.object(&tree.get(rel)?.oid).await.ok()?);
        if key.as_deref() != Some(mine) {
            return Some(before);
        }
        Self::read(&file, Some(mine), wanted)
    }

    pub fn read(file: &Path, current: Option<&str>, wanted: &str) -> Option<Self> {
        let stat = lstat(file).ok()?;
        let key = if stat.is_some() && crate::evict::is_old(file, Some(wanted)) {
            Some(wanted.to_owned())
        } else {
            current.map(str::to_owned)
        };
        (lstat(file).ok()? == stat).then_some(Self { stat, key })
    }

    pub fn clear(&self, file: &Path) -> bool {
        self.stat.is_none_or(|stat| {
            stat.is_regular()
                && pigeon_sys::writers::scan(stat.id).is_clear()
                && crate::evict::is_old(file, self.key.as_deref())
        }) && lstat(file).ok() == Some(self.stat)
    }

    pub fn source(&self) -> crate::install::RestoreSource<'_> {
        crate::install::RestoreSource {
            key: self.key.as_deref(),
            stat: self.stat,
        }
    }
}

/// A stable observation of the requested bytes and executable mode. The
/// returned stat can bind a suggestion; a later save then differs from it.
pub(super) fn verified(file: &Path, key: &str, mode: &str) -> Option<Stat> {
    let stat = lstat(file).ok()??;
    (stat.is_regular()
        && stat.is_executable() == (mode == "100755")
        && pigeon_sys::writers::scan(stat.id).is_clear()
        && crate::evict::is_old(file, Some(key))
        && lstat(file).ok()? == Some(stat))
    .then_some(stat)
}

#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;
