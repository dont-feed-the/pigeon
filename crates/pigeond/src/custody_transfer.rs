//! One key per native batch exchange (15 §15.2). Reuse the annex/helper
//! between keys without queuing future authority decisions in its stdin.
//! The caller revalidates each key; the transport still gates every byte.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use pigeon_git::annex::Answer;
use pigeon_git::batch::Batch;
use pigeon_git::work::{Class, Control};

use crate::git::Git;

// Batches drop before their pending registration, including cancellation.
#[derive(Default)]
pub(crate) struct Pool {
    batches: BTreeMap<String, Batch>,
    locations: Option<Locations>,
    pending: Option<Pending>,
}

// One worker owns this process. Results are never cached, and the native
// session is rotated before it exceeds the same metadata input bounds.
struct Locations {
    repository: std::path::PathBuf,
    batch: Batch,
    queries: usize,
    bytes: usize,
}

struct Pending(Arc<dyn Control>);

impl Drop for Pending {
    fn drop(&mut self) {
        self.0.ended(Class::Transfer);
    }
}

impl Pool {
    pub(crate) async fn local_object(
        &mut self,
        git: &Git,
        key: &str,
    ) -> Option<std::path::PathBuf> {
        self.local_object_for(git, key, pigeon_git::DEFAULT_TIMEOUT)
            .await
    }

    pub(crate) async fn local_object_for(
        &mut self,
        git: &Git,
        key: &str,
        timeout: Duration,
    ) -> Option<std::path::PathBuf> {
        let bytes = key.len().saturating_add(1);
        if self.locations.as_ref().is_none_or(|state| {
            state.repository != git.repo.dir
                || state.queries >= crate::custody::METADATA_KEYS
                || (state.queries > 0
                    && state.bytes.saturating_add(bytes) > crate::custody::METADATA_INPUT_BYTES)
        }) {
            self.locations = None;
            self.locations = Some(Locations {
                repository: git.repo.dir.clone(),
                batch: git
                    .repo
                    .batch(&["annex", "contentlocation", "--batch"])
                    .ok()?,
                queries: 0,
                bytes: 0,
            });
        }
        let state = self.locations.as_mut()?;
        state.queries += 1;
        state.bytes = state.bytes.saturating_add(bytes);
        let answer = state.batch.call(key, timeout).await.ok()?;
        crate::custody::batch_object_for(git, key, &answer, timeout).await
    }

    pub(crate) async fn copy(
        &mut self,
        git: &Git,
        remote: &str,
        key: &str,
        timeout: Duration,
    ) -> Result<(), String> {
        self.ask(
            git,
            remote,
            &["annex", "copy", "--batch-keys", "--json", "--to", remote],
            key,
            timeout,
        )
        .await
    }

    pub(crate) async fn get(
        &mut self,
        git: &Git,
        key: &str,
        timeout: Duration,
    ) -> Result<(), String> {
        self.ask(
            git,
            "",
            &["annex", "get", "--batch-keys", "--json"],
            key,
            timeout,
        )
        .await
    }

    async fn ask(
        &mut self,
        git: &Git,
        session: &str,
        args: &[&str],
        key: &str,
        timeout: Duration,
    ) -> Result<(), String> {
        // A completed key is not the end of this queue. Keep its class
        // visible while the caller revalidates the remaining keys. Use the
        // repository's view so optional downloads retain their lower class.
        if self.pending.is_none() {
            self.pending = git.repo.work_control().map(|control| {
                control.began(Class::Transfer);
                Pending(control)
            });
        }
        if !self.batches.contains_key(session) {
            self.batches.insert(
                session.to_owned(),
                git.repo.batch(args).map_err(|e| e.to_string())?,
            );
        }
        let batch = self
            .batches
            .get_mut(session)
            .ok_or("missing transfer session")?;
        let answer: Answer = batch
            .call_json(key, timeout)
            .await
            .map_err(|e| e.to_string())?
            .ok_or("missing transfer answer")?;
        if answer.success && answer.key.as_deref() == Some(key) {
            Ok(())
        } else {
            Err(format!("transfer answer refused or mismatched: {answer:?}"))
        }
    }

    pub(crate) async fn stop(&mut self) {
        for batch in self.batches.values_mut() {
            batch.stop(Duration::from_secs(5)).await;
        }
        if let Some(locations) = &mut self.locations {
            locations.batch.stop(Duration::from_secs(5)).await;
        }
        self.locations = None;
        self.pending = None;
    }
}

#[cfg(test)]
#[path = "custody_location_tests.rs"]
mod location_tests;
