//! Default joining destinations must not treat received names as paths
//! (09 §9.6, §9.9). Explicit folders remain the person's choice.

use std::path::{Path, PathBuf};

/// A received display name may supply one portable folder component only.
/// A name that cannot do so needs an explicit folder instead of sanitization.
pub(crate) fn default_folder(base: &Path, name: &str) -> Result<PathBuf, String> {
    if name.contains('/') || pigeon_core::names::portable(name).is_err() {
        return Err("join-folder-needed".into());
    }
    Ok(base.join(name))
}

#[cfg(test)]
#[path = "join_destination_tests.rs"]
mod tests;
