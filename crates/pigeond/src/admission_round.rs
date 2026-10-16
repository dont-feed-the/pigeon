//! Admission acknowledgements are checkpointed outside restored groups,
//! before they enter signed status reports (proposed ADR 0075). One store
//! per daemon serializes groups, avoiding lost updates to their markers.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::admission_receipts::Ledger;
use crate::git::Error;
use crate::group::Group;
use crate::membership::{self, invalid};
use crate::report_file::{AdmissionKind, Read};

/// Private process-wide receipt file. An unreadable existing file is kept.
#[derive(Debug)]
pub struct Store {
    path: PathBuf,
    ledger: tokio::sync::Mutex<Result<Ledger, String>>,
    recovered: std::sync::Mutex<BTreeSet<String>>,
}

impl Store {
    /// Construct once from the daemon state directory, never a group path.
    pub fn open(path: PathBuf) -> Self {
        let ledger = tokio::sync::Mutex::new(Ledger::load(&path));
        Self {
            path,
            ledger,
            recovered: Default::default(),
        }
    }

    /// Caller holds the group's `busy`. A failure leaves an explicit
    /// checking state and blocks content, seals and physical erasure.
    pub async fn round(&self, g: &Group, me: &str) {
        if let Err(error) = self.acknowledge(g, me).await {
            let mut known = g.lock();
            known.admission_acknowledgements.clear();
            known.purge_seals.clear();
            known.admission_error = Some(error.to_string());
            drop(known);
            g.git.purge_gate.suspend();
            tracing::warn!(group = %g.id, error = %error, "admission-waits");
        }
    }

    async fn acknowledge(&self, g: &Group, me: &str) -> Result<(), Error> {
        let main = g
            .git
            .rev("refs/heads/main")
            .await
            .ok_or_else(|| invalid("no accepted main"))?;
        crate::merge::judge_history(g, std::slice::from_ref(&main)).await?;
        let state = membership::read(g, &main).await?;
        {
            let mut known = g.lock();
            known.membership = Some((main.clone(), state.clone()));
        }
        let mut guard = self.ledger.lock().await;
        let ledger = guard.as_mut().map_err(|e| invalid(e))?;
        let recovered = self
            .recovered
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains(&g.id);
        if !recovered {
            recover(g, me, ledger, &self.path).await?;
            self.recovered
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .insert(g.id.clone());
        }
        let critical = state.upgrade.is_some()
            || !state.requests.is_empty()
            || ledger.watermark(&g.id).is_some();
        if !critical {
            g.lock().admission_error = None;
            return Ok(());
        }
        let contains = match ledger.watermark(&g.id) {
            Some(old) => g.git.contains(&main, old).await,
            None => true,
        };
        if !contains {
            return Err(invalid(
                "recover metadata containing the durable admission fence before continuing",
            ));
        }
        ledger
            .advance(&g.id, &main, contains)
            .map_err(|e| invalid(&e))?;
        let pending: BTreeSet<_> = state
            .pending
            .values()
            .map(|r| r.id())
            .collect::<Result<_, _>>()
            .map_err(|e| invalid(&e))?;
        if state.upgrade.is_none() {
            let chosen = ledger
                .upgrade(&g.id)
                .map(str::to_owned)
                .or_else(|| state.requests.keys().next().cloned());
            if let Some(chosen) = chosen {
                let request = state.requests.get(&chosen).ok_or_else(|| {
                    invalid("recover the previously acknowledged upgrade request")
                })?;
                let base = membership::roster(g, &request.base).await?;
                let current = membership::roster(g, &main).await?;
                if membership::participants(&base) != membership::participants(&current)
                    || membership::configured(&base) != membership::configured(&current)
                {
                    return Err(invalid(
                        "initial upgrade membership changed; keep erasure disabled",
                    ));
                }
                ledger
                    .acknowledge(&g.id, AdmissionKind::Upgrade, &chosen)
                    .map_err(|e| invalid(&e))?;
            }
        }
        for request in &pending {
            ledger
                .acknowledge(&g.id, AdmissionKind::Custodian, request)
                .map_err(|e| invalid(&e))?;
        }
        ledger.save(&self.path).map_err(|e| invalid(&e))?;
        // The new critical main is durable before old pending fences are
        // pruned. Restoring a repository alone cannot forget those fences.
        ledger.retain_pending(&g.id, &pending);
        ledger.save(&self.path).map_err(|e| invalid(&e))?;
        let mut known = g.lock();
        known.admission_fence = ledger.watermark(&g.id).map(str::to_owned);
        known.admission_acknowledgements = ledger.acknowledgements(&g.id);
        known.admission_error = None;
        Ok(())
    }
}

/// Recover only the device's own signed assertions from this group's
/// history. Ref names alone, another group's status, and unsigned files
/// cannot reset a choice. Ancestor reports never roll the marker back.
async fn recover(
    g: &Group,
    me: &str,
    ledger: &mut Ledger,
    path: &std::path::Path,
) -> Result<(), Error> {
    let prefix = format!("{}{}/", crate::admission_proofs::ACKS, g.id);
    let refs = g
        .git
        .run(&[
            "for-each-ref",
            "--format=%(objectname)",
            &prefix,
            crate::purge_epoch::PROOFS,
        ])
        .await?;
    for oid in refs.lines() {
        let raw = g.git.object(oid).await?;
        if pigeon_core::sig::verify(&raw)
            .ok()
            .as_ref()
            .map(|d| d.0.as_str())
            != Some(me)
        {
            continue;
        }
        let Some(bytes) = g.git.blob(&format!("{oid}:status.toml")).await else {
            continue;
        };
        let Read::Known(report) = crate::report_file::parse(&String::from_utf8_lossy(&bytes))
        else {
            continue;
        };
        if report.device != me || report.acknowledgements.is_empty() {
            continue;
        }
        let Some(main) = report.main else {
            return Err(invalid("own acknowledgement lost its main"));
        };
        if g.git.root(&main).await.as_deref() != Some(g.id.as_str()) {
            continue;
        }
        let advance = match ledger.watermark(&g.id) {
            Some(old) if g.git.contains(old, &main).await => false,
            Some(old) if !g.git.contains(&main, old).await => {
                return Err(invalid("own signed admission fences conflict"));
            }
            _ => true,
        };
        if advance {
            ledger
                .advance(&g.id, &main, true)
                .map_err(|e| invalid(&e))?
        }
        for ack in report.acknowledgements {
            if ack.protocol != 2 {
                return Err(invalid("own acknowledgement has unknown protocol"));
            }
            ledger
                .acknowledge(&g.id, ack.kind, &ack.request)
                .map_err(|e| invalid(&e))?;
        }
        ledger.save(path).map_err(|e| invalid(&e))?;
    }
    Ok(())
}
