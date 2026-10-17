//! One key per native batch exchange (15 §15.2). Reuse the annex/helper
//! between keys without queuing future authority decisions in its stdin.
//! The caller revalidates each key; the transport still gates every byte.

use std::collections::BTreeMap;
use std::time::Duration;

use pigeon_git::annex::Answer;
use pigeon_git::batch::Batch;

use crate::git::Git;

#[derive(Default)]
pub(crate) struct Pool(BTreeMap<String, Batch>);

impl Pool {
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
        if !self.0.contains_key(session) {
            self.0.insert(
                session.to_owned(),
                git.repo.batch(args).map_err(|e| e.to_string())?,
            );
        }
        let batch = self.0.get_mut(session).ok_or("missing transfer session")?;
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
        for batch in self.0.values_mut() {
            batch.stop(Duration::from_secs(5)).await;
        }
    }
}
