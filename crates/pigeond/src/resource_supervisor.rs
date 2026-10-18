//! Native preset requests (ADR 0083, 15 §15.6). The supervisor replaces its
//! own scheduling configuration and restarts only for a priority change.
//! This hint grants no content authority and contains no paths or commands.

use pigeon_core::resources::Preset;
use std::path::Path;

fn desired(preset: Preset) -> &'static [u8] {
    match preset {
        Preset::Gentle | Preset::Balanced => b"1 background\n",
        Preset::Fast => b"1 normal\n",
    }
}

/// A complete valid resource reading requests the matching native mode.
/// Identical readings never rewrite: both supervisors watch file changes.
/// Invalid configuration never reaches this function; retries after an I/O
/// failure leave the prior supervisor policy in force until a full write.
pub(crate) fn request(state: &Path, preset: Preset) {
    if let Err(error) = write(state, preset) {
        tracing::warn!(%error, "resource-supervisor-request-failed");
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
