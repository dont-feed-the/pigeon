//! The automatic update's signal (14 §14.5, ADR 0076). When a report a peer
//! newly published says it runs a newer release than this build, `pigeond`
//! writes that release in `update-wanted` in its state directory; the
//! supervisor watches the file and answers each write by running the latest
//! release's installer. A pre-release or a build of main, on either side,
//! asks for nothing.

use std::path::Path;

/// This build's version as its reports carry it: a build of main adds its
/// commit, as `pigeon --version` does.
pub fn own() -> String {
    match option_env!("PIGEON_COMMIT") {
        Some(commit) => format!("{}+g{commit}", env!("CARGO_PKG_VERSION")),
        None => env!("CARGO_PKG_VERSION").to_owned(),
    }
}

/// A release's version `X.Y.Z` as its three numbers; anything else is none.
fn release(version: &str) -> Option<[u64; 3]> {
    let mut numbers = version.split('.').map(|n| {
        n.bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| n.parse().ok())
            .flatten()
    });
    let parsed = [numbers.next()??, numbers.next()??, numbers.next()??];
    numbers.next().is_none().then_some(parsed)
}

/// Whether `peer` is a release newer than the release `own`.
pub fn newer(peer: &str, own: &str) -> bool {
    matches!((release(peer), release(own)), (Some(p), Some(o)) if p > o)
}

/// Asks the supervisor for the update when `peer` is newer than this build.
pub fn follow(wanted: &Path, peer: &str) {
    if !newer(peer, &own()) {
        return;
    }
    // A plain write, not a rename: the supervisors watch this file's writes.
    let written = wanted
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(wanted, format!("{peer}\n")));
    match written {
        Ok(()) => tracing::info!(peer = %peer, "update-wanted"),
        Err(e) => tracing::warn!(peer = %peer, error = %e, "update-wanted-failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::{follow, newer};

    #[test]
    fn only_a_newer_release_asks_for_an_update() {
        assert!(newer("0.5.9", "0.5.8"));
        assert!(newer("0.10.0", "0.9.12"));
        assert!(newer("1.0.0", "0.99.99"));
        assert!(!newer("0.5.8", "0.5.8"));
        assert!(!newer("0.5.7", "0.5.8"));
        assert!(!newer("0.5.9+g1a2b3c4d5e6f", "0.5.8"));
        assert!(!newer("0.5.9", "0.5.8+g1a2b3c4d5e6f"));
        assert!(!newer("2.0.0-alpha.0", "0.5.8"));
        assert!(!newer("0.5.+9", "0.5.8"));
        assert!(!newer("0.5", "0.4.0"));
        assert!(!newer("0.5.9.1", "0.5.8"));
        assert!(!newer("", "0.5.8"));
    }

    #[test]
    fn a_newer_peer_rewrites_the_watched_file_and_an_older_one_leaves_it() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let wanted = dir.path().join("state/update-wanted");
        follow(&wanted, "0.0.1");
        assert!(!wanted.exists());
        follow(&wanted, "999.0.0");
        assert_eq!(std::fs::read_to_string(&wanted)?, "999.0.0\n");
        Ok(())
    }
}
