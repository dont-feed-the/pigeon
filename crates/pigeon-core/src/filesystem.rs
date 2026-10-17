//! Filesystem admission (14 §14.2): an exact native driver name, platform,
//! role and locality evidence determine whether a repository is supported.
//! Laws: unknown drivers and explicit remote evidence never admit; laptops
//! admit only their platform's listed drivers; ZFS is Linux backup-only.

use crate::filter::Role;

/// Platform whose native filesystem semantics the adapter observes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    /// Linux kernel filesystem drivers.
    Linux,
    /// Apple's APFS and native MNT_LOCAL evidence.
    Mac,
    /// No supported native adapter.
    Other,
}

/// Whether this exact observation meets the supported-filesystem policy.
/// An unknown Linux locality flag is acceptable only for a listed native
/// driver; macOS must explicitly report its volume as local.
pub fn supported(platform: Platform, role: Role, driver: &str, local: Option<bool>) -> bool {
    if local == Some(false) {
        return false;
    }
    match platform {
        Platform::Linux => {
            matches!(driver, "ext4" | "btrfs" | "xfs")
                || (role == Role::Custodian && driver == "zfs")
        }
        Platform::Mac => role == Role::Laptop && driver == "apfs" && local == Some(true),
        Platform::Other => false,
    }
}
