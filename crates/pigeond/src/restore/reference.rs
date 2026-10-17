//! A restore retains its new ordinary signed reference before receiving
//! absent bytes (08 §8.6). Main and proposal refs are the durable intent;
//! no content exception or additional replicated format is introduced.
//! A crash can leave a requested missing version or an extra suggestion,
//! but never makes fetching depend on reviving a covered old reference.

use std::collections::BTreeSet;

use crate::{git::Error, group::Group, suggest::Changes};

#[derive(Default)]
pub(super) struct Intent {
    changes: Changes,
    proposals: Vec<(String, Vec<String>)>,
}

fn invalid(detail: &str) -> Error {
    Error::Parse {
        argv: vec!["restore reference".into()],
        detail: detail.into(),
    }
}

/// Caller holds busy and has captured private saves. Only verified saved
/// pointer entries are supplied; no worktree or index entry is changed.
pub(super) async fn retain(g: &Group, me: &str, mut changes: Changes) -> Result<Intent, Error> {
    if changes.values().any(Option::is_none) {
        return Err(invalid("a restore reference cannot delete a path"));
    }
    let head = g
        .git
        .rev("refs/heads/main")
        .await
        .ok_or_else(|| invalid("no main"))?;
    let main = g.git.tree(&head).await?;
    changes.retain(|path, entry| main.get(path) != entry.as_ref());
    crate::conflicts_file::read(g).ok_or_else(|| invalid("unreadable divergence table"))?;
    // Start at current accepted main, rather than an older suggestion that
    // might already contain this key and be covered by an erasure. Each
    // changed path must have a new birth; existing suggestions stay retained.
    let table = crate::conflicts_file::Table::default();
    let (allowed, proposals) =
        crate::suggest::plan(g, me, &head, &main, &table, changes.clone()).await?;
    let mut intent = Intent {
        changes,
        proposals: Vec::new(),
    };
    for proposal in proposals {
        let commit = crate::proposal::write(g, me, &proposal.parent, &proposal.tree).await?;
        intent.proposals.push((commit, proposal.paths));
    }
    if !allowed.is_empty() {
        let tree = crate::suggest::applied(&main, &allowed);
        let tree = crate::merge::write_tree(&g.git, &tree).await?;
        let commit = g
            .git
            .commit_tree(&tree, &[&head], "pigeon: restore")
            .await?;
        g.git
            .update_ref("refs/heads/main", &commit, Some(&head))
            .await?;
    }
    Ok(intent)
}

impl Intent {
    /// Bind a suggestion to actual installed bytes only after installation
    /// succeeds. A failed/raced path must never inherit another file's stat.
    /// If a crash precedes this step, ordinary capture may make an extra
    /// suggestion; both signed references retain the explicitly requested key.
    pub(super) async fn installed(
        &self,
        g: &Group,
        me: &str,
        paths: &BTreeSet<String>,
    ) -> Result<(), Error> {
        if self.proposals.is_empty() {
            return Ok(());
        }
        let worktree = g.worktree.as_ref().ok_or_else(|| invalid("no worktree"))?;
        let head = g
            .git
            .rev("refs/heads/main")
            .await
            .ok_or_else(|| invalid("no main"))?;
        let main = g.git.tree(&head).await?;
        let mut table =
            crate::conflicts_file::read(g).ok_or_else(|| invalid("unreadable divergence table"))?;
        for (commit, proposed) in &self.proposals {
            if g.git.contains(&head, commit).await
                || g.git
                    .rev(&crate::proposal::name(me, commit))
                    .await
                    .as_deref()
                    != Some(commit.as_str())
            {
                continue;
            }
            let installed: Vec<_> = proposed
                .iter()
                .filter(|p| paths.contains(*p))
                .cloned()
                .collect();
            // Record only a stat verified against the requested bytes, not a
            // newer save made between installation and binding the suggestion.
            let mut verified = std::collections::BTreeMap::new();
            for path in &installed {
                let stat = match self.changes.get(path).and_then(Option::as_ref) {
                    Some(entry) => {
                        let key = crate::git::pointer_key(&g.git.object(&entry.oid).await?);
                        key.and_then(|key| {
                            super::guard::verified(&worktree.join(path), &key, &entry.mode)
                        })
                    }
                    None => None,
                };
                verified.insert(path, stat.map(|s| s.tuple()));
            }
            crate::suggest::record(
                g,
                worktree,
                &main,
                &mut table,
                commit,
                &installed,
                &self.changes,
            )
            .await?;
            for (path, stat) in verified {
                if let Some(divergence) = table.paths.get_mut(path) {
                    divergence.mine_stat = stat;
                }
            }
        }
        crate::conflicts_file::write(g, &table).map_err(|e| invalid(&e))
    }
}
