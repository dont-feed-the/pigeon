//! Where pigeon keeps its files: the per-user directories of 11 §11.2, the
//! system directories of a backup machine, and the per-repository files of
//! `.git/pigeon/` (11 §11.3).

use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// The four per-device directories of 11 §11.2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dirs {
    /// `config.toml` and the device key.
    pub config: PathBuf,
    /// Logs, telemetry files and trace events.
    pub state: PathBuf,
    /// Copies opened by `pigeon view`.
    pub cache: PathBuf,
    /// The socket, its lock file and the web port.
    pub runtime: PathBuf,
}

/// Who `pigeond` runs for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// A person's computer: the user's own directories.
    Person,
    /// A backup machine, where `pigeond` is a system service.
    Service,
}

impl Dirs {
    /// The directories for `role` on this platform. A person's are found with
    /// the `dirs` crate (ADR 0036): `$XDG_CONFIG_HOME/pigeon` and
    /// `~/Library/Application Support/pigeon` and so on. The runtime directory
    /// is `$XDG_RUNTIME_DIR/pigeon` on Linux, and on macOS, which has none,
    /// `run/` under the cache directory.
    pub fn for_role(role: Role) -> Result<Dirs> {
        match role {
            Role::Service => {
                let root = crate::lab::service_root().unwrap_or_else(|| PathBuf::from("/"));
                Ok(Dirs {
                    config: root.join("var/lib/pigeon"),
                    state: root.join("var/log/pigeon"),
                    cache: root.join("var/cache/pigeon"),
                    runtime: root.join("run/pigeon"),
                })
            }
            Role::Person => {
                let missing = |what: &'static str| Error::Malformed {
                    what,
                    detail: "the platform names no such directory".to_owned(),
                };
                let config = dirs::config_dir().ok_or_else(|| missing("config directory"))?;
                let cache = dirs::cache_dir().ok_or_else(|| missing("cache directory"))?;
                let state = if cfg!(target_vendor = "apple") {
                    dirs::home_dir().map(|h| h.join("Library/Logs"))
                } else {
                    dirs::state_dir()
                }
                .ok_or_else(|| missing("state directory"))?;
                let runtime = match dirs::runtime_dir() {
                    Some(dir) => dir.join("pigeon"),
                    None => cache.join("pigeon").join("run"),
                };
                Ok(Dirs {
                    config: config.join("pigeon"),
                    state: state.join("pigeon"),
                    cache: cache.join("pigeon"),
                    runtime,
                })
            }
        }
    }

    /// The person's settings, `config.toml`.
    pub fn config_file(&self) -> PathBuf {
        self.config.join("config.toml")
    }

    /// The device key (OpenSSH format, mode 0600); its public half adds `.pub`.
    pub fn device_key(&self) -> PathBuf {
        self.config.join("device-key")
    }

    /// The local socket of `pigeond`'s API.
    pub fn socket(&self) -> PathBuf {
        self.runtime.join("pigeond.sock")
    }

    /// The lock file that makes `pigeond` single (ADR 0011).
    pub fn lock_file(&self) -> PathBuf {
        self.runtime.join("pigeond.lock")
    }

    /// The UDP port the endpoint picked when `[network] bind` names port 0,
    /// kept so that peers' addresses of this device survive a restart.
    pub fn port(&self) -> PathBuf {
        self.state.join("port")
    }

    /// Daily JSON log files (ADR 0022).
    pub fn logs(&self) -> PathBuf {
        self.state.join("logs")
    }

    /// Trace events, written only by a build with tracing on (ADR 0008).
    pub fn traces(&self) -> PathBuf {
        self.state.join("traces")
    }
}

/// The socket a client, `pigeon` or the transport helper, reaches `pigeond`
/// on (09 §9.3): `PIGEON_SOCKET` if set, else the person's daemon's
/// ([`Dirs::socket`] of [`Role::Person`]), unless only a backup machine's
/// service answers here, whose socket is under `/run/pigeon`.
pub fn client_socket() -> PathBuf {
    let person = Dirs::for_role(Role::Person).ok().map(|d| d.socket());
    let service = Dirs::for_role(Role::Service).ok().map(|d| d.socket());
    client_socket_from(std::env::var_os("PIGEON_SOCKET"), person, service, |p| {
        p.exists()
    })
}

/// [`client_socket`] from its inputs: the override, the person's socket,
/// the service's, and which paths exist.
pub fn client_socket_from(
    over: Option<std::ffi::OsString>,
    person: Option<PathBuf>,
    service: Option<PathBuf>,
    exists: impl Fn(&Path) -> bool,
) -> PathBuf {
    if let Some(p) = over.filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    match (person, service) {
        (Some(p), Some(s)) if !exists(&p) && exists(&s) => s,
        (Some(p), _) => p,
        (None, Some(s)) => s,
        (None, None) => PathBuf::from("/tmp/pigeon/pigeond.sock"),
    }
}

/// The per-repository files of 11 §11.3, under `.git/pigeon/` of a
/// repository with a working tree, or `pigeon/` of a bare one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoFiles {
    /// The `pigeon` directory itself.
    pub root: PathBuf,
}

impl RepoFiles {
    /// The files of the repository whose git directory is `git_dir` (`.git`
    /// of a working tree, or the bare repository itself).
    pub fn new(git_dir: &Path) -> RepoFiles {
        RepoFiles {
            root: git_dir.join("pigeon"),
        }
    }

    /// The install journal, `install.ndjson`.
    pub fn install_journal(&self) -> PathBuf {
        self.root.join("install.ndjson")
    }

    /// The divergence table, `conflicts.json`.
    pub fn conflicts(&self) -> PathBuf {
        self.root.join("conflicts.json")
    }

    /// The purge decisions received and sealed, `purges.toml`.
    pub fn purges(&self) -> PathBuf {
        self.root.join("purges.toml")
    }

    /// The Lamport clock of trace events, `clock.json`.
    pub fn clock(&self) -> PathBuf {
        self.root.join("clock.json")
    }

    /// Where install step 4 moves the file of entry `seq`.
    pub fn aside(&self, seq: u64) -> PathBuf {
        self.root.join("aside").join(seq.to_string())
    }

    /// Where install step 2 writes the new file of entry `seq`.
    pub fn stage(&self, seq: u64) -> PathBuf {
        self.root.join("stage").join(seq.to_string())
    }

    /// The folder of moved-aside content that was not pigeon's.
    pub fn recovered(&self) -> PathBuf {
        self.root.join("recovered")
    }

    /// Creates the directories install needs, so that every rename between
    /// them and the working tree stays on one file system.
    pub fn create(&self) -> Result<()> {
        for dir in [
            self.root.join("aside"),
            self.root.join("stage"),
            self.recovered(),
        ] {
            std::fs::create_dir_all(&dir).map_err(|e| Error::path("mkdir", &dir, e))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn person_directories_end_in_pigeon() {
        if let Ok(d) = Dirs::for_role(Role::Person) {
            for p in [&d.config, &d.state, &d.cache] {
                assert!(p.ends_with("pigeon"), "{p:?}");
            }
            assert!(d.socket().starts_with(&d.runtime));
        }
        let s = Dirs::for_role(Role::Service).unwrap();
        assert_eq!(
            s.config_file(),
            PathBuf::from("/var/lib/pigeon/config.toml")
        );
    }

    #[test]
    fn a_client_reaches_the_daemon_that_runs_here() {
        let p = PathBuf::from("/me/pigeond.sock");
        let s = PathBuf::from("/run/pigeon/pigeond.sock");
        let pick = |over: Option<&str>, running: Option<&Path>| {
            client_socket_from(
                over.map(Into::into),
                Some(p.clone()),
                Some(s.clone()),
                |q: &Path| Some(q) == running,
            )
        };
        assert_eq!(
            pick(Some("/t/s.sock"), Some(&p)),
            PathBuf::from("/t/s.sock")
        );
        assert_eq!(pick(None, Some(&p)), p);
        assert_eq!(pick(None, Some(&s)), s, "only the service runs");
        assert_eq!(pick(Some(""), None), p, "neither runs: the person's");
        if let Ok(d) = Dirs::for_role(Role::Person) {
            assert_eq!(
                client_socket_from(None, Some(d.socket()), None, |_| true),
                d.socket(),
                "the daemon's own socket"
            );
        }
    }

    #[test]
    fn repository_files_sit_under_one_root() {
        let dir = tempfile::tempdir().unwrap();
        let f = RepoFiles::new(&dir.path().join(".git"));
        f.create().unwrap();
        assert!(f.aside(41).parent().unwrap().is_dir());
        assert!(f.stage(41).parent().unwrap().is_dir());
        assert_eq!(f.install_journal().file_name().unwrap(), "install.ndjson");
    }
}
