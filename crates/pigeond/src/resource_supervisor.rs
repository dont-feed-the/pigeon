//! Native preset requests (ADR 0083, 15 §15.6). The supervisor replaces its
//! own scheduling configuration and restarts only for a priority change.
//! This hint grants no content authority and contains no paths or commands.

use pigeon_core::resources::Preset;
use std::path::{Path, PathBuf};

fn desired(preset: Preset) -> &'static [u8] {
    match preset {
        Preset::Gentle | Preset::Balanced => b"1 background\n",
        Preset::Fast => b"1 normal\n",
    }
}

/// One resource loop's last successfully reconciled native mode. Repeated
/// complete configuration polls must not reopen an uncached control file.
pub(crate) struct Requester {
    state: PathBuf,
    known: Option<&'static [u8]>,
}

impl Requester {
    pub(crate) fn new(state: PathBuf) -> Self {
        Self { state, known: None }
    }

    /// Invalid configuration never calls this; failed I/O stays retryable.
    pub(crate) fn request(&mut self, preset: Preset) {
        if let Err(error) = self.reconcile(preset) {
            tracing::warn!(%error, "resource-supervisor-request-failed");
        }
    }

    fn reconcile(&mut self, preset: Preset) -> std::io::Result<bool> {
        let wanted = desired(preset);
        if self.known == Some(wanted) {
            return Ok(false);
        }
        // A failed durable write may already have renamed its file before
        // the directory fsync failed. Forget the old mode before any I/O,
        // so even reverting the preset reconciles the actual file again.
        self.known = None;
        let changed = write(&self.state, preset)?;
        self.known = Some(wanted);
        Ok(changed)
    }
}

fn write(state: &Path, preset: Preset) -> std::io::Result<bool> {
    use std::io::Read;
    let path = state.join("resource-priority");
    let wanted = desired(preset);
    let failed = |error: pigeon_sys::Error| std::io::Error::other(error.to_string());
    if let Some(stat) = pigeon_sys::fs::lstat(&path).map_err(failed)? {
        if !stat.is_regular() {
            return Err(std::io::Error::other(
                "resource-priority is not a regular file",
            ));
        }
        if let Some(file) = pigeon_sys::fs::streaming_read_exact(&path, stat).map_err(failed)? {
            let mut current = Vec::new();
            file.take(17).read_to_end(&mut current)?;
            if current == wanted {
                return Ok(false);
            }
        }
    }
    std::fs::create_dir_all(state)?;
    // A single atomic replacement exposes only complete enum values.
    // Both provided path watches re-arm after replacement.
    pigeon_sys::fs::write_atomic(&path, wanted).map_err(failed)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hegel::{TestCase, generators as gs};

    /// Law: arbitrary preset sequences request exactly the last mode;
    /// only crossing the background/normal boundary writes again.
    #[hegel::test(test_cases = 1000)]
    #[allow(clippy::unwrap_used, reason = "isolated versioned hint fixture")]
    fn mode_changes_are_idempotent_and_last_choice_wins(tc: TestCase) {
        let state = tempfile::tempdir().unwrap();
        let mut previous = None;
        for _ in 0..tc.draw(gs::integers::<u8>().max_value(16)) {
            let preset = match tc.draw(gs::integers::<u8>().max_value(2)) {
                0 => Preset::Gentle,
                1 => Preset::Balanced,
                _ => Preset::Fast,
            };
            let next = desired(preset);
            assert_eq!(write(state.path(), preset).unwrap(), previous != Some(next));
            assert_eq!(
                std::fs::read(state.path().join("resource-priority")).unwrap(),
                next
            );
            previous = Some(next);
        }
    }

    /// Law: an unchanged native mode never reconciles a missing hint again;
    /// crossing the mode boundary reconciles to the final preset exactly.
    #[hegel::test(test_cases = 1000)]
    #[allow(
        clippy::unwrap_used,
        reason = "isolated per-loop reconciliation fixture"
    )]
    fn each_loop_reconciles_only_mode_changes(tc: TestCase) {
        let temp = tempfile::tempdir().unwrap();
        let state = temp.path().join("state");
        let mut requester = Requester::new(state.clone());
        let mut previous = None;
        for _ in 0..tc.draw(gs::integers::<u8>().min_value(1).max_value(16)) {
            let preset = match tc.draw(gs::integers::<u8>().max_value(2)) {
                0 => Preset::Gentle,
                1 => Preset::Balanced,
                _ => Preset::Fast,
            };
            let next = desired(preset);
            let hint = state.join("resource-priority");
            let parked = temp.path().join("parked");
            if previous == Some(next) {
                std::fs::rename(&hint, &parked).unwrap();
                assert!(!requester.reconcile(preset).unwrap());
                assert!(!hint.exists());
                std::fs::rename(&parked, &hint).unwrap();
            } else {
                assert!(requester.reconcile(preset).unwrap());
                assert_eq!(std::fs::read(&hint).unwrap(), next);
            }
            previous = Some(next);
        }
    }

    #[test]
    #[allow(
        clippy::unwrap_used,
        reason = "isolated failed-request recovery fixture"
    )]
    fn failed_io_does_not_cache_a_mode_or_hide_a_later_revert() {
        let temp = tempfile::tempdir().unwrap();
        let state = temp.path().join("state");
        let mut requester = Requester::new(state.clone());
        requester.reconcile(Preset::Fast).unwrap();
        let hint = state.join("resource-priority");
        std::fs::remove_file(&hint).unwrap();
        std::fs::create_dir(&hint).unwrap();
        assert!(requester.reconcile(Preset::Gentle).is_err());
        std::fs::remove_dir(&hint).unwrap();
        // Conservatively cover a failed write having changed the observed
        // bytes already. Returning to Fast must not trust its old memo.
        std::fs::write(&hint, desired(Preset::Gentle)).unwrap();
        assert!(requester.reconcile(Preset::Fast).unwrap());
        assert_eq!(std::fs::read(&hint).unwrap(), desired(Preset::Fast));
        let before = std::fs::metadata(&hint).unwrap().modified().unwrap();
        assert!(!Requester::new(state).reconcile(Preset::Fast).unwrap());
        assert_eq!(
            std::fs::metadata(&hint).unwrap().modified().unwrap(),
            before
        );
    }

    #[test]
    #[allow(
        clippy::unwrap_used,
        reason = "isolated idle-reload filesystem fixture"
    )]
    fn equal_native_modes_do_not_reopen_or_recreate_the_hint() {
        let temp = tempfile::tempdir().unwrap();
        let state = temp.path().join("state");
        let mut requester = Requester::new(state.clone());
        requester.request(Preset::Gentle);
        let parked = temp.path().join("parked");
        std::fs::rename(&state, &parked).unwrap();
        requester.request(Preset::Balanced);
        assert!(
            !state.exists(),
            "an unchanged native mode touched missing state"
        );
    }

    #[test]
    #[allow(clippy::unwrap_used, reason = "isolated local-state symlink fixture")]
    fn a_replaced_hint_does_not_follow_a_symlink_or_modify_its_target() {
        let state = tempfile::tempdir().unwrap();
        let target = state.path().join("identity");
        std::fs::write(&target, b"private identity").unwrap();
        std::os::unix::fs::symlink(&target, state.path().join("resource-priority")).unwrap();
        assert!(write(state.path(), Preset::Fast).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"private identity");
    }

    #[test]
    #[allow(
        clippy::unwrap_used,
        reason = "isolated configuration rejection fixture"
    )]
    fn invalid_resource_reading_keeps_the_last_supervisor_request() {
        let state = tempfile::tempdir().unwrap();
        write(state.path(), Preset::Fast).unwrap();
        let config = state.path().join("config.toml");
        std::fs::write(
            &config,
            "[resources]\npreset = \"gentle\"\non_metered = \"invalid\"\n",
        )
        .unwrap();
        assert!(crate::config::load(&config).is_err());
        assert_eq!(
            std::fs::read(state.path().join("resource-priority")).unwrap(),
            desired(Preset::Fast)
        );
    }
}

#[cfg(test)]
#[path = "resource_supervisor_runtime_tests.rs"]
mod runtime_tests;
