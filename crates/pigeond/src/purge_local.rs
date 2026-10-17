//! Local staged pointers keep their existing bytes during physical erasure
//! (07 §7.5, 08 §8.6). They do not create replicated references, authorize
//! transfers of released history, or change a signed erasure decision.

use std::collections::BTreeSet;

use crate::{git::Error, group::Group, membership::invalid};

/// Caller holds `busy`, also excluding native index writers. Bare backup
/// repositories have no private working index to protect. Inspect bounded
/// pointer blobs only; an ordinary large staged blob is already in Git.
pub(crate) async fn keys(g: &Group) -> Result<BTreeSet<String>, Error> {
    if g.worktree.is_none() {
        return Ok(BTreeSet::new());
    }
    let ids: BTreeSet<_> = g
        .git
        .index()
        .await?
        .into_iter()
        .filter(|(path, e)| !crate::git::is_pigeon_path(path) && e.mode.starts_with("100"))
        .map(|(_, e)| e.oid)
        .collect();
    let ids: Vec<_> = ids.into_iter().collect();
    let mut keys = BTreeSet::new();
    for chunk in ids.chunks(256) {
        let input = format!("{}\n", chunk.join("\n"));
        let checked = g
            .git
            .run_in(
                &[
                    "cat-file",
                    "--batch-check=%(objectname) %(objecttype) %(objectsize)",
                ],
                input.as_bytes(),
            )
            .await?;
        let mut blobs = Vec::new();
        let lines: Vec<_> = checked.lines().collect();
        if lines.len() != chunk.len() {
            return Err(invalid("incomplete private index object reading"));
        }
        for (line, expected) in lines.into_iter().zip(chunk) {
            let fields: Vec<_> = line.split_whitespace().collect();
            match fields.as_slice() {
                [oid, "blob", size] if oid == expected => {
                    let size = size
                        .parse::<usize>()
                        .map_err(|_| invalid("invalid staged blob size"))?;
                    if size <= 2048 {
                        blobs.push((*oid).to_owned());
                    }
                }
                _ => return Err(invalid("private staged object is unavailable")),
            }
        }
        let objects = g.git.objects(&blobs).await?;
        if objects.len() != blobs.len() {
            return Err(invalid("incomplete private pointer reading"));
        }
        for object in objects {
            let Some((kind, bytes)) = object else {
                return Err(invalid("private staged pointer is unavailable"));
            };
            if kind != "blob" {
                return Err(invalid("private staged pointer is not a blob"));
            }
            if let Some(key) = crate::git::pointer_key(&bytes) {
                keys.insert(key);
            }
        }
    }
    Ok(keys)
}
