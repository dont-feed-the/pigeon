//! Daily diagnostic files with the existing name and seven-file retention
//! (ADR 0022, 12 §12.1). Only the logging worker performs this I/O.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

pub(super) struct Daily {
    dir: PathBuf,
    opened: Option<(String, File)>,
}

impl Daily {
    pub(super) fn new(dir: PathBuf) -> Self {
        Self { dir, opened: None }
    }

    fn file(&mut self) -> io::Result<&mut File> {
        let day = crate::util::date(crate::util::now());
        if self.opened.as_ref().is_none_or(|(d, _)| *d != day) {
            std::fs::create_dir_all(&self.dir)?;
            let name = format!("pigeond-{day}.ndjson");
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .mode(0o600)
                .open(self.dir.join(&name))?;
            self.opened = Some((day, file));
            // A stale diagnostic file's permissions cannot prevent writing
            // the current log. No repository or user document is pruned.
            let _ = prune(&self.dir, &name);
        }
        self.opened
            .as_mut()
            .map(|(_, f)| f)
            .ok_or_else(|| io::Error::other("log file unavailable"))
    }
}

impl Write for Daily {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.file()?.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        if let Some((_, file)) = &mut self.opened {
            file.flush()?;
        }
        Ok(())
    }
}

fn dated(name: &str) -> bool {
    let Some(date) = name
        .strip_prefix("pigeond-")
        .and_then(|s| s.strip_suffix(".ndjson"))
    else {
        return false;
    };
    date.len() == 10
        && date.bytes().enumerate().all(|(i, b)| match i {
            4 | 7 => b == b'-',
            _ => b.is_ascii_digit(),
        })
        && pigeon_proto::time::parse_rfc3339(&format!("{date}T00:00:00Z"))
            .is_some_and(|time| pigeon_proto::time::date(time) == date)
}

/// Keep the active file and at most six other daily files. The active file
/// stays even after a backwards wall-clock correction. Unknown names,
/// directories and symbolic links are never selected for removal.
pub(super) fn prune(dir: &Path, active: &str) -> io::Result<()> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if name != active && dated(&name) && entry.file_type()?.is_file() {
            names.push(name);
        }
    }
    names.sort_unstable_by(|a, b| b.cmp(a));
    for name in names.into_iter().skip(6) {
        std::fs::remove_file(dir.join(name))?;
    }
    Ok(())
}
