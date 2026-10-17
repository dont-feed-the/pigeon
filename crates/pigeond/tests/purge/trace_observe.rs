//! Observations are read back from admitted main refs, signed report
//! objects and real annex files. No abstract Purge state is maintained by
//! the fixture; TLC derives its clocks, causal facts and safety ghosts.

use super::*;
use pigeond::report_file::{Purge, Read};

impl World {
    pub(super) async fn observe(&mut self, action: &str, from: usize, to: usize) {
        let mut nodes = Vec::new();
        for i in 0..3 {
            let g = self.group(i);
            let main = g.git.rev("refs/heads/main").await.unwrap();
            assert!(g.lock().engine.accepted(&main));
            let d = g.git.contains(&main, &self.decision).await;
            let r1 = match &self.restored {
                Some(restored) => g.git.contains(&main, restored).await,
                None => false,
            };
            let cancel = match &self.cancelled {
                Some(cancelled) => g.git.contains(&main, cancelled).await,
                None => false,
            };
            let seals = reports(g).await;
            let own = seals.get(&dev(NUMBERS[i]).0);
            let seal_restore = match (own, self.restored.as_ref()) {
                (Some(seal), Some(restored)) => g.git.contains(&seal.main, restored).await,
                _ => false,
            };
            let seal_cancel = match (own, self.cancelled.as_ref()) {
                (Some(seal), Some(cancelled)) => g.git.contains(&seal.main, cancelled).await,
                _ => false,
            };
            assert!(
                own.is_none_or(|s| s.held.is_empty()),
                "this trace has no references outside main"
            );
            let known: Vec<_> = (1..3)
                .filter(|j| seals.contains_key(&dev(NUMBERS[*j]).0))
                .map(|j| IDS[j])
                .collect();
            let object = pigeond::custody::present(g, &self.key).await;
            if let Some(object) = &object {
                assert_eq!(std::fs::read(object).unwrap(), BYTES);
            }
            nodes.push(json!({ "id": IDS[i], "decision": d, "restored": r1,
                "cancelled": cancel, "sealed": own.is_some(),
                "seal_restore": seal_restore,
                "seal_cancel": seal_cancel, "reports": known, "present": object.is_some() }));
        }
        self.events
            .push(json!({ "spec": "Purge", "device": IDS[from],
            "clock": self.events.len(), "action": action,
            "vars": { "from": IDS[from], "to": IDS[to], "nodes": nodes } }));
    }
}

/// Observe actual authenticated received reports even while their main
/// has not arrived. Production's proof reader must still refuse release
/// until that main is admitted; the trace checks that separate boundary.
async fn reports(g: &Group) -> BTreeMap<String, Purge> {
    let refs = g
        .git
        .run(&[
            "for-each-ref",
            "--format=%(objectname)",
            "refs/pigeon/status/",
            "refs/pigeon/purge-seals/",
        ])
        .await
        .unwrap();
    let mut seals = BTreeMap::new();
    for oid in refs.lines() {
        let raw = g.git.object(oid).await.unwrap();
        let signer = sig::verify(&raw).unwrap();
        let bytes = g.git.blob(&format!("{oid}:status.toml")).await.unwrap();
        let Read::Known(report) = pigeond::report_file::parse(std::str::from_utf8(&bytes).unwrap())
        else {
            panic!("unknown own fixture report")
        };
        assert_eq!(report.device, signer.0);
        assert!(!report.checking);
        assert!(
            report
                .custody
                .as_ref()
                .is_some_and(|c| c.repository == "ok")
        );
        for seal in report.purge {
            if let Some(old) = seals.insert(signer.0.clone(), seal.clone()) {
                assert_eq!(old, seal, "signed seals must remain immutable");
            }
        }
    }
    seals
}
