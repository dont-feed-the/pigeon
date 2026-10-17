//! Read-only reference-aware erasure planning (08 §8.6). The digest binds
//! exact named refs, group and selection. Confirmation must recompute it;
//! no history arriving after review can be erased by that old consent.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use pigeon_core::authority::{self, ROSTER};
use pigeon_proto::erase::{Preview, Selection};
use sha2::{Digest, Sha256};

use crate::git::Error;
use crate::group::Group;
use crate::membership::invalid;
use crate::purge_file::PurgeDoc;

/// Internal exact scope; replicated documents get their deadline only
/// when the person confirms. Previewing never starts any waiting clock.
pub struct Plan {
    /// Reviewable counts and exact confirmation token.
    pub preview: Preview,
    /// One base per verified accepted archive/current history domain.
    pub documents: Vec<PurgeDoc>,
    /// Snapshot that must still match before the main CAS.
    pub(crate) reading: crate::retention::Snapshot,
}

fn relative(g: &Group, path: &str) -> Result<String, Error> {
    let path = Path::new(path);
    let root = g.worktree.as_deref().unwrap_or(&g.git_dir);
    let path = std::path::absolute(path).map_err(|_| invalid("invalid local erase path"))?;
    let rel = path
        .strip_prefix(root)
        .map_err(|_| invalid("erase path is outside this folder"))?;
    let parts: Option<Vec<_>> = rel
        .components()
        .map(|part| match part {
            std::path::Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect();
    let rel = parts
        .ok_or_else(|| invalid("erase path cannot traverse parent folders"))?
        .join("/");
    if !rel.is_empty()
        && (pigeon_core::names::portable(&rel).is_err()
            || crate::git::is_pigeon_path(&rel)
            || rel.split('/').any(|p| p == ".git"))
    {
        return Err(invalid("erase path is not a shared file or folder"));
    }
    if g.worktree.is_some() && !crate::restore::path::parents_clear(g, &rel) {
        return Err(invalid("erase path crosses a link or another repository"));
    }
    Ok(rel)
}

/// Build outside `busy` from immutable objects. The caller must first
/// ensure the current head has been accepted, and recheck on confirmation.
pub async fn plan(g: &Group, me: &str, selection: &Selection) -> Result<Plan, Error> {
    let reading = crate::retention::snapshot(g).await?;
    let main = reading
        .main
        .as_ref()
        .ok_or_else(|| invalid("no accepted main"))?;
    let roster = crate::membership::roster(g, main).await?;
    let person = roster
        .devices
        .get(&pigeon_core::roster::DeviceId(me.into()))
        .filter(|d| d.role != pigeon_core::roster::Role::Recovery)
        .map(|d| d.person.clone())
        .ok_or_else(|| invalid("this device is not a group member"))?;
    let index = crate::purge::reference_index(g, &reading.tips).await?;
    let mut retained = BTreeSet::new();
    for tip in &reading.tips {
        retained.extend(
            index
                .ancestors(tip)
                .ok_or_else(|| invalid("incomplete retained history"))?,
        );
    }
    // The cache is append-only; only births reachable from this exact
    // retained-ref reading participate in preview counts and shared bytes.
    let references: Vec<_> = index
        .references
        .iter()
        .filter(|r| retained.contains(&r.birth))
        .collect();
    let (paths, proposals, history, bases) = match selection {
        Selection::Path(path) => {
            let rel = relative(g, path)?;
            let verified = crate::history::tips_at(g, main).await;
            let named: BTreeSet<_> = reading
                .named
                .iter()
                .filter(|(name, _)| {
                    *name == "refs/heads/main" || name.starts_with(crate::refound::ARCHIVE)
                })
                .map(|(_, tip)| tip.clone())
                .collect();
            if named != verified.iter().cloned().collect() {
                return Err(invalid("not every retained archive could be verified"));
            }
            (vec![rel], Vec::new(), true, verified)
        }
        Selection::Suggestion(id) => {
            if id.len() < 4
                || id.len() > 40
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            {
                return Err(invalid("use an unambiguous suggestion id"));
            }
            let kept = crate::proposal::kept(&g.git).await;
            let matches: BTreeSet<_> = kept
                .iter()
                .filter(|(_, tip)| tip.starts_with(id))
                .map(|(_, tip)| tip.clone())
                .collect();
            if matches.len() != 1 {
                return Err(invalid("suggestion id is absent or ambiguous"));
            }
            let tip = matches
                .into_iter()
                .next()
                .ok_or_else(|| invalid("no suggestion"))?;
            let signer = kept
                .iter()
                .find(|(_, candidate)| *candidate == tip)
                .map(|(device, _)| device)
                .ok_or_else(|| invalid("no suggestion signer"))?;
            if !crate::proposal::valid(g, signer, &tip).await? {
                return Err(invalid("suggestion proof is unavailable"));
            }
            let base = crate::proposal::base(g, &tip)
                .await?
                .ok_or_else(|| invalid("suggestion has no accepted base"))?;
            let original = g.git.tree(&base).await?;
            let proposed = g.git.tree(&tip).await?;
            let paths: Vec<_> = authority::delta(&original, &proposed)
                .into_iter()
                .filter(|p| !authority::is_pigeon_path(p))
                .collect();
            if paths.is_empty() {
                return Err(invalid("suggestion contains no file versions"));
            }
            (paths, vec![tip], false, vec![base])
        }
    };
    let documents: Vec<_> = bases
        .into_iter()
        .map(|base| PurgeDoc {
            format: 1,
            kind: "purge".into(),
            history,
            base,
            paths: paths.clone(),
            keys: Vec::new(),
            proposals: proposals.clone(),
            requested_by: person.0.clone(),
            // This is not a submitted deadline. Confirmation sets now + R.
            retention_end: crate::report_file::datetime(0),
        })
        .collect();
    let scopes: Vec<_> = documents.iter().map(PurgeDoc::decision).collect();
    let mut ancestry = BTreeMap::new();
    for scope in &scopes {
        for tip in std::iter::once(&scope.base).chain(scope.proposals.iter()) {
            ancestry.insert(
                tip.clone(),
                index
                    .ancestors(tip)
                    .ok_or_else(|| invalid("incomplete preview history"))?,
            );
        }
    }
    let contains =
        |tip: &String, birth: &String| ancestry.get(tip).is_some_and(|set| set.contains(birth));
    let covered: Vec<_> = references
        .iter()
        .filter(|reference| {
            scopes
                .iter()
                .any(|scope| scope.covers(reference, &contains))
        })
        .collect();
    if covered.is_empty() {
        return Err(invalid(
            "no observed versions are covered by this selection",
        ));
    }
    let keys: BTreeSet<_> = covered.iter().map(|r| r.key.clone()).collect();
    let outside: Vec<_> = references
        .iter()
        .filter(|reference| {
            keys.contains(&reference.key)
                && !scopes
                    .iter()
                    .any(|scope| scope.covers(reference, &contains))
        })
        .collect();
    let shared: BTreeSet<_> = outside.iter().map(|r| &r.key).collect();
    let current = g.git.tree(main).await?;
    let removed: BTreeSet<_> = current
        .keys()
        .filter(|path| {
            history
                && !authority::is_pigeon_path(path)
                && paths
                    .iter()
                    .any(|prefix| pigeon_core::restore::under(path, prefix))
        })
        .cloned()
        .collect();
    let writers = g
        .lock()
        .engine
        .catalogue(std::slice::from_ref(main))
        .writers
        .clone();
    let admin = roster.persons.get(&person).is_some_and(|p| p.admin);
    let needs_approval = !admin
        || removed
            .iter()
            .any(|path| !authority::allowed(&roster, &writers, &person, path, false));
    let snapshot = serde_json::to_vec(&(&g.id, &reading.named, selection, &documents))
        .map_err(|e| invalid(&e.to_string()))?;
    let token = crate::util::hex(&Sha256::digest(snapshot));
    let after = crate::retention::snapshot(g).await?;
    if after.named != reading.named {
        return Err(crate::erase_commit::refuse(
            "erase-stale",
            "history changed; review a fresh erasure preview",
        ));
    }
    let preview = Preview {
        selection: selection.clone(),
        token,
        current_files: removed.len() as u64,
        versions: covered.len() as u64,
        bytes: keys
            .iter()
            .filter_map(|key| pigeon_core::capacity::key_size(key))
            .fold(0u64, u64::saturating_add),
        shared_versions: covered.iter().filter(|r| shared.contains(&r.key)).count() as u64,
        retained_elsewhere: outside
            .iter()
            .map(|r| r.path.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .take(20)
            .collect(),
        retention_days: roster.retention_days,
        needs_approval,
        readiness: crate::erase_state::readiness(g, main).await?,
    };
    Ok(Plan {
        preview,
        documents,
        reading,
    })
}

/// Still-current named history, also checked immediately before mutation.
pub async fn unchanged(g: &Group, plan: &Plan) -> Result<bool, Error> {
    Ok(crate::retention::snapshot(g).await?.named == plan.reading.named)
}

/// Current files the confirmed accepted-history erasure removes. A
/// suggestion-only request never deletes the current accepted tree.
pub(crate) fn removes(plan: &Plan, path: &str) -> bool {
    path != ROSTER
        && !authority::is_pigeon_path(path)
        && plan.documents.iter().any(|doc| {
            doc.history
                && doc
                    .paths
                    .iter()
                    .any(|prefix| pigeon_core::restore::under(path, prefix))
        })
}
