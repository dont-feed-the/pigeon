//! Failure-domain fingerprints (ADR 0046): which disks hold a repository,
//! which host runs it, and under what virtualisation. Values pigeon cannot
//! find are unknown, never guessed. A lab build reads them from the file named
//! by `PIGEON_LAB_FINGERPRINTS` instead ([`crate::lab`], 13 §13.12).

use std::path::Path;
use std::process::Command;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::{Error, Result};

/// The raw identifiers of a repository's failure domains, before hashing.
/// `None` is unknown.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Identifiers {
    /// Every whole disk under the repository's file system (WWN, else serial,
    /// else `/dev/disk/by-id` name), a network share as `server:export`, or
    /// the file system's UUID.
    pub storage: Option<Vec<String>>,
    /// The host: `/etc/machine-id`, or the IOPlatformUUID on macOS.
    pub host: Option<String>,
    /// What `systemd-detect-virt` says; `""` on bare metal.
    #[serde(rename = "virtual")]
    pub virt: Option<String>,
}

/// The fingerprint of one identifier in one group: the first 16 bytes of
/// SHA-256 over `pigeon-domain`, the group ID and the identifier, each
/// followed by a zero byte. Members learn which custodians share hardware,
/// not serial numbers, and values differ between groups.
pub fn hash(group: &str, identifier: &str) -> [u8; 16] {
    let mut h = Sha256::new();
    for part in ["pigeon-domain", group, identifier] {
        h.update(part.as_bytes());
        h.update([0u8]);
    }
    let digest = h.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

/// A file system pigeon refuses to hold a repository: a reboot empties it.
pub fn is_volatile(fstype: &str) -> bool {
    matches!(fstype, "tmpfs" | "ramfs")
}

/// One line of `/proc/self/mountinfo`: the device number, the mount point,
/// the file-system type and the source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mount {
    /// `major:minor` of the device.
    pub dev: (u32, u32),
    /// Where it is mounted, with octal escapes decoded.
    pub mount_point: String,
    /// The file-system type, such as `ext4`.
    pub fstype: String,
    /// The source, such as `/dev/nvme0n1p2` or `server:/export`.
    pub source: String,
}

fn unescape(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let octal = bytes
            .get(i + 1..i + 4)
            .filter(|d| d.iter().all(|b| (b'0'..=b'7').contains(b)));
        if let (b'\\', Some(d)) = (bytes[i], octal) {
            let v = d.iter().fold(0u32, |acc, b| acc * 8 + u32::from(b - b'0'));
            if let Ok(v) = u8::try_from(v) {
                out.push(v);
                i += 4;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Parses `/proc/self/mountinfo`; lines it cannot read are skipped.
pub fn parse_mountinfo(text: &str) -> Vec<Mount> {
    text.lines()
        .filter_map(|line| {
            let (left, right) = line.split_once(" - ")?;
            let l: Vec<&str> = left.split(' ').collect();
            let r: Vec<&str> = right.split(' ').collect();
            let (major, minor) = l.get(2)?.split_once(':')?;
            Some(Mount {
                dev: (major.parse().ok()?, minor.parse().ok()?),
                mount_point: unescape(l.get(4)?),
                fstype: (*r.first()?).to_owned(),
                source: unescape(r.get(1)?),
            })
        })
        .collect()
}

/// Parses `lsblk -s -n -P -o NAME,TYPE,WWN,SERIAL` and names each row of
/// type `disk` by its WWN, else its serial. A serial is never used when a WWN
/// exists, since some NVMe disks all report one serial (E22).
pub fn disks_from_lsblk(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut fields = std::collections::BTreeMap::new();
        let mut rest = line.trim();
        while let Some((key, after)) = rest.split_once("=\"") {
            let Some((value, tail)) = after.split_once('"') else {
                break;
            };
            fields.insert(key.trim().to_owned(), value.to_owned());
            rest = tail.trim_start();
        }
        if fields.get("TYPE").map(String::as_str) != Some("disk") {
            continue;
        }
        let pick = |k: &str| fields.get(k).filter(|v| !v.trim().is_empty()).cloned();
        if let Some(id) = pick("WWN")
            .map(|w| format!("wwn:{w}"))
            .or_else(|| pick("SERIAL").map(|s| format!("serial:{s}")))
            && !out.contains(&id)
        {
            out.push(id);
        }
    }
    out
}

fn command_stdout(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The identifiers of the repository at `path`: in a lab build from the
/// fingerprints file, else by probing the machine.
pub fn identify(path: &Path) -> Result<Identifiers> {
    if let Some(file) = crate::lab::fingerprints_file() {
        let text = std::fs::read_to_string(&file).map_err(|e| Error::path("read", &file, e))?;
        return serde_json::from_str(&text).map_err(|e| Error::Malformed {
            what: crate::lab::FINGERPRINTS_VAR,
            detail: e.to_string(),
        });
    }
    Ok(Identifiers {
        storage: storage(path)?,
        host: host(),
        virt: virt(),
    })
}

fn host() -> Option<String> {
    if cfg!(target_vendor = "apple") {
        let text = command_stdout("ioreg", &["-rd1", "-c", "IOPlatformExpertDevice"])?;
        let line = text.lines().find(|l| l.contains("\"IOPlatformUUID\""))?;
        return line.rsplit('"').nth(1).map(str::to_owned);
    }
    let id = std::fs::read_to_string("/etc/machine-id").ok()?;
    let id = id.trim();
    (!id.is_empty()).then(|| id.to_owned())
}

fn virt() -> Option<String> {
    if cfg!(target_vendor = "apple") {
        return None;
    }
    // systemd-detect-virt prints `none` and exits 1 on bare metal.
    let out = Command::new("systemd-detect-virt").output().ok()?;
    let said = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    match said.as_str() {
        "none" => Some(String::new()),
        "" => None,
        _ => Some(said),
    }
}

/// The storage identifiers of the file system holding `path` (Linux), or
/// unknown. A loop device is followed to the storage of its backing file.
#[cfg(not(target_os = "linux"))]
fn storage(_path: &Path) -> Result<Option<Vec<String>>> {
    Ok(None)
}

#[cfg(target_os = "linux")]
fn storage(path: &Path) -> Result<Option<Vec<String>>> {
    storage_depth(path, 0)
}

#[cfg(target_os = "linux")]
fn storage_depth(path: &Path, depth: u32) -> Result<Option<Vec<String>>> {
    let Some(st) = crate::fs::lstat(path)? else {
        return Ok(None);
    };
    let dev = (rustix::fs::major(st.id.dev), rustix::fs::minor(st.id.dev));
    let text = std::fs::read_to_string("/proc/self/mountinfo")
        .map_err(|e| Error::path("read", "/proc/self/mountinfo", e))?;
    let Some(mount) = parse_mountinfo(&text).into_iter().find(|m| m.dev == dev) else {
        return Ok(None);
    };
    if is_volatile(&mount.fstype) {
        return Err(Error::Malformed {
            what: "file system",
            detail: mount.fstype,
        });
    }
    if matches!(mount.fstype.as_str(), "nfs" | "nfs4" | "cifs" | "smb3") {
        return Ok(Some(vec![format!("share:{}", mount.source)]));
    }
    if let Some(loop_name) = mount
        .source
        .strip_prefix("/dev/")
        .filter(|n| n.starts_with("loop"))
    {
        let backing = format!("/sys/block/{loop_name}/loop/backing_file");
        if depth < 4
            && let Ok(file) = std::fs::read_to_string(&backing)
        {
            return storage_depth(Path::new(file.trim()), depth + 1);
        }
        return Ok(None);
    }
    if mount.source.starts_with("/dev/") {
        if let Some(text) = command_stdout(
            "lsblk",
            &[
                "-s",
                "-n",
                "-P",
                "-o",
                "NAME,TYPE,WWN,SERIAL",
                &mount.source,
            ],
        ) {
            let disks = disks_from_lsblk(&text);
            if !disks.is_empty() {
                return Ok(Some(disks));
            }
        }
        if let Some(uuid) = command_stdout("lsblk", &["-n", "-d", "-o", "UUID", &mount.source]) {
            let uuid = uuid.trim();
            if !uuid.is_empty() {
                return Ok(Some(vec![format!("fs-uuid:{uuid}")]));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_differ_by_group_and_identifier() {
        assert_eq!(hash("g", "disk"), hash("g", "disk"));
        assert_ne!(hash("g", "disk"), hash("h", "disk"));
        assert_ne!(hash("g", "disk"), hash("g", "disk2"));
        assert_ne!(hash("ga", "b"), hash("g", "ab"), "parts are delimited");
    }

    #[test]
    fn mountinfo_lines_parse_with_escapes() {
        let text = "36 25 259:2 / /home/m\\040x rw,relatime shared:1 - ext4 /dev/nvme0n1p2 rw\n\
                    40 25 0:45 / /tmp rw - tmpfs tmpfs rw\nbroken line\n";
        let m = parse_mountinfo(text);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].dev, (259, 2));
        assert_eq!(m[0].mount_point, "/home/m x");
        assert_eq!(
            (m[0].fstype.as_str(), m[0].source.as_str()),
            ("ext4", "/dev/nvme0n1p2")
        );
        assert!(is_volatile(&m[1].fstype));
    }

    #[test]
    fn lsblk_prefers_the_wwn_and_skips_partitions() {
        let text = "NAME=\"nvme0n1p2\" TYPE=\"part\" WWN=\"\" SERIAL=\"\"\n\
                    NAME=\"nvme0n1\" TYPE=\"disk\" WWN=\"eui.0025\" SERIAL=\"00000\"\n\
                    NAME=\"sda\" TYPE=\"disk\" WWN=\"\" SERIAL=\"S3Z\"\n";
        assert_eq!(disks_from_lsblk(text), vec!["wwn:eui.0025", "serial:S3Z"]);
    }

    #[test]
    fn probing_this_machine_never_fails_on_a_disk() {
        let dir = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        let ids = identify(dir.path());
        if let Ok(ids) = ids
            && let Some(storage) = ids.storage
        {
            assert!(!storage.is_empty());
        }
    }
}
