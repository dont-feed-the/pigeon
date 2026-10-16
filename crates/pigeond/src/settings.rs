//! A repository's settings (03 §3.3, §3.4): set when it is made, and put
//! back by the settings check of every round (08 §8.1), which compares
//! `.git/config` and `.git/info/attributes` with what the role asks.
//!
//! Group-wide and per-device git-annex settings (trust, groups, `wanted`,
//! numcopies) live in the git-annex branch and are written once, when the
//! repository is made.

use std::collections::BTreeMap;
use std::path::Path;

use pigeon_core::filter::Role;

use crate::git::{Error, Git};
use crate::group::Group;

/// `.git/info/attributes`, on every device (03 §3.7).
pub const INFO_ATTRIBUTES: &str = "* filter=annex annex.largefiles=anything annex.backend=SHA256E -text -eol -ident -working-tree-encoding\n/.gitattributes annex.largefiles=nothing\n/.pigeon/** annex.largefiles=nothing\n";

/// `.gitattributes`, in the genesis commit.
pub const GITATTRIBUTES: &str = "* annex.largefiles=anything\n.gitattributes annex.largefiles=nothing\n.pigeon/** annex.largefiles=nothing\n";

/// The `annex.diskreserve` of a role in bytes on the file system holding
/// `dir`, max(1 GB, 1 %) on laptops and max(5 GB, 2 %) on custodians, which
/// `wanted_config` sets and pigeon checks itself before it receives
/// content (03 §3.4, 08 §8.9). A file system it cannot measure gets the
/// floor.
pub fn reserve_bytes(role: Role, dir: &Path) -> u64 {
    let total = pigeon_sys::fs::space(dir).map_or(0, |s| s.total);
    pigeon_core::capacity::reserve(role, total)
}

/// The `.git/config` values of a role (03 §3.4).
/// `reserve` is the `annex.diskreserve` in bytes ([`reserve_bytes`]).
pub fn wanted_config(role: Role, empty_file: &str, reserve: u64) -> Vec<(String, String)> {
    let mut v: Vec<(&str, String)> = vec![
        ("annex.autocommit", "false".into()),
        ("annex.synccontent", "false".into()),
        ("annex.resolvemerge", "false".into()),
        ("annex.dotfiles", "true".into()),
        ("annex.backend", "SHA256E".into()),
        ("core.attributesFile", empty_file.to_owned()),
        ("core.excludesFile", empty_file.to_owned()),
        ("core.fsync", "objects,reference".into()),
    ];
    match role {
        Role::Laptop => v.extend([
            ("annex.addunlocked", "true".into()),
            ("annex.thin", "true".into()),
            ("annex.jobs", "1".into()),
            ("annex.diskreserve", reserve.to_string()),
            ("core.sparseCheckout", "true".into()),
        ]),
        Role::Custodian => {
            let cpus = std::thread::available_parallelism().map_or(1, usize::from);
            v.extend([
                ("annex.diskreserve", reserve.to_string()),
                ("annex.jobs", cpus.to_string()),
            ]);
        }
    }
    v.into_iter().map(|(k, x)| (k.to_owned(), x)).collect()
}

async fn local_config(git: &Git) -> BTreeMap<String, String> {
    let out = git
        .run(&["config", "--local", "--list"])
        .await
        .unwrap_or_default();
    out.lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.to_ascii_lowercase(), v.to_owned()))
        .collect()
}

/// Puts back what differs; answers how many settings it changed.
pub async fn check(g: &Group) -> usize {
    let empty = g.root().join("empty");
    if !empty.exists() {
        let _ = std::fs::write(&empty, b"");
    }
    let mut changed = 0;
    let current = local_config(&g.git).await;
    let reserve = reserve_bytes(g.role, &g.git_dir);
    let immutable =
        g.shared.is_some() || !matches!(crate::shared_pool::registration(&g.git_dir), Ok(None));
    for (k, mut v) in wanted_config(g.role, &empty.to_string_lossy(), reserve) {
        // Shared objects must never become writable working-tree hard links.
        if immutable && k == "annex.thin" {
            v = "false".into();
        }
        if current.get(&k.to_ascii_lowercase()) != Some(&v)
            && g.git.run(&["config", &k, &v]).await.is_ok()
        {
            changed += 1;
        }
    }
    let attributes = g.git_dir.join("info").join("attributes");
    if std::fs::read_to_string(&attributes).ok().as_deref() != Some(INFO_ATTRIBUTES) {
        let _ = std::fs::create_dir_all(g.git_dir.join("info"));
        if pigeon_sys::fs::write_atomic(&attributes, INFO_ATTRIBUTES.as_bytes()).is_ok() {
            changed += 1;
        }
    }
    if g.role == Role::Laptop {
        let sparse = g.git_dir.join("info").join("sparse-checkout");
        if !sparse.exists() && pigeon_sys::fs::write_atomic(&sparse, b"/*\n").is_ok() {
            changed += 1;
        }
    }
    if changed > 0 {
        tracing::info!(group = %g.id, changed, "settings-check");
    }
    changed
}

/// Makes a repository's git-annex side: `annex init`, its trust, group and
/// `wanted` (03 §3.4), then enables the iroh P2P network and answers the
/// annex UUID and the token git-annex accepts (09 §9.3).
pub async fn init_annex(
    git: &Git,
    role: Role,
    description: &str,
    k: u32,
) -> Result<(String, String), Error> {
    git.run(&["annex", "init", description]).await?;
    match role {
        Role::Laptop => {
            git.run(&["annex", "untrust", "here"]).await?;
            git.run(&["annex", "group", "here", "laptop"]).await?;
        }
        Role::Custodian => {
            git.run(&["annex", "semitrust", "here"]).await?;
            git.run(&["annex", "group", "here", "custodian"]).await?;
            let wanted = format!("sizebalanced=custodian:{k}");
            git.run(&["annex", "wanted", "here", &wanted]).await?;
        }
    }
    git.run(&["annex", "p2p", "--enable", "iroh"]).await?;
    let addresses = git.run(&["annex", "p2p", "--gen-addresses"]).await?;
    let token = addresses
        .lines()
        .find(|l| l.starts_with("p2p-annex::iroh:"))
        .and_then(|l| l.rsplit_once(':'))
        .map(|(_, t)| t.trim().to_owned())
        .unwrap_or_default();
    let uuid = git.config("annex.uuid").await.unwrap_or_default();
    Ok((uuid, token))
}

/// Adds or updates the git remote of a peer: its URL through the helper,
/// its annex UUID, and a refspec that fetches its branches where accept
/// reads them, so remotedaemon's fetches of a notice serve too (09 §9.3,
/// 03 §3.6). Roster peers stay enabled: Remote/Git.hs of git-annex
/// 10.20261006 can persist ignore flags after an unsuccessful first probe.
/// The remote is named by the device ID.
pub async fn ensure_remote(git: &Git, device: &str, uuid: &str) -> Result<(), Error> {
    let url = format!("p2p-annex::iroh:{device}");
    let key = format!("remote.{device}.url");
    if git.config(&key).await.as_deref() != Some(url.as_str()) {
        git.run(&["remote", "add", device, &url]).await?;
    }
    let key = format!("remote.{device}.annex-uuid");
    if git.config(&key).await.as_deref() != Some(uuid) {
        git.run(&["config", &key, uuid]).await?;
    }
    let key = format!("remote.{device}.fetch");
    let spec = peer_refspec(device);
    if git.config(&key).await.as_deref() != Some(spec.as_str()) {
        git.run(&["config", &key, &spec]).await?;
    }
    for option in ["annex-ignore", "annex-ignore-auto"] {
        let key = format!("remote.{device}.{option}");
        if git.config(&key).await.as_deref() != Some("false") {
            git.run(&["config", &key, "false"]).await?;
        }
    }
    Ok(())
}

/// Where a peer's branches are fetched: `refs/pigeon/peers/<device>/`.
pub fn peer_refspec(device: &str) -> String {
    format!("+refs/heads/*:refs/pigeon/peers/{device}/*")
}
