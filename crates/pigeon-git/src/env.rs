//! The environment of every git and git-annex process (03 §3.7, spec 03 §3.7):
//! no inherited `GIT_*` variable, no system or global configuration, no
//! system attributes, no prompt, and a UTF-8 character type. git then reads
//! only the repository's `.git/config`, which pigeon writes.

use std::ffi::OsString;
use std::path::PathBuf;

/// The variables pigeon sets on every child.
pub const FIXED: [(&str, &str); 4] = [
    ("GIT_CONFIG_NOSYSTEM", "1"),
    ("GIT_CONFIG_GLOBAL", "/dev/null"),
    ("GIT_ATTR_NOSYSTEM", "1"),
    ("GIT_TERMINAL_PROMPT", "0"),
];

/// How children are started.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GitEnv {
    /// Directories put first in the children's `PATH`, in order: the one
    /// holding the transport helper's link (and the `git-remote-p2p-annex`
    /// link where Homebrew installs none), then the git-annex the installer
    /// put in pigeon's directory (14 §14.3).
    pub path_first: Vec<PathBuf>,
    /// Extra variables, set after the fixed ones (for example by tests).
    pub extra: Vec<(OsString, OsString)>,
}

impl GitEnv {
    /// The full environment of a child, computed from `parent`, the
    /// environment of `pigeond`.
    pub fn child_env(
        &self,
        parent: impl IntoIterator<Item = (OsString, OsString)>,
    ) -> Vec<(OsString, OsString)> {
        let mut out: Vec<(OsString, OsString)> = parent
            .into_iter()
            .filter(|(k, _)| !k.to_string_lossy().starts_with("GIT_"))
            .collect();
        if !self.path_first.is_empty() {
            let old = out
                .iter()
                .position(|(k, _)| k == "PATH")
                .map(|i| out.remove(i).1);
            let mut paths = self.path_first.clone();
            if let Some(old) = &old {
                paths.extend(std::env::split_paths(old));
            }
            let joined = std::env::join_paths(paths)
                .or_else(|_| std::env::join_paths(&self.path_first))
                .unwrap_or_default();
            out.push(("PATH".into(), joined));
        }
        for (k, v) in FIXED {
            out.push((k.into(), v.into()));
        }
        utf8_ctype(&mut out);
        out.extend(self.extra.iter().cloned());
        out
    }

    /// Applies the environment to a command: clears what it inherited and
    /// sets [`GitEnv::child_env`] of this process's environment.
    pub fn apply(&self, cmd: &mut tokio::process::Command) {
        cmd.env_clear();
        cmd.envs(self.child_env(std::env::vars_os()));
    }
}

/// The UTF-8 locale children fall back to: present on every glibc since
/// 2.35 and on Debian's older ones; macOS has no `C.UTF-8`.
const UTF8_LOCALE: &str = if cfg!(target_vendor = "apple") {
    "en_US.UTF-8"
} else {
    "C.UTF-8"
};

/// Makes the children's character type UTF-8 when `pigeond`'s is not, as
/// under launchd or a bare service manager: git-annex decodes file names
/// with it, and without it skips a name such as `été.txt` while reporting
/// success, so the file is never captured. Only the character type
/// changes; an `LC_ALL` that would override it is replaced.
fn utf8_ctype(env: &mut Vec<(OsString, OsString)>) {
    let get = |k: &str| {
        env.iter()
            .rev()
            .find(|(kk, v)| kk == k && !v.is_empty())
            .map(|(_, v)| v.to_string_lossy().to_ascii_lowercase())
    };
    let ctype = get("LC_ALL")
        .or_else(|| get("LC_CTYPE"))
        .or_else(|| get("LANG"));
    if ctype.is_some_and(|c| c.contains("utf-8") || c.contains("utf8")) {
        return;
    }
    let key = if get("LC_ALL").is_some() {
        "LC_ALL"
    } else {
        "LC_CTYPE"
    };
    env.retain(|(k, _)| k != key);
    env.push((key.into(), UTF8_LOCALE.into()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctype(parent: Vec<(OsString, OsString)>) -> Vec<(OsString, OsString)> {
        let out = GitEnv::default().child_env(parent);
        out.into_iter()
            .filter(|(k, _)| k.to_string_lossy().starts_with("LC_") || k == "LANG")
            .collect()
    }

    #[test]
    fn a_child_without_a_locale_gets_a_utf8_character_type() {
        assert_eq!(ctype(vec![]), vec![("LC_CTYPE".into(), UTF8_LOCALE.into())]);
        assert_eq!(
            ctype(vec![("LANG".into(), "C".into())]),
            vec![
                ("LANG".into(), "C".into()),
                ("LC_CTYPE".into(), UTF8_LOCALE.into())
            ]
        );
        assert_eq!(
            ctype(vec![("LC_ALL".into(), "POSIX".into())]),
            vec![("LC_ALL".into(), UTF8_LOCALE.into())]
        );
    }

    #[test]
    fn a_utf8_locale_is_kept() {
        let fr = vec![("LANG".into(), "fr_FR.UTF-8".into())];
        assert_eq!(ctype(fr.clone()), fr);
        let all = vec![
            ("LANG".into(), "C".into()),
            ("LC_ALL".into(), "de_DE.utf8".into()),
        ];
        assert_eq!(ctype(all.clone()), all);
    }

    #[test]
    fn drops_git_variables_and_sets_the_fixed_ones() {
        let parent = vec![
            ("GIT_DIR".into(), "/x".into()),
            ("GIT_CONFIG_GLOBAL".into(), "/home/m/.gitconfig".into()),
            ("HOME".into(), "/home/m".into()),
            ("PATH".into(), "/usr/bin".into()),
        ];
        let env = GitEnv {
            path_first: vec!["/opt/bin".into(), "/opt/annex".into()],
            extra: vec![],
        };
        let out = env.child_env(parent);
        let get = |k: &str| {
            out.iter()
                .rev()
                .find(|(kk, _)| kk == k)
                .map(|(_, v)| v.clone())
        };
        assert_eq!(get("GIT_DIR"), None);
        assert_eq!(get("GIT_CONFIG_GLOBAL"), Some("/dev/null".into()));
        assert_eq!(get("HOME"), Some("/home/m".into()));
        assert_eq!(get("PATH"), Some("/opt/bin:/opt/annex:/usr/bin".into()));
        assert_eq!(
            out.iter().filter(|(k, _)| k == "GIT_CONFIG_GLOBAL").count(),
            1
        );
    }
}
