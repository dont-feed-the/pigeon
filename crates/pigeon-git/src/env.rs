//! The environment of every git and git-annex process (03 §3.7, spec 03 §3.7):
//! no inherited `GIT_*` variable, no system or global configuration, no
//! system attributes, no prompt. git then reads only the repository's
//! `.git/config`, which pigeon writes.

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

#[cfg(test)]
mod tests {
    use super::*;

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
