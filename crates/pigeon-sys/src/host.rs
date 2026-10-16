//! This computer's name as the people who use it know it, which a device
//! given no name takes in the roster (09 §9.6).

/// What the system calls this computer: on macOS the computer name of
/// Sharing settings, else the host name up to its first dot; `computer`
/// when the system gives none.
pub fn name() -> String {
    #[cfg(target_vendor = "apple")]
    if let Some(name) = computer_name() {
        return name;
    }
    short(&rustix::system::uname().nodename().to_string_lossy())
        .unwrap_or_else(|| "computer".to_owned())
}

/// A host name up to its first dot, as `hostname -s` shows it; none when
/// that is blank.
fn short(host: &str) -> Option<String> {
    let label = host.trim().split('.').next().unwrap_or_default();
    (!label.is_empty()).then(|| label.to_owned())
}

/// The name macOS shows for this computer in Sharing settings, AirDrop and
/// Finder; none when blank.
#[cfg(target_vendor = "apple")]
fn computer_name() -> Option<String> {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use system_configuration_sys::dynamic_store_copy_specific::SCDynamicStoreCopyComputerName;

    // SAFETY: a null store asks the system's own, and a null encoding
    // pointer asks for none; the call returns a new string or null.
    let name = unsafe { SCDynamicStoreCopyComputerName(std::ptr::null(), std::ptr::null_mut()) };
    if name.is_null() {
        return None;
    }
    // SAFETY: `name` is a non-null CFString this call created, which the
    // wrapper owns and releases (the Create Rule).
    let name = unsafe { CFString::wrap_under_create_rule(name) }.to_string();
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::short;

    #[test]
    fn a_host_name_is_cut_at_its_first_dot() {
        assert_eq!(
            short("Marios-MacBook-Pro.local").as_deref(),
            Some("Marios-MacBook-Pro")
        );
        assert_eq!(short("nas.home.example.org\n").as_deref(), Some("nas"));
        assert_eq!(short("rtx-home").as_deref(), Some("rtx-home"));
        assert_eq!(short(" \n"), None);
        assert_eq!(short(".local"), None);
    }

    #[test]
    fn the_name_is_never_blank() {
        let name = super::name();
        assert!(!name.trim().is_empty() && !name.contains('\n'), "{name:?}");
    }

    /// The name agrees with what `scutil` reads from the same store.
    #[cfg(target_vendor = "apple")]
    #[test]
    fn on_macos_the_name_is_the_computer_name() {
        let out = std::process::Command::new("/usr/sbin/scutil")
            .args(["--get", "ComputerName"])
            .output()
            .unwrap();
        // A Mac whose computer name was never set has none to compare.
        if out.status.success() {
            let expected = String::from_utf8(out.stdout).unwrap();
            assert_eq!(super::name(), expected.trim());
        }
    }
}
