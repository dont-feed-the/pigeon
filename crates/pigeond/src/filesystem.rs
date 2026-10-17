//! Filesystem preflight shared by create, join and doctor (14 §14.2).
//! Inspect before making a target folder or repository; never infer a
//! supported volume from its pathname or from a failed native observation.

use pigeon_core::filesystem::{Platform, supported};
use pigeon_core::filter::Role;
use std::path::Path;

/// Refuse unsupported or unreadable native filesystem identities.
pub(crate) fn check(path: &Path, role: Role) -> Result<(), &'static str> {
    let platform = if cfg!(target_os = "linux") {
        Platform::Linux
    } else if cfg!(target_vendor = "apple") {
        Platform::Mac
    } else {
        Platform::Other
    };
    if let Some(code) = role_problem(platform, role, cfg!(feature = "lab")) {
        return Err(code);
    }
    let volume = pigeon_sys::volume::inspect(path).map_err(|error| {
        tracing::warn!(path = %path.display(), %error, "filesystem-unreadable");
        "file-system-unknown"
    })?;
    if supported(platform, role, &volume.name, volume.local) {
        return Ok(());
    }
    // Container lab fixtures run on overlayfs. This is not filesystem
    // support: release builds cannot take this branch and doctor reports
    // the lab build. No network, FUSE or other refused driver is exempt.
    if cfg!(feature = "lab") && platform == Platform::Linux && volume.name == "overlay" {
        return Ok(());
    }
    // Existing multi-device Mac labs simulate Linux backup machines on
    // local APFS images; release builds still refuse macOS custodians.
    if cfg!(feature = "lab")
        && platform == Platform::Mac
        && volume.name == "apfs"
        && volume.local == Some(true)
    {
        return Ok(());
    }
    tracing::warn!(path = %path.display(), driver = %volume.name, ?role, "filesystem-unsupported");
    Err("file-system-unsupported")
}

fn role_problem(platform: Platform, role: Role, lab: bool) -> Option<&'static str> {
    (role == Role::Custodian && platform != Platform::Linux && !(lab && platform == Platform::Mac))
        .then_some("backup-machine-needs-linux")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_backup_roles_get_the_linux_requirement_and_lab_simulation_stays_explicit() {
        assert_eq!(
            role_problem(Platform::Mac, Role::Custodian, false),
            Some("backup-machine-needs-linux")
        );
        assert_eq!(
            role_problem(Platform::Other, Role::Custodian, true),
            Some("backup-machine-needs-linux")
        );
        assert_eq!(role_problem(Platform::Linux, Role::Custodian, false), None);
        assert_eq!(role_problem(Platform::Mac, Role::Custodian, true), None);
        assert_eq!(role_problem(Platform::Mac, Role::Laptop, false), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn unsupported_native_volume_is_refused_before_any_target_is_created() {
        let path = Path::new("/proc/pigeon-filesystem-preflight/nested");
        for role in [Role::Laptop, Role::Custodian] {
            assert_eq!(check(path, role), Err("file-system-unsupported"));
        }
        assert!(!path.exists());
    }
}
