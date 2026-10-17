//! Filesystem admission laws, each over 1000 generated cases (14 §14.2).
//! Driver names are exact kernel identities, never pathname heuristics.

use hegel::{TestCase, generators as gs};
use pigeon_core::filesystem::{Platform, supported};
use pigeon_core::filter::Role;

#[hegel::test(test_cases = 1000)]
fn unknown_and_lookalike_drivers_never_admit(tc: TestCase) {
    let suffix = tc.draw(gs::integers::<u64>());
    for driver in [
        format!("unknown-{suffix}"),
        format!("ext4.{suffix}"),
        format!("fuse.apfs-{suffix}"),
    ] {
        for platform in [Platform::Linux, Platform::Mac, Platform::Other] {
            for role in [Role::Laptop, Role::Custodian] {
                for local in [None, Some(true), Some(false)] {
                    assert!(!supported(platform, role, &driver, local));
                }
            }
        }
    }
}

#[hegel::test(test_cases = 1000)]
fn explicit_remote_evidence_overrides_every_supported_driver(tc: TestCase) {
    let i = tc.draw(gs::integers::<usize>().max_value(4));
    let driver = ["ext4", "btrfs", "xfs", "zfs", "apfs"][i];
    for platform in [Platform::Linux, Platform::Mac, Platform::Other] {
        for role in [Role::Laptop, Role::Custodian] {
            assert!(!supported(platform, role, driver, Some(false)));
        }
    }
}

#[hegel::test(test_cases = 1000)]
fn platform_role_and_locality_bound_the_supported_set(tc: TestCase) {
    let i = tc.draw(gs::integers::<usize>().max_value(11));
    let driver = [
        "ext4", "btrfs", "xfs", "zfs", "apfs", "hfs", "vfat", "exfat", "nfs", "cifs", "fuse",
        "fuseblk",
    ][i];
    let role = if tc.draw(gs::booleans()) {
        Role::Laptop
    } else {
        Role::Custodian
    };
    let local = [None, Some(true), Some(false)][tc.draw(gs::integers::<usize>().max_value(2))];
    let linux = local != Some(false)
        && (["ext4", "btrfs", "xfs"].contains(&driver)
            || (driver == "zfs" && role == Role::Custodian));
    let mac = driver == "apfs" && role == Role::Laptop && local == Some(true);
    assert_eq!(supported(Platform::Linux, role, driver, local), linux);
    assert_eq!(supported(Platform::Mac, role, driver, local), mac);
    assert!(!supported(Platform::Other, role, driver, local));
}
