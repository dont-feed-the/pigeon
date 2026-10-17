//! Durable admission acknowledgements (proposed ADR 0081). Each group has
//! one immutable initial-upgrade choice. Pending-custodian requests can be
//! acknowledged together. Only successful atomic, fsynced checkpoints are
//! exposed to signed reports; unknown or torn ledgers are preserved.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::report_file::{AdmissionAck, AdmissionKind};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Votes {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    watermark: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    upgrade: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    custodians: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileT {
    format: u32,
    groups: BTreeMap<String, Votes>,
}

fn request_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Private staged and last-durable choices. Publication reads only durable.
/// Store this outside the repository being restored; check its durable
/// watermark before a new acknowledgement, seal or physical erasure.
#[derive(Debug)]
pub struct Ledger {
    staged: FileT,
    durable: FileT,
}

impl Ledger {
    /// A missing file starts empty. Any unrecognized existing file is an
    /// error and must not be replaced by a reassuring fresh choice.
    pub fn load(path: &Path) -> Result<Self, String> {
        let file: FileT = match std::fs::read_to_string(path) {
            Ok(text) => toml_edit::de::from_str(&text).map_err(|e| e.to_string())?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => FileT {
                format: 1,
                groups: BTreeMap::new(),
            },
            Err(e) => return Err(e.to_string()),
        };
        if file.format != 1
            || file.groups.iter().any(|(group, votes)| {
                !crate::purge_file::commit_id(group)
                    || ((votes.upgrade.is_some() || !votes.custodians.is_empty())
                        && votes.watermark.is_none())
                    || votes.upgrade.as_ref().is_some_and(|id| !request_id(id))
                    || votes
                        .watermark
                        .as_ref()
                        .is_some_and(|id| !crate::purge_file::commit_id(id))
                    || votes.custodians.iter().any(|id| !request_id(id))
            })
        {
            return Err("unknown or inconsistent admission acknowledgements".into());
        }
        Ok(Self {
            staged: file.clone(),
            durable: file,
        })
    }

    /// Stage a verified pending introduction. A later or recovered signed
    /// proof cannot change an earlier initial-upgrade choice.
    pub fn acknowledge(
        &mut self,
        group: &str,
        kind: AdmissionKind,
        request: &str,
    ) -> Result<(), String> {
        if !crate::purge_file::commit_id(group) || !request_id(request) {
            return Err("invalid admission identity".into());
        }
        let votes = self.staged.groups.entry(group.into()).or_default();
        if votes.watermark.is_none() {
            return Err("checkpoint a verified admission fence before acknowledging".into());
        }
        match kind {
            AdmissionKind::Upgrade => match &votes.upgrade {
                Some(first) if first != request => {
                    return Err("another initial upgrade was already acknowledged".into());
                }
                Some(_) => {}
                None => votes.upgrade = Some(request.into()),
            },
            AdmissionKind::Custodian => {
                votes.custodians.insert(request.into());
            }
        }
        Ok(())
    }

    /// The acknowledged upgrade request, including one staged for saving.
    pub fn upgrade(&self, group: &str) -> Option<&str> {
        self.staged.groups.get(group)?.upgrade.as_deref()
    }

    /// Last durable critical main: an older restored repository must first
    /// recover this history, rather than forgetting a sealing fence.
    pub fn watermark(&self, group: &str) -> Option<&str> {
        self.durable.groups.get(group)?.watermark.as_deref()
    }

    /// Stage a newer verified head only when the adapter proved it
    /// contains the old critical head. The bool is independent Git-ancestry
    /// evidence; equality needs no additional proof.
    pub fn advance(&mut self, group: &str, main: &str, contains_old: bool) -> Result<(), String> {
        if !crate::purge_file::commit_id(group) || !crate::purge_file::commit_id(main) {
            return Err("invalid critical main identity".into());
        }
        let votes = self.staged.groups.entry(group.into()).or_default();
        if votes
            .watermark
            .as_ref()
            .is_some_and(|old| old != main && !contains_old)
        {
            return Err("repository is behind its durable admission fence".into());
        }
        votes.watermark = Some(main.into());
        Ok(())
    }

    /// Completed/cancelled custody requests can stop appearing in reports
    /// once their complete signed proof objects have their own roots. The
    /// group's initial-upgrade choice is never cleared or changed.
    pub fn retain_pending(&mut self, group: &str, requests: &BTreeSet<String>) {
        if let Some(votes) = self.staged.groups.get_mut(group) {
            votes.custodians.retain(|id| requests.contains(id));
        }
    }

    /// Publishable acknowledgements after the last successful checkpoint.
    pub fn acknowledgements(&self, group: &str) -> Vec<AdmissionAck> {
        let Some(votes) = self.durable.groups.get(group) else {
            return Vec::new();
        };
        votes
            .upgrade
            .iter()
            .map(|request| AdmissionAck {
                kind: AdmissionKind::Upgrade,
                request: request.clone(),
                protocol: 2,
            })
            .chain(votes.custodians.iter().map(|request| AdmissionAck {
                kind: AdmissionKind::Custodian,
                request: request.clone(),
                protocol: 2,
            }))
            .collect()
    }

    /// Atomic fsync before any acknowledgement becomes visible.
    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        if self.staged == self.durable {
            return Ok(());
        }
        let text = toml_edit::ser::to_string(&self.staged).map_err(|e| e.to_string())?;
        pigeon_sys::fs::write_atomic(path, text.as_bytes()).map_err(|e| e.to_string())?;
        self.durable = self.staged.clone();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "durable admission ledger fixture")]
    use super::*;

    #[test]
    fn failed_writes_restarts_and_refounding_cannot_change_a_group_choice() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("admission-acks.toml");
        let group = "a".repeat(40);
        let request = "b".repeat(64);
        let other = "c".repeat(64);
        let mut ledger = Ledger::load(&path).unwrap();
        let first_main = "1".repeat(40);
        let later_main = "2".repeat(40);
        ledger.advance(&group, &first_main, false).unwrap();
        assert!(ledger.watermark(&group).is_none());
        ledger
            .acknowledge(&group, AdmissionKind::Upgrade, &request)
            .unwrap();
        assert!(ledger.acknowledgements(&group).is_empty());
        assert!(ledger.save(&dir.path().join("absent/file")).is_err());
        assert!(ledger.acknowledgements(&group).is_empty());
        ledger.save(&path).unwrap();
        let mut resumed = Ledger::load(&path).unwrap();
        assert_eq!(resumed.watermark(&group), Some(first_main.as_str()));
        assert!(resumed.advance(&group, &later_main, false).is_err());
        assert_eq!(resumed.watermark(&group), Some(first_main.as_str()));
        resumed.advance(&group, &later_main, true).unwrap();
        assert_eq!(resumed.acknowledgements(&group).len(), 1);
        assert!(
            resumed
                .acknowledge(&group, AdmissionKind::Upgrade, &other)
                .is_err()
        );
        assert_eq!(resumed.upgrade(&group), Some(request.as_str()));
        resumed
            .acknowledge(&group, AdmissionKind::Custodian, &other)
            .unwrap();
        resumed.save(&path).unwrap();
        assert_eq!(resumed.watermark(&group), Some(later_main.as_str()));
        assert_eq!(resumed.acknowledgements(&group).len(), 2);
        resumed.retain_pending(&group, &BTreeSet::new());
        resumed.save(&path).unwrap();
        assert_eq!(resumed.acknowledgements(&group).len(), 1);
        assert_eq!(resumed.upgrade(&group), Some(request.as_str()));
        let refounded = "d".repeat(40);
        resumed.advance(&refounded, &"3".repeat(40), false).unwrap();
        resumed
            .acknowledge(&refounded, AdmissionKind::Upgrade, &other)
            .unwrap();
        resumed.save(&path).unwrap();
        assert_eq!(resumed.upgrade(&refounded), Some(other.as_str()));
        assert_eq!(resumed.upgrade(&group), Some(request.as_str()));
    }

    #[test]
    fn an_unknown_or_torn_ack_file_is_never_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("admission-acks.toml");
        for text in [
            "format = 2\n[groups]\n",
            "format = 1\nfuture = true\n[groups]\n",
            "format = [torn",
            &format!(
                "format = 1\n[groups.{}]\nupgrade = \"{}\"\n",
                "a".repeat(40),
                "b".repeat(64)
            ),
        ] {
            std::fs::write(&path, text).unwrap();
            assert!(Ledger::load(&path).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        }
    }
}
