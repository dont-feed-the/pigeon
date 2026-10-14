//! Repair started by a removed custodian (08 §8.4, 08 §8.3): once the
//! roster removes a custodian, its annex UUID is marked dead, so that no
//! copy it held counts any more, and place sends every version to the
//! custodians that remain until K domains hold it again.

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
