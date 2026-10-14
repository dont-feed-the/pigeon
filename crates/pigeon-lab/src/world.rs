//! The world of a scenario: its devices, folders and files, as typed handles.
//!
//! A [`World`] declares every device up front: laptops, which a person uses,
//! and custodians (backup machines). A handle is an index into the world,
//! typed by role, so an operation a custodian cannot do (such as `free`) has
//! no way to name one. Spare devices, which join or recover during the
//! scenario, are declared with `present = false`.

use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A laptop of the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Laptop(pub u8);

/// A custodian (backup machine) of the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Custodian(pub u8);

/// Any device, for what every device does: start, stop, crash, fill its disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Device {
    /// A laptop.
    Laptop(Laptop),
    /// A custodian.
    Custodian(Custodian),
}

impl From<Laptop> for Device {
    fn from(l: Laptop) -> Device {
        Device::Laptop(l)
    }
}

impl From<Custodian> for Device {
    fn from(c: Custodian) -> Device {
        Device::Custodian(c)
    }
}

/// A folder of the group's repository.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Folder(pub u8);

/// A file: a portable name inside a folder (05 §5.3).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct File {
    /// Its folder.
    pub folder: Folder,
    /// Its name in the folder: `/`-separated, portable components.
    pub name: String,
}

impl Folder {
    /// The file `name` in this folder.
    pub fn file(self, name: &str) -> File {
        File {
            folder: self,
            name: name.to_owned(),
        }
    }
}

/// Whether `name` is a portable relative name: non-empty components of
/// letters, digits, `.`, `-`, `_` and spaces, none `.` or `..`.
pub fn portable(name: &str) -> bool {
    !name.is_empty()
        && name.split('/').all(|c| {
            !c.is_empty()
                && c != "."
                && c != ".."
                && c.chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_' | ' '))
        })
}

/// What a file holds, as a value: short text, or deterministic bytes of any
/// length (a 1 GiB file is twelve bytes of scenario).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Content {
    /// These bytes.
    Text(String),
    /// `len` bytes of a SplitMix64 stream seeded with `seed`.
    Random {
        /// The seed.
        seed: u64,
        /// The length in bytes.
        len: u64,
    },
}

/// The SplitMix64 step.
fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

impl Content {
    /// The length in bytes.
    pub fn len(&self) -> u64 {
        match self {
            Content::Text(t) => t.len() as u64,
            Content::Random { len, .. } => *len,
        }
    }

    /// Whether it is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Feeds the bytes to `sink`, in chunks of at most 1 MiB.
    pub fn for_each_chunk(&self, mut sink: impl FnMut(&[u8])) {
        match self {
            Content::Text(t) => sink(t.as_bytes()),
            Content::Random { seed, len } => {
                let mut state = *seed;
                let mut left = *len;
                let mut buf = Vec::with_capacity(1 << 20);
                while left > 0 {
                    buf.clear();
                    let n = left.min(1 << 20);
                    while (buf.len() as u64) < n {
                        let word = splitmix(&mut state).to_le_bytes();
                        let take = ((n as usize) - buf.len()).min(8);
                        buf.extend_from_slice(&word[..take]);
                    }
                    sink(&buf);
                    left -= n;
                }
            }
        }
    }

    /// The SHA-256 of the bytes, in lowercase hex.
    pub fn sha256(&self) -> String {
        let mut h = Sha256::new();
        self.for_each_chunk(|c| h.update(c));
        hex(&h.finalize())
    }

    /// The content's identity in the model: git-annex's `SHA256` key of a
    /// text, and the seed for random bytes, whose hash a run computes only
    /// when it compares a file on disk.
    pub fn key(&self) -> Key {
        match self {
            Content::Text(_) => Key(format!("SHA256-s{}--{}", self.len(), self.sha256())),
            Content::Random { seed, len } => Key(format!("RANDOM-s{len}--{seed:016x}")),
        }
    }
}

/// Lowercase hex of `bytes`.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A content key: what custodians hold and what versions name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Key(pub String);

/// A laptop of the world.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaptopSpec {
    /// Its name in the roster.
    pub name: String,
    /// In the group from the start; the first present laptop creates it.
    pub present: bool,
    /// Its disk, in MiB: the size of the image its home lives on.
    pub disk_mib: u32,
}

/// A custodian of the world.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustodianSpec {
    /// Its name in the roster.
    pub name: String,
    /// In the group from the start; otherwise a laptop invites it later.
    pub present: bool,
    /// Its disk, in MiB: the size of its image.
    pub disk_mib: u32,
}

/// Every device, folder and setting of a scenario.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct World {
    /// The group's name.
    pub group: String,
    /// K: custody copies, on distinct custodians.
    pub k: u32,
    /// The laptops; the first present one is the group's admin.
    pub laptops: Vec<LaptopSpec>,
    /// The custodians.
    pub custodians: Vec<CustodianSpec>,
    /// The folders' names.
    pub folders: Vec<String>,
}

/// A laptop's disk unless a scenario asks otherwise, in MiB: room above
/// the `annex.diskreserve` pigeon sets on a laptop, max(1 GB, 1 %) (03
/// §3.4), which git-annex and capture refuse to cross.
pub const LAPTOP_DISK_MIB: u32 = 4096;

/// A custodian's disk unless a scenario asks otherwise, in MiB: room above
/// its `annex.diskreserve`, max(5 GB, 2 %) (03 §3.4); on a smaller disk
/// every get and incoming copy is refused (E22). The images are sparse.
pub const CUSTODIAN_DISK_MIB: u32 = 8192;

impl World {
    /// A world with no device yet.
    pub fn new(group: &str, k: u32) -> World {
        World {
            group: group.to_owned(),
            k,
            laptops: Vec::new(),
            custodians: Vec::new(),
            folders: Vec::new(),
        }
    }

    /// Adds a laptop in the group from the start.
    pub fn laptop(&mut self, name: &str) -> Laptop {
        self.laptops.push(LaptopSpec {
            name: name.to_owned(),
            present: true,
            disk_mib: LAPTOP_DISK_MIB,
        });
        Laptop((self.laptops.len() - 1) as u8)
    }

    /// Adds a laptop with a disk of `disk_mib` MiB.
    pub fn laptop_with_disk(&mut self, name: &str, disk_mib: u32) -> Laptop {
        let l = self.laptop(name);
        self.laptops[l.0 as usize].disk_mib = disk_mib;
        l
    }

    /// Adds a fresh laptop, outside the group until it recovers or joins.
    pub fn spare_laptop(&mut self, name: &str) -> Laptop {
        let l = self.laptop(name);
        self.laptops[l.0 as usize].present = false;
        l
    }

    /// Adds a custodian in the group from the start.
    pub fn custodian(&mut self, name: &str) -> Custodian {
        self.custodian_with_disk(name, CUSTODIAN_DISK_MIB)
    }

    /// Adds a custodian with a disk of `disk_mib` MiB.
    pub fn custodian_with_disk(&mut self, name: &str, disk_mib: u32) -> Custodian {
        self.custodians.push(CustodianSpec {
            name: name.to_owned(),
            present: true,
            disk_mib,
        });
        Custodian((self.custodians.len() - 1) as u8)
    }

    /// Adds a custodian that a laptop invites during the scenario.
    pub fn spare_custodian(&mut self, name: &str) -> Custodian {
        let c = self.custodian(name);
        self.custodians[c.0 as usize].present = false;
        c
    }

    /// Adds a folder.
    pub fn folder(&mut self, name: &str) -> Folder {
        self.folders.push(name.to_owned());
        Folder((self.folders.len() - 1) as u8)
    }

    /// The admin: the first present laptop.
    pub fn admin(&self) -> Option<Laptop> {
        self.laptops
            .iter()
            .position(|l| l.present)
            .map(|i| Laptop(i as u8))
    }

    /// Whether the handle names a device of this world.
    pub fn has(&self, d: Device) -> bool {
        match d {
            Device::Laptop(l) => (l.0 as usize) < self.laptops.len(),
            Device::Custodian(c) => (c.0 as usize) < self.custodians.len(),
        }
    }

    /// The device's name, or `?` for a handle of another world.
    pub fn name(&self, d: Device) -> &str {
        match d {
            Device::Laptop(l) => self.laptops.get(l.0 as usize).map(|s| s.name.as_str()),
            Device::Custodian(c) => self.custodians.get(c.0 as usize).map(|s| s.name.as_str()),
        }
        .unwrap_or("?")
    }

    /// The folder's name, or `?`.
    pub fn folder_name(&self, f: Folder) -> &str {
        self.folders.get(f.0 as usize).map_or("?", String::as_str)
    }

    /// The file's path from the repository's root: `folder/name`.
    pub fn path(&self, f: &File) -> String {
        format!("{}/{}", self.folder_name(f.folder), f.name)
    }

    /// Every device, laptops first.
    pub fn devices(&self) -> Vec<Device> {
        let ls = (0..self.laptops.len()).map(|i| Device::Laptop(Laptop(i as u8)));
        let cs = (0..self.custodians.len()).map(|i| Device::Custodian(Custodian(i as u8)));
        ls.chain(cs).collect()
    }
}

impl fmt::Display for World {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names = |present: bool, specs: Vec<(&str, bool)>| {
            specs
                .into_iter()
                .filter(|(_, p)| *p == present)
                .map(|(n, _)| n)
                .collect::<Vec<_>>()
                .join(", ")
        };
        let ls: Vec<(&str, bool)> = self
            .laptops
            .iter()
            .map(|l| (l.name.as_str(), l.present))
            .collect();
        let cs: Vec<(&str, bool)> = self
            .custodians
            .iter()
            .map(|c| (c.name.as_str(), c.present))
            .collect();
        write!(
            f,
            "group {} (K = {}); laptops: {}; custodians: {}; folders: {}",
            self.group,
            self.k,
            names(true, ls.clone()),
            names(true, cs.clone()),
            self.folders.join(", ")
        )?;
        let spare: Vec<String> = [names(false, ls), names(false, cs)]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect();
        if !spare.is_empty() {
            write!(f, "; spare: {}", spare.join(", "))?;
        }
        Ok(())
    }
}
