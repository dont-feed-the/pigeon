//! Observe actual working bytes across a bounded journalled rename window.
//! A journal never supplies bytes or authorizes a different content hash.

use std::path::Path;
use std::time::{Duration, Instant};

use super::{Exec, Verdict};
use crate::world::{Content, File, Laptop, World};

pub(super) fn condition(
    exec: &mut Exec,
    on: Laptop,
    file: &File,
    content: &Content,
    world: &World,
) -> Verdict {
    let want = exec.sha(content);
    let device = &exec.devs[on.0 as usize];
    let got = settled(
        &exec.path(on, file, world),
        &device.tree.join(".git/pigeon/install.ndjson"),
        &world.path(file),
    );
    if got.as_deref() == Some(want.as_str()) {
        Verdict::Pass
    } else {
        Verdict::Fail(format!(
            "{} {} sha256 {got:?}",
            device.name,
            world.path(file)
        ))
    }
}

// A native writer scan may outlast the ordinary one-second byte observation.
// Only a valid open transaction for this path extends it, to five seconds.
// This is a functional observer, never a resource or responsiveness measure.
pub(super) fn settled(file: &Path, journal: &Path, relative: &str) -> Option<String> {
    let start = Instant::now();
    settle_with(
        || super::observe::sha256_file(file),
        || super::absence::in_progress(journal, relative),
        || start.elapsed(),
        || std::thread::sleep(Duration::from_millis(200)),
    )
}

fn settle_with(
    mut read: impl FnMut() -> Option<String>,
    mut pending: impl FnMut() -> bool,
    mut elapsed: impl FnMut() -> Duration,
    mut wait: impl FnMut(),
) -> Option<String> {
    loop {
        // Any readable bytes, including the wrong hash, are returned at once.
        if let Some(hash) = read() {
            return Some(hash);
        }
        let spent = elapsed();
        if spent >= Duration::from_secs(5) || (spent >= Duration::from_secs(1) && !pending()) {
            return None;
        }
        wait();
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "isolated native byte-observation fixtures"
)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn even_an_open_transaction_does_not_hide_readable_wrong_bytes() {
        let result = settle_with(
            || Some("wrong bytes".to_owned()),
            || panic!("readable bytes must not consult the journal"),
            || panic!("readable bytes must not consult the clock"),
            || panic!("readable bytes must not wait"),
        );
        assert_eq!(result.as_deref(), Some("wrong bytes"));
    }

    #[test]
    fn persistent_absence_has_a_hard_bound_even_with_an_open_transaction() {
        for (pending, expected_ms) in [(false, 1000), (true, 5000)] {
            let ms = Cell::new(0);
            let result = settle_with(
                || None,
                || pending,
                || Duration::from_millis(ms.get()),
                || ms.set(ms.get() + 200),
            );
            assert_eq!(result, None);
            assert_eq!(ms.get(), expected_ms);
        }
    }

    #[test]
    fn malformed_completed_and_other_path_journals_cannot_extend_the_wait() {
        let root = tempfile::tempdir().unwrap();
        let journal = root.path().join("journal");
        for record in [
            "not json\n",
            "{\"format\":1,\"seq\":1,\"op\":\"begin\",\"path\":\"other/file\"}\n",
            "{\"format\":1,\"seq\":1,\"op\":\"begin\",\"path\":\"docs/file\"}",
            "{\"format\":1,\"seq\":1,\"op\":\"begin\",\"path\":\"docs/file\"}\n{\"format\":1,\"seq\":1,\"op\":\"done\"}\n",
        ] {
            std::fs::write(&journal, record).unwrap();
            assert!(!super::super::absence::in_progress(&journal, "docs/file"));
        }
    }

    #[test]
    fn a_journalled_rename_can_outlast_the_ordinary_byte_observation() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("file");
        let aside = root.path().join("aside");
        let journal = root.path().join("journal");
        std::fs::write(&file, b"protected unchanged bytes").unwrap();
        let expected = super::super::observe::sha256_file(&file);
        std::fs::write(
            &journal,
            b"{\"format\":1,\"seq\":1,\"op\":\"begin\",\"path\":\"docs/file\"}\n",
        )
        .unwrap();
        std::fs::rename(&file, &aside).unwrap();
        let (restore_file, restore_aside) = (file.clone(), aside.clone());
        let restore = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(1400));
            std::fs::rename(restore_aside, restore_file).unwrap();
        });
        let observed = settled(&file, &journal, "docs/file");
        restore.join().unwrap();
        assert_eq!(
            observed, expected,
            "a journalled move-aside was observed as lost bytes"
        );
    }
}
