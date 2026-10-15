//! What a device moves first, and what a custodian takes with the room it
//! has (08 §8.9, 15 §15.2): the protect items of `main`, in queue order,
//! planned by `pigeon_core::capacity::admit`.
//!
//! The room is the free space above the reserve less the downloads in
//! progress, each a partial file in `.git/annex/tmp` that still needs its
//! key's size less its length, as git-annex counts them
//! (`sizeOfDownloadsInProgress` in `Annex/Content/LowLevel.hs`). A key is
//! endangered while a removed custodian held it and fewer than K custodians
//! do (08 §8.4): repair records the keys a custodian held as it marks it
//! dead, in `endangered.json`, and a key leaves once K custodians hold it.
//! What a custodian defers is its shortfall, which `doctor` and `status`
//! report as `no-capacity`. Introduction times are not read yet: every item
//! counts as introduced at once, so among equals the smallest goes first.

use std::collections::BTreeSet;
use std::path::Path;

use pigeon_core::capacity::{Admits, Plan, admit, key_size, room};
use pigeon_core::filter::Role;
use pigeon_core::queue::Work;
use pigeon_core::roster::Role as DeviceRole;
use pigeon_core::time::Time;
use pigeon_proto::api::Arg;

use crate::custody::{holders, peers};
use crate::group::Group;

/// What a custodian could not take in its last round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shortfall {
    /// The keys deferred.
    pub keys: usize,
    /// The bytes of free space missing for all of them (08 §8.9).
    pub missing_bytes: u64,
    /// Whether it keeps its room for endangered keys (06 §6.3 `admits`).
    pub admits: Admits,
}

/// The arguments of `no-capacity`: the files deferred and the free space
/// they miss, in MB rounded up.
pub fn no_capacity_args(s: &Shortfall) -> [(&'static str, Arg); 2] {
    let n = |x: u64| Arg::Number(i64::try_from(x).unwrap_or(i64::MAX));
    [
        ("files", n(u64::try_from(s.keys).unwrap_or(u64::MAX))),
        ("missing_mb", n(s.missing_bytes.div_ceil(1_000_000))),
    ]
}

/// The bytes the downloads in progress under `git_dir` still need.
pub fn in_progress(git_dir: &Path) -> u64 {
    let Ok(dir) = std::fs::read_dir(git_dir.join("annex").join("tmp")) else {
        return 0;
    };
    dir.flatten()
        .filter_map(|e| {
            let size = key_size(e.file_name().to_str()?)?;
            Some(size.saturating_sub(e.metadata().ok()?.len()))
        })
        .sum()
}

/// The room of `g`'s file system: free space above the reserve, less the
/// downloads in progress. One that cannot be measured is left to
/// git-annex's own reserve check.
pub fn room_of(g: &Group) -> u64 {
    let Ok(space) = pigeon_sys::fs::space(&g.git_dir) else {
        return u64::MAX;
    };
    let reserve = crate::settings::reserve_bytes(g.role, &g.git_dir);
    room(space.available, reserve, in_progress(&g.git_dir))
}

fn endangered_file(g: &Group) -> std::path::PathBuf {
    g.root().join("endangered.json")
}

/// The keys recorded as endangered.
pub fn endangered(g: &Group) -> BTreeSet<String> {
    std::fs::read_to_string(endangered_file(g))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Records `keys` as endangered, with those already recorded.
pub fn save_endangered(g: &Group, keys: &BTreeSet<String>) -> Result<(), String> {
    let text = serde_json::to_vec(keys).map_err(|e| e.to_string())?;
    pigeon_sys::fs::write_atomic(&endangered_file(g), &text).map_err(|e| e.to_string())
}

/// The protect items of `keys` this device lacks (a custodian) or that some
/// custodian lacks (a laptop), in queue order, each with the custodians
/// that lack it; endangered keys that K custodians now hold are forgotten.
pub async fn items(g: &Group, me: &str, keys: &BTreeSet<String>) -> Vec<(Work, Vec<String>)> {
    let k = crate::status::settings(g).k;
    let peers = peers(g, me);
    let uuids = |role| -> Vec<(String, String)> {
        peers
            .iter()
            .filter(|(_, _, r)| *r == role)
            .map(|(id, u, _)| (id.clone(), u.clone()))
            .collect()
    };
    let (mut custodians, laptops) = (uuids(DeviceRole::Custodian), uuids(DeviceRole::Laptop));
    let own = g.lock().annex_uuid.clone();
    if g.role == Role::Custodian
        && let Some(u) = own
    {
        custodians.push((me.to_owned(), u));
    }
    let mut endangered = endangered(g);
    let recorded = endangered.len();
    endangered.retain(|key| keys.contains(key));
    let mut out = Vec::new();
    for key in keys {
        if g.role == Role::Custodian
            && !endangered.contains(key)
            && crate::custody::present(g, key).await.is_some()
        {
            continue;
        }
        let have = holders(&g.git, key).await;
        let lacking: Vec<String> = custodians
            .iter()
            .filter(|(_, u)| !have.contains(u))
            .map(|(id, _)| id.clone())
            .collect();
        let custody = custodians.len() - lacking.len();
        if custody >= k {
            endangered.remove(key);
        }
        let wanted = match g.role {
            Role::Custodian => lacking.iter().any(|id| id == me),
            Role::Laptop => !lacking.is_empty(),
        };
        if !wanted {
            continue;
        }
        let work = Work::Protect {
            key: key.clone(),
            custody_copies: u32::try_from(custody).unwrap_or(u32::MAX),
            laptop_copies: u32::try_from(laptops.iter().filter(|(_, u)| have.contains(u)).count())
                .unwrap_or(u32::MAX),
            endangered: endangered.contains(key),
            introduced: Time(0),
            size: key_size(key).unwrap_or(0),
        };
        out.push((work, lacking));
    }
    if endangered.len() != recorded
        && let Err(error) = save_endangered(g, &endangered)
    {
        tracing::warn!(group = %g.id, %error, "endangered-save-failed");
    }
    out.sort_by(|a, b| pigeon_core::queue::compare(&a.0, &b.0));
    out
}

/// A custodian's plan for the keys of `main` it lacks: what to get, in
/// queue order, and what waits; the shortfall is kept for `doctor`.
pub async fn plan(g: &Group, me: &str, keys: &BTreeSet<String>) -> Plan {
    let items: Vec<Work> = items(g, me, keys)
        .await
        .into_iter()
        .map(|(w, _)| w)
        .collect();
    let plan = admit(room_of(g), &items);
    let shortfall = (!plan.deferred.is_empty()).then(|| Shortfall {
        keys: plan.deferred.len(),
        missing_bytes: plan.missing_bytes(),
        admits: plan.admits(),
    });
    if let Some(s) = &shortfall {
        tracing::info!(group = %g.id, keys = s.keys, missing_bytes = s.missing_bytes,
            room = plan.room, "admission-deferred");
    }
    g.lock().shortfall = shortfall;
    plan
}
