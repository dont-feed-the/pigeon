//! Completed erasure in signed refounding archives (08 §8.6, proposed
//! ADR 0077). Each old group gets its own history judge. Only complete
//! immutable epochs continue into the new authority domain; pending old
//! requests remain readable in the archive and are not silently reissued.

use std::collections::{BTreeMap, BTreeSet};

use crate::{git::Error, group::Group, purge_epoch::Verified, purge_state::Active};

/// Immutable checked evidence, cached only after the complete chain passes.
#[derive(Clone, Debug, Default)]
pub(crate) struct Archives {
    /// Canonical archive references bound by the signed genesis chain.
    pub anchors: BTreeMap<String, String>,
    /// Original identities and scopes of completed old decisions.
    pub active: BTreeMap<String, Active>,
    /// Complete original reports checked in their own authority domain.
    pub epochs: BTreeMap<String, Verified>,
}

fn invalid(detail: &str) -> Error {
    crate::membership::invalid(detail)
}

impl Archives {
    /// A removed or substituted root invalidates even a previously checked
    /// cache. Extra retained histories still keep their uncovered births.
    pub fn anchored(&self, named: &BTreeMap<String, String>) -> bool {
        self.anchors
            .iter()
            .all(|(name, tip)| named.get(name) == Some(tip))
    }
}

/// Called by background preparation, outside the worktree lock. Missing
/// histories or proof objects pause content; they never mean no old purge.
pub(crate) async fn load(g: &Group) -> Result<Archives, Error> {
    let mut out = Archives::default();
    let mut cursor = g.id.clone();
    let mut seen = BTreeSet::new();
    for _ in 0..32 {
        if !seen.insert(cursor.clone()) {
            return Err(invalid("refounded erasure archive cycle"));
        }
        let raw = g.git.object(&cursor).await?;
        let Some(link) = pigeon_core::refound::archive(&raw) else {
            if crate::refound::refounded(&raw).is_some() {
                return Err(invalid("unverified refounding link"));
            }
            return Ok(out);
        };
        if out.anchors.len() >= 31 {
            return Err(invalid("refounded erasure chain exceeds 31 links"));
        }
        let name = crate::refound::archive_ref(&link.group);
        if g.git.rev(&name).await.as_deref() != Some(&link.main)
            || g.git.root(&link.main).await.as_deref() != Some(&link.group)
        {
            return Err(invalid(
                "refounded erasure archive is missing or substituted",
            ));
        }
        let judge = g.history_judge(&link.group);
        let result = crate::verify::judge(&judge, None, &link.main).await?;
        if result.accepted.as_deref() != Some(&link.main) {
            return Err(invalid("refounded erasure history is not accepted"));
        }
        let active = crate::purge_state::active(&judge, &link.main).await?;
        let epochs = crate::purge_epoch::read(&judge, &link.main).await?;
        for (id, epoch) in epochs {
            let decision = active
                .get(&id)
                .ok_or_else(|| invalid("archived epoch has no active scope"))?;
            if out.active.insert(id.clone(), decision.clone()).is_some()
                || out.epochs.insert(id, epoch).is_some()
            {
                return Err(invalid("duplicate archived erasure identity"));
            }
        }
        out.anchors.insert(name, link.main);
        cursor = link.group;
    }
    Err(invalid("refounded erasure chain exceeds 31 links"))
}
