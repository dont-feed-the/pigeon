//! Durable receiver waits (08 §8.6, 11 §11.3). Elapsed monotonic uptime is
//! checkpointed, never inferred from wall-clock differences or downtime.
//! A crash can lengthen the wait; neither a restart nor a clock jump can
//! shorten it. Only durably written seals are exposed for publication.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::report_file::Purge;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    elapsed_nanos: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    seal: Option<Purge>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileT {
    format: u32,
    receipts: BTreeMap<String, Receipt>,
}

/// One custodian's receiver ledger, accessed under the group lock.
#[derive(Debug)]
pub struct Ledger {
    file: FileT,
    durable: FileT,
    tick: Instant,
    saved_at: Instant,
}

impl Ledger {
    /// Unknown or malformed existing files stop sealing and are never
    /// overwritten. A missing ledger conservatively starts a fresh wait.
    pub fn load(path: &Path) -> Result<Self, String> {
        let file = match std::fs::read_to_string(path) {
            Ok(text) => {
                let f: FileT = toml_edit::de::from_str(&text).map_err(|e| e.to_string())?;
                if f.format != 1
                    || f.receipts.iter().any(|(id, r)| {
                        r.elapsed_nanos > i64::MAX as u64
                            || r.seal.as_ref().is_some_and(|seal| seal.decision != *id)
                    })
                {
                    return Err("unknown or inconsistent purge receipts".into());
                }
                f
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => FileT {
                format: 1,
                receipts: BTreeMap::new(),
            },
            Err(e) => return Err(e.to_string()),
        };
        Ok(Self {
            durable: file.clone(),
            file,
            tick: Instant::now(),
            saved_at: Instant::now(),
        })
    }

    fn advance(&mut self, now: Instant) {
        let elapsed = u64::try_from(now.saturating_duration_since(self.tick).as_nanos())
            .unwrap_or(i64::MAX as u64);
        for receipt in self.file.receipts.values_mut() {
            receipt.elapsed_nanos = receipt
                .elapsed_nanos
                .saturating_add(elapsed)
                .min(i64::MAX as u64);
        }
        self.tick = now;
    }

    /// Notice active decisions after counting time only for receipts that
    /// already existed. A newly seen decision starts at zero now.
    pub fn observe(&mut self, active: &BTreeSet<String>) {
        self.observe_at(active, Instant::now());
    }

    fn observe_at(&mut self, active: &BTreeSet<String>, now: Instant) {
        self.advance(now);
        self.file.receipts.retain(|id, _| active.contains(id));
        for id in active {
            self.file.receipts.entry(id.clone()).or_default();
        }
    }

    /// Whole monotonic seconds since this identity was first received.
    pub fn elapsed(&self, id: &str) -> u64 {
        self.file
            .receipts
            .get(id)
            .map_or(0, |r| r.elapsed_nanos / 1_000_000_000)
    }

    /// Stage an immutable seal. It becomes publishable only after a
    /// successful forced checkpoint. A contradictory second seal fails.
    pub fn seal(&mut self, seal: Purge) -> Result<(), String> {
        let Some(receipt) = self.file.receipts.get_mut(&seal.decision) else {
            return Err("purge was not received".into());
        };
        if let Some(old) = &receipt.seal {
            if old != &seal {
                return Err("purge seal is immutable".into());
            }
        } else {
            receipt.seal = Some(seal);
        }
        Ok(())
    }

    /// The last durable seals; an I/O failure cannot create an ack.
    pub fn seals(&self) -> Vec<Purge> {
        self.durable
            .receipts
            .values()
            .filter_map(|r| r.seal.clone())
            .collect()
    }

    /// At most one ordinary checkpoint per minute, plus explicit receipt,
    /// cancellation and seal boundaries. Every write is atomic and fsynced.
    pub fn save(&mut self, path: &Path, force: bool) -> Result<(), String> {
        if !force && self.saved_at.elapsed() < Duration::from_secs(60) {
            return Ok(());
        }
        self.advance(Instant::now());
        if self.file != self.durable {
            let text = toml_edit::ser::to_string(&self.file).map_err(|e| e.to_string())?;
            pigeon_sys::fs::write_atomic(path, text.as_bytes()).map_err(|e| e.to_string())?;
            self.durable = self.file.clone();
        }
        self.saved_at = Instant::now();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::unwrap_used, reason = "durable ledger fixture")]
    fn restarts_cancellation_and_failed_writes_cannot_ack_early() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("purges.toml");
        let mut ledger = Ledger::load(&path).unwrap();
        let active = ["commit:path".to_owned()].into();
        ledger.observe(&active);
        let later = ledger.tick + Duration::from_secs(123);
        ledger.observe_at(&active, later);
        assert_eq!(ledger.elapsed("commit:path"), 123);
        // Rebase the test clock before persistence; no real sleep or wall
        // clock manipulation stands in for an observed retention period.
        ledger.tick = Instant::now();
        ledger.save(&path, true).unwrap();
        let mut resumed = Ledger::load(&path).unwrap();
        assert_eq!(resumed.elapsed("commit:path"), 123);
        resumed
            .seal(Purge {
                decision: "commit:path".into(),
                main: "main".into(),
                held: vec![],
            })
            .unwrap();
        assert!(resumed.seals().is_empty());
        assert!(
            resumed
                .save(&temp.path().join("absent/ledger"), true)
                .is_err()
        );
        assert!(resumed.seals().is_empty());
        resumed.save(&path, true).unwrap();
        assert_eq!(resumed.seals().len(), 1);
        resumed.observe(&BTreeSet::new());
        resumed.observe(&["new-commit:path".into()].into());
        assert_eq!(resumed.elapsed("new-commit:path"), 0);
        resumed.save(&path, true).unwrap();
        assert!(resumed.seals().is_empty());
    }

    #[test]
    #[allow(clippy::unwrap_used, reason = "unknown ledger fixture")]
    fn a_newer_or_torn_file_is_preserved() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("purges.toml");
        for text in [
            "format = 2\n[receipts]\n",
            "format = 1\nfuture = true\n[receipts]\n",
            "format = [torn",
        ] {
            std::fs::write(&path, text).unwrap();
            assert!(Ledger::load(&path).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        }
    }
}
