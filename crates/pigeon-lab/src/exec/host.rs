//! Where a device lives: its disk and how a command runs on it.
//!
//! - `linux-namespaces` (13 §13.12): each device's disk is a sparse ext4
//!   image, loop-mounted with `sudo`; each custodian runs in its own mount
//!   namespace, where its image's directories are bound over
//!   `/var/lib/pigeon`, `/var/log/pigeon`, `/var/cache/pigeon` and
//!   `/run/pigeon`.
//! - `macos-images`: each disk is a sparse APFS image attached with
//!   `hdiutil`; custodians keep their system directories under the lab
//!   seam `PIGEON_LAB_SERVICE_ROOT` (lab only: macOS is no backup machine).
//! - `dirs`: plain directories and the same seam, for a developer's machine
//!   without `sudo`; disks have no size, so scenarios that need one skip.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// How devices are kept apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Isolation {
    /// Loop-mounted ext4 images and mount namespaces (Linux, `sudo`).
    Namespaces,
    /// APFS disk images (macOS).
    Images,
    /// Plain directories.
    Dirs,
}

fn succeeds(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Runs a command, or says how it failed.
pub fn run(cmd: &mut Command) -> Result<String, String> {
    let out = cmd.output().map_err(|e| format!("{cmd:?}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "{cmd:?}: {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

impl Isolation {
    /// The best this host offers.
    pub fn detect() -> Isolation {
        if cfg!(target_os = "linux")
            && succeeds("sudo", &["-n", "true"])
            && succeeds("unshare", &["--help"])
        {
            Isolation::Namespaces
        } else if cfg!(target_os = "macos") && succeeds("hdiutil", &["help"]) {
            Isolation::Images
        } else {
            Isolation::Dirs
        }
    }

    /// Its name in reports.
    pub fn name(self) -> &'static str {
        match self {
            Isolation::Namespaces => "linux-namespaces",
            Isolation::Images => "macos-images",
            Isolation::Dirs => "dirs",
        }
    }

    /// Whether disks have their declared size and fill up.
    pub fn sized(self) -> bool {
        self != Isolation::Dirs
    }

    /// Whether custodians need the service-root seam.
    pub fn service_seam(self) -> bool {
        self != Isolation::Namespaces
    }
}

/// The system directories of a backup machine, relative to `/`.
pub const SERVICE_DIRS: [&str; 4] = [
    "var/lib/pigeon",
    "var/log/pigeon",
    "var/cache/pigeon",
    "run/pigeon",
];

/// Removes `path` and everything under it, git-annex's read-only object
/// directories included.
pub fn remove_tree(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fn open_up(p: &Path) {
        let Ok(meta) = std::fs::symlink_metadata(p) else {
            return;
        };
        if !meta.is_dir() {
            return;
        }
        let mode = meta.permissions().mode();
        if mode & 0o700 != 0o700 {
            let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode | 0o700));
        }
        for e in std::fs::read_dir(p).into_iter().flatten().flatten() {
            open_up(&e.path());
        }
    }
    open_up(path);
    match std::fs::remove_dir_all(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        r => r,
    }
}

/// One device's disk and, for a namespaced custodian, its namespace.
#[derive(Debug)]
pub struct Space {
    /// How it is isolated.
    pub isolation: Isolation,
    /// Its directory on the host: logs, the pid file, the image.
    pub dir: PathBuf,
    /// Where its disk is mounted, visible to the executor.
    pub root: PathBuf,
    /// Its image, if any.
    image: Option<PathBuf>,
    /// The namespace's holder process and its pid.
    holder: Option<(Child, u32)>,
    /// The user and group that run its daemon.
    ids: (String, String),
}

fn id(flag: &str) -> Result<String, String> {
    run(Command::new("id").arg(flag)).map(|s| s.trim().to_owned())
}

impl Space {
    /// Makes the disk of a device named `name` under `work`.
    pub fn create(
        isolation: Isolation,
        work: &Path,
        name: &str,
        mib: u32,
        custodian: bool,
    ) -> Result<Space, String> {
        let dir = work.join(name);
        let root = dir.join("disk");
        std::fs::create_dir_all(&root).map_err(|e| format!("{}: {e}", root.display()))?;
        let ids = (id("-u")?, id("-g")?);
        let mut space = Space {
            isolation,
            dir: dir.clone(),
            root: root.clone(),
            image: None,
            holder: None,
            ids,
        };
        match isolation {
            Isolation::Dirs => {}
            Isolation::Namespaces => {
                let image = dir.join("disk.ext4");
                run(Command::new("truncate")
                    .arg("-s")
                    .arg(format!("{mib}M"))
                    .arg(&image))?;
                let owner = format!("root_owner={}:{}", space.ids.0, space.ids.1);
                run(Command::new("mkfs.ext4")
                    .args(["-q", "-F", "-E", &owner])
                    .arg(&image))?;
                run(Command::new("sudo")
                    .args(["-n", "mount", "-o", "loop"])
                    .arg(&image)
                    .arg(&root))?;
                space.image = Some(image);
            }
            Isolation::Images => {
                let image = dir.join("disk.sparseimage");
                run(Command::new("hdiutil")
                    .args([
                        "create", "-quiet", "-type", "SPARSE", "-fs", "APFS", "-size",
                    ])
                    .arg(format!("{mib}m"))
                    .args(["-volname", &format!("pigeon-lab-{name}")])
                    .arg(&image))?;
                run(Command::new("hdiutil")
                    .args([
                        "attach",
                        "-quiet",
                        "-nobrowse",
                        "-owners",
                        "on",
                        "-mountpoint",
                    ])
                    .arg(&root)
                    .arg(&image))?;
                space.image = Some(image);
            }
        }
        if custodian {
            for d in SERVICE_DIRS {
                let p = root.join("service").join(d);
                std::fs::create_dir_all(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            }
            if isolation == Isolation::Namespaces {
                space.enter_namespace()?;
            }
        }
        Ok(space)
    }

    /// Starts the holder of a custodian's mount namespace, with the service
    /// directories of its image bound over the system ones.
    fn enter_namespace(&mut self) -> Result<(), String> {
        let mut script = String::from("set -e; ");
        for d in SERVICE_DIRS {
            let from = self.root.join("service").join(d);
            script.push_str(&format!(
                "mkdir -p /{d}; mount --bind '{}' /{d}; ",
                from.display()
            ));
        }
        script.push_str("echo $$; exec sleep 1000000");
        let mut child = Command::new("sudo")
            .args([
                "-n",
                "unshare",
                "--mount",
                "--propagation",
                "private",
                "--",
                "sh",
                "-c",
                &script,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("unshare: {e}"))?;
        let mut line = String::new();
        if let Some(out) = child.stdout.take() {
            let _ = BufReader::new(out).read_line(&mut line);
        }
        let pid: u32 = line
            .trim()
            .parse()
            .map_err(|_| format!("the namespace of {} did not start", self.dir.display()))?;
        self.holder = Some((child, pid));
        Ok(())
    }

    /// A command that runs `program` on this device, as the user, with
    /// exactly `env` (and `PATH`).
    pub fn command(&self, program: &Path, args: &[String], env: &[(String, String)]) -> Command {
        let path = std::env::var("PATH").unwrap_or_default();
        match &self.holder {
            Some((_, pid)) => {
                let mut cmd = Command::new("sudo");
                cmd.args([
                    "-n",
                    "nsenter",
                    "-t",
                    &pid.to_string(),
                    "-m",
                    "--",
                    "setpriv",
                ])
                .arg(format!("--reuid={}", self.ids.0))
                .arg(format!("--regid={}", self.ids.1))
                .args(["--clear-groups", "--", "env", "-i"])
                .arg(format!("PATH={path}"));
                for (k, v) in env {
                    cmd.arg(format!("{k}={v}"));
                }
                cmd.arg(program).args(args);
                cmd
            }
            None => {
                let mut cmd = Command::new(program);
                cmd.args(args)
                    .env_clear()
                    .env("PATH", path)
                    .envs(env.iter().map(|(k, v)| (k, v)));
                cmd
            }
        }
    }

    /// Bytes free on its disk.
    pub fn free_bytes(&self) -> Result<u64, String> {
        let out = run(Command::new("df").arg("-Pk").arg(&self.root))?;
        let kib: u64 = out
            .lines()
            .nth(1)
            .and_then(|l| l.split_whitespace().nth(3))
            .and_then(|n| n.parse().ok())
            .ok_or_else(|| format!("df: {out}"))?;
        Ok(kib * 1024)
    }

    /// Unmounts and removes everything.
    pub fn teardown(&mut self) {
        if let Some((mut child, pid)) = self.holder.take() {
            let _ = run(Command::new("sudo").args(["-n", "kill", "-9", &pid.to_string()]));
            let _ = child.wait();
        }
        match self.isolation {
            Isolation::Namespaces => {
                let _ = run(Command::new("sudo")
                    .args(["-n", "umount", "-l"])
                    .arg(&self.root));
            }
            Isolation::Images => {
                let _ = run(Command::new("hdiutil")
                    .args(["detach", "-quiet", "-force"])
                    .arg(&self.root));
            }
            Isolation::Dirs => {}
        }
        if let Some(image) = &self.image {
            let _ = std::fs::remove_file(image);
        }
    }
}
