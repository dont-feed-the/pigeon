//! Immutable completion witnesses for purge (08 §8.6, proposed ADR 0075).
//! Raw signed reports, a fully accepted closing head and that head's whole
//! custodian quorum are checked independently. A witness closes an epoch;
//! coverage/current keys still decide release. Report hashes in TOML are
//! not Git edges, so verified reports get durable, fetched proof roots.

use std::collections::BTreeMap;

use pigeon_core::authority::Unread;
use pigeon_core::purge::{Epoch, Seal};
use pigeon_core::roster::{DeviceId, Role};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::git::Error;
use crate::group::Group;
use crate::purge_proofs::Proofs;

/// Replicated, add-only evidence; hash suffixes avoid concurrent-writer conflicts.
pub const PREFIX: &str = ".pigeon/purge-epochs/";
/// Full signed report roots, since a TOML hash is not Git reachability.
pub const PROOFS: &str = "refs/pigeon/purge-proof/";

/// The complete immutable certificate. No stored released-key list exists.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Doc {
    /// Exactly one in this release.
    pub format: u32,
    /// Exactly `purge-epoch`.
    pub kind: String,
    /// Stable accepted decision introduction/path identity.
    pub decision: String,
    /// Accepted closing head containing each original seal's main.
    pub main: String,
    /// Exactly one full signed report per custodian of the closing roster.
    pub reports: BTreeMap<String, String>,
}

/// A checked certificate, keeping the exact original signer set and holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verified {
    /// Its exact signed-history evidence references.
    pub doc: Doc,
    /// Input to the pure release algebra.
    pub epoch: Epoch<String, String>,
    /// Exact immutable seal values from the original report objects.
    pub seals: Proofs,
}

fn identity(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn invalid(detail: &str) -> Error {
    Error::Parse {
        argv: vec!["purge epoch".into()],
        detail: detail.into(),
    }
}

/// Canonical document bytes and their collision-free metadata path.
pub fn render(doc: &Doc) -> Result<(String, String), String> {
    let text = toml_edit::ser::to_string(doc).map_err(|e| e.to_string())?;
    let hash = crate::util::hex(&Sha256::digest(text.as_bytes()));
    Ok((format!("{PREFIX}{}/{hash}.toml", doc.decision), text))
}

/// Closed format, exact identifiers and a path binding the actual bytes.
/// Future fields remain unknown; they never authorize an epoch.
pub fn parse(path: &str, text: &str) -> Result<Doc, Unread> {
    let table: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e: toml_edit::TomlError| Unread::Malformed(e.to_string()))?;
    match table.get("format").and_then(toml_edit::Item::as_integer) {
        Some(1) => {}
        Some(n) if n > 1 => return Err(Unread::Unknown("purge epoch format".into())),
        _ => return Err(Unread::Malformed("purge epoch format".into())),
    }
    if table
        .iter()
        .any(|(k, _)| !["format", "kind", "decision", "main", "reports"].contains(&k))
    {
        return Err(Unread::Unknown("purge epoch field".into()));
    }
    let doc: Doc = toml_edit::de::from_str(text).map_err(|e| Unread::Malformed(e.to_string()))?;
    if doc.kind != "purge-epoch" {
        return Err(Unread::Unknown("purge epoch kind".into()));
    }
    let hash = crate::util::hex(&Sha256::digest(text.as_bytes()));
    if !identity(&doc.decision)
        || !crate::purge_file::commit_id(&doc.main)
        || doc.reports.is_empty()
        || doc
            .reports
            .iter()
            .any(|(device, oid)| !identity(device) || !crate::purge_file::commit_id(oid))
        || path != format!("{PREFIX}{}/{hash}.toml", doc.decision)
    {
        return Err(Unread::Malformed(
            "purge epoch identity, quorum or content path".into(),
        ));
    }
    Ok(doc)
}

/// Exact full-report refspec, also needed by welcomes and archive recovery.
pub fn refspec(peer: &str) -> String {
    format!("+{PROOFS}*:refs/pigeon/peers/{peer}/purge-proof/*")
}

/// Check new witnesses before core history admission. Missing report
/// objects are provisional, so a later proof fetch can finish admission.
pub(crate) async fn checks(
    g: &Group,
    after: &pigeon_core::authority::Tree,
    changed: &std::collections::BTreeSet<String>,
    parents: &[String],
) -> Result<BTreeMap<String, Result<(), Unread>>, Error> {
    let mut out = BTreeMap::new();
    for path in changed.iter().filter(|p| p.starts_with(PREFIX)) {
        let Some(entry) = after.get(path) else {
            continue;
        };
        let bytes = g.git.object(&entry.oid).await?;
        let parsed = std::str::from_utf8(&bytes)
            .map_err(|_| Unread::Malformed("epoch encoding".into()))
            .and_then(|text| parse(path, text));
        let value = match parsed {
            Err(e) => Err(e),
            Ok(doc) => match available(g, &doc).await? {
                false => Err(Unread::Unknown("completion proof objects".into())),
                true => match verify(g, &doc, parents).await {
                    Ok(verified) => {
                        retain(g, &verified).await?;
                        Ok(())
                    }
                    Err(Error::Parse { detail, .. }) => Err(Unread::Malformed(detail)),
                    Err(_) => Err(Unread::Unknown("completion proof objects".into())),
                },
            },
        };
        out.insert(path.clone(), value);
    }
    Ok(out)
}

async fn available(g: &Group, doc: &Doc) -> Result<bool, Error> {
    let ids: Vec<_> = doc.reports.values().cloned().collect();
    let objects = g.git.objects(&ids).await?;
    Ok(objects.len() == ids.len() && objects.iter().all(Option::is_some))
}

/// Recheck immutable witnesses from accepted main after restart or a
/// membership change. Several valid witnesses may close the same epoch.
pub async fn collect(g: &Group, main: &str) -> Result<BTreeMap<String, Verified>, Error> {
    collect_inner(g, main, true).await
}

/// Read-only status/preview observation; acceptance already rooted proof
/// objects. No Git reference is created by this reading.
pub async fn read(g: &Group, main: &str) -> Result<BTreeMap<String, Verified>, Error> {
    collect_inner(g, main, false).await
}

async fn collect_inner(
    g: &Group,
    main: &str,
    save: bool,
) -> Result<BTreeMap<String, Verified>, Error> {
    let mut out = BTreeMap::new();
    for (path, entry) in g.git.tree(main).await? {
        if !path.starts_with(PREFIX) {
            continue;
        }
        let bytes = g.git.object(&entry.oid).await?;
        let text = std::str::from_utf8(&bytes).map_err(|_| invalid("epoch encoding"))?;
        let doc = parse(&path, text).map_err(|_| invalid("unknown epoch"))?;
        let verified = verify(g, &doc, &[main.into()]).await?;
        if save {
            retain(g, &verified).await?
        }
        out.entry(doc.decision.clone()).or_insert(verified);
    }
    Ok(out)
}

/// Publish at most one completed epoch per round. This metadata commit
/// uses a private tree and a main CAS; it never stages the user's files.
/// Caller holds `busy`. Unmerged seal histories wait for ordinary accept.
pub async fn close(g: &Group) -> Result<bool, Error> {
    if !g.git.purge_gate.armed() {
        return Ok(false);
    }
    let main = g
        .git
        .rev("refs/heads/main")
        .await
        .ok_or_else(|| invalid("no main"))?;
    let active = crate::purge_state::active(g, &main).await?;
    let closed = collect(g, &main).await?;
    let proofs = crate::purge_proofs::collect(g, &active).await?;
    let custodians = crate::purge_proofs::custodians(g);
    for id in active.keys().filter(|id| !closed.contains_key(*id)) {
        if custodians.is_empty()
            || custodians
                .iter()
                .any(|d| !proofs.contains_key(&(id.clone(), d.clone())))
        {
            continue;
        }
        let mut reports = BTreeMap::new();
        for device in &custodians {
            let seal = &proofs[&(id.clone(), device.clone())];
            if !g.git.contains(&main, &seal.main).await {
                break;
            }
            let name = format!("refs/pigeon/purge-seals/{id}/{device}");
            let oid = g
                .git
                .rev(&name)
                .await
                .ok_or_else(|| invalid("lost seal report root"))?;
            reports.insert(device.clone(), oid);
        }
        if reports.len() != custodians.len() {
            continue;
        }
        let doc = Doc {
            format: 1,
            kind: "purge-epoch".into(),
            decision: id.clone(),
            main: main.clone(),
            reports,
        };
        let verified = verify(g, &doc, std::slice::from_ref(&main)).await?;
        retain(g, &verified).await?;
        let (path, text) = render(&doc).map_err(|e| invalid(&e))?;
        let oid = g
            .git
            .run_in(&["hash-object", "-w", "--stdin"], text.as_bytes())
            .await?;
        let mut tree = g.git.tree(&main).await?;
        tree.insert(
            path,
            pigeon_core::authority::Entry {
                mode: "100644".into(),
                kind: "blob".into(),
                oid: oid.trim().into(),
            },
        );
        let tree = crate::merge::write_tree(&g.git, &tree).await?;
        let commit = g
            .git
            .commit_tree(&tree, &[&main], "pigeon: retain completed erasure proof")
            .await?;
        crate::merge::judge_history(g, std::slice::from_ref(&commit)).await?;
        if !g.lock().engine.accepted(&commit) {
            return Err(invalid("completion witness commit was not accepted"));
        }
        g.git
            .update_ref("refs/heads/main", &commit, Some(&main))
            .await?;
        return Ok(true);
    }
    Ok(false)
}

/// Immutable proof checking is boxed because history admission may itself
/// encounter an older certificate. Closing ancestry makes that recursion
/// strictly historical; no current-clock reading changes its validity.
pub async fn verify(g: &Group, doc: &Doc, accepted_parents: &[String]) -> Result<Verified, Error> {
    Box::pin(verify_inner(g, doc, accepted_parents)).await
}

async fn verify_inner(g: &Group, doc: &Doc, parents: &[String]) -> Result<Verified, Error> {
    let (path, text) = render(doc).map_err(|e| invalid(&e))?;
    parse(&path, &text).map_err(|_| invalid("unknown or malformed completion certificate"))?;
    let mut seen = false;
    for parent in parents {
        let accepted = g.lock().engine.accepted(parent);
        if accepted && g.git.contains(parent, &doc.main).await {
            seen = true;
        }
    }
    if !seen {
        return Err(invalid("closing head was not accepted before this witness"));
    }
    crate::merge::judge_history(g, std::slice::from_ref(&doc.main)).await?;
    if !g.lock().engine.accepted(&doc.main) {
        return Err(invalid("closing head is not accepted"));
    }
    let decisions = crate::purge_state::active(g, &doc.main).await?;
    let active = decisions
        .get(&doc.decision)
        .ok_or_else(|| invalid("decision absent at closing head"))?;
    let bytes = g
        .git
        .blob(&format!("{}:.pigeon/roster.toml", doc.main))
        .await
        .ok_or_else(|| invalid("missing closing roster"))?;
    let roster = crate::roster_file::parse(&String::from_utf8_lossy(&bytes))
        .map_err(|_| invalid("unknown closing roster"))?
        .roster;
    let custodians: std::collections::BTreeSet<String> = roster
        .devices
        .iter()
        .filter(|(_, d)| d.role == Role::Custodian)
        .map(|(id, _)| id.0.clone())
        .collect();
    let introduction = g
        .git
        .blob(&format!("{}:.pigeon/roster.toml", active.introduced))
        .await
        .ok_or_else(|| invalid("missing original decision roster"))?;
    let original = crate::roster_file::parse(&String::from_utf8_lossy(&introduction))
        .map_err(|_| invalid("unknown original decision roster"))?
        .roster;
    let original: std::collections::BTreeSet<String> = original
        .devices
        .iter()
        .filter(|(_, d)| d.role == Role::Custodian)
        .map(|(id, _)| id.0.clone())
        .collect();
    if custodians != original {
        return Err(invalid(
            "closing roster differs from the original decision epoch",
        ));
    }
    if custodians.is_empty()
        || doc
            .reports
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            != custodians
    {
        return Err(invalid(
            "completion does not cover its exact original custodian quorum",
        ));
    }
    let mut seals = Proofs::new();
    for (device, oid) in &doc.reports {
        let raw = g.git.object(oid).await?;
        if pigeon_core::sig::verify(&raw).ok() != Some(DeviceId(device.clone())) {
            return Err(invalid("completion report signature"));
        }
        let bytes = g
            .git
            .blob(&format!("{oid}:status.toml"))
            .await
            .ok_or_else(|| invalid("missing completion report body"))?;
        let crate::report_file::Read::Known(report) =
            crate::report_file::parse(&String::from_utf8_lossy(&bytes))
        else {
            return Err(invalid("unknown completion report"));
        };
        if report.device != *device
            || report.checking
            || (report.lab && !pigeon_sys::lab::LAB)
            || report.custody.as_ref().is_none_or(|c| c.repository != "ok")
            || active
                .doc
                .end()
                .is_none_or(|end| report.at_unix().is_none_or(|at| at < end))
        {
            return Err(invalid("ineligible or provably early completion report"));
        }
        let offered: Vec<_> = report
            .purge
            .iter()
            .filter(|p| p.decision == doc.decision)
            .collect();
        if offered.len() != 1 {
            return Err(invalid("report has no unique decision seal"));
        }
        let seal = offered[0];
        let accepted = g.lock().engine.accepted(&seal.main);
        if !crate::purge_file::commit_id(&seal.main)
            || !accepted
            || !g.git.contains(&doc.main, &seal.main).await
            || !crate::purge_state::active(g, &seal.main)
                .await?
                .contains_key(&doc.decision)
        {
            return Err(invalid(
                "closing head does not contain the exact sealer history",
            ));
        }
        seals.insert(
            (doc.decision.clone(), device.clone()),
            Seal {
                main: seal.main.clone(),
                held: seal.held.iter().cloned().collect(),
            },
        );
    }
    Ok(Verified {
        doc: doc.clone(),
        epoch: Epoch {
            main: doc.main.clone(),
            custodians,
        },
        seals,
    })
}

/// Retain full signed objects before admitting or using their witness.
/// A conflicting local root is preserved and stops the operation.
pub async fn retain(g: &Group, verified: &Verified) -> Result<(), Error> {
    for oid in verified.doc.reports.values() {
        let name = format!("{PROOFS}{oid}");
        match g.git.rev(&name).await {
            Some(found) if found == *oid => {}
            Some(_) => return Err(invalid("completion report root names another object")),
            None => g.git.update_ref(&name, oid, None).await?,
        }
    }
    Ok(())
}
