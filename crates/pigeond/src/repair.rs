//! Repair started by a removed custodian (08 §8.4, 08 §8.3): once the
//! roster removes a custodian, its annex UUID is marked dead, so that no
//! copy it held counts any more, and place sends every version to the
//! custodians that remain until K of them hold it again. The keys of
//! retained history it held are endangered until then (08 §8.4, `admission`).

use std::collections::BTreeSet;

use crate::group::Group;

/// Marks the annex UUIDs of removed devices dead, once each.
pub async fn repair(g: &Group, _me: &str) -> usize {
    let Some(doc) = g.roster() else {
        return 0;
    };
    let dead_file = g.root().join("dead.json");
    let mut dead: BTreeSet<String> = std::fs::read_to_string(&dead_file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let mut marked = 0;
    for uuid in doc.roster.removed.values().flatten() {
        if dead.contains(uuid) {
            continue;
        }
        if let Err(error) = endanger(g, uuid).await {
            tracing::warn!(group = %g.id, uuid = %uuid, %error, "endanger-failed");
            continue;
        }
        match g.git.run(&["annex", "dead", uuid]).await {
            Ok(_) => {
                tracing::info!(group = %g.id, uuid = %uuid, "repair-started");
                dead.insert(uuid.clone());
                marked += 1;
            }
            Err(e) => tracing::warn!(group = %g.id, uuid = %uuid, error = %e, "dead-failed"),
        }
    }
    if marked > 0
        && let Ok(text) = serde_json::to_vec(&dead)
    {
        let _ = pigeon_sys::fs::write_atomic(&dead_file, &text);
    }
    marked
}

/// Records as endangered the retained keys that `uuid` holds, before it
/// is marked dead and no longer listed.
async fn endanger(g: &Group, uuid: &str) -> Result<(), String> {
    let keys = crate::retention::keys(g).await.map_err(|e| e.to_string())?;
    let mut endangered = crate::admission::endangered(g);
    let before = endangered.len();
    for key in keys {
        if crate::custody::holders(&g.git, &key).await.contains(uuid) {
            endangered.insert(key);
        }
    }
    if endangered.len() != before {
        tracing::info!(group = %g.id, uuid = %uuid, keys = endangered.len() - before, "endangered");
        crate::admission::save_endangered(g, &endangered)?;
    }
    Ok(())
}
