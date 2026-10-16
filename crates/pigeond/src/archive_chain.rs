//! Retain only histories anchored by signed refounding links, including
//! the separately signed proposals needed to verify historical decisions.
//! The chain is bounded to 31 links, matching history and offline export.

use std::collections::BTreeSet;

use crate::group::Group;

/// Validate the complete linked chain and retain its proposal proofs.
/// Caller holds the group busy lock; no content files are installed.
pub(crate) async fn retain(g: &Group, root: &str) -> Result<Vec<(String, String)>, String> {
    let refs = g
        .git
        .run(&[
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            "refs/pigeon/",
        ])
        .await
        .map_err(|e| e.to_string())?;
    let candidates: BTreeSet<String> = refs
        .lines()
        .filter_map(|line| {
            let (name, tip) = line.split_once(' ')?;
            (name.contains("/proposals/") && name.ends_with(&format!("/{tip}")))
                .then(|| tip.to_owned())
        })
        .collect();
    let mut cursor = root.to_owned();
    let mut seen = BTreeSet::new();
    let mut chain = Vec::new();
    for _ in 0..32 {
        if !seen.insert(cursor.clone()) {
            return Err("archive cycle".into());
        }
        let raw = g.git.object(&cursor).await.map_err(|e| e.to_string())?;
        pigeon_core::sig::verify(&raw).map_err(|_| "unsigned archive link")?;
        let Some((old, tip)) = crate::refound::refounded(&raw) else {
            return Ok(chain);
        };
        if chain.len() >= 31 || g.git.root(&tip).await.as_deref() != Some(&old) {
            return Err("archive link unavailable".into());
        }
        let name = crate::refound::archive_ref(&old);
        if g.git.rev(&name).await.is_some_and(|saved| saved != tip) {
            return Err("archive link substituted".into());
        }
        let judge = g.history_judge(&old);
        if !g.lock().verified_archives.contains(&tip) {
            let accepted = crate::verify::judge(&judge, None, &tip)
                .await
                .map_err(|e| e.to_string())?;
            if accepted.accepted.as_deref() != Some(&tip) {
                return Err("archive history unverified".into());
            }
            g.lock().verified_archives.insert(tip.clone());
        }
        if g.git.rev(&name).await.is_none() {
            g.git
                .update_ref(&name, &tip, None)
                .await
                .map_err(|e| e.to_string())?;
        }
        for proposal in &candidates {
            if g.git.root(proposal).await.as_deref() != Some(&old) {
                continue;
            }
            let raw = g.git.object(proposal).await.map_err(|e| e.to_string())?;
            let Ok(device) = pigeon_core::sig::verify(&raw) else {
                continue;
            };
            let name = format!(
                "{}{old}/proposals/{}/{proposal}/{proposal}",
                crate::refound::ARCHIVE,
                device.0
            );
            if g.git.rev(&name).await.as_deref() == Some(proposal.as_str()) {
                continue;
            }
            if crate::proposal::valid(&judge, &device.0, proposal)
                .await
                .map_err(|e| e.to_string())?
            {
                g.git
                    .update_ref(&name, proposal, None)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
        chain.push((old.clone(), tip));
        cursor = old;
    }
    Err("archive chain exceeds 31 links".into())
}
