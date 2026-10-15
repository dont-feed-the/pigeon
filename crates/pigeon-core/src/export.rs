//! The pure half of `pigeon export` (09 §9.9, ADR 0047): which tree entry is
//! a person's file, where it may be written, which content a git-annex key
//! promises, and the `MANIFEST.tsv` that lists what was written. The CLI
//! reads the repositories and writes the files; this module decides.
//!
//! Laws (`tests/export.rs`): a manifest reads back as written, whatever its
//! fields hold; content matches a key exactly when its size and SHA-256 are
//! the key's; a safe path stays below the export directory and every path
//! with a `..`, `.git` or absolute component is refused; a pointer gives back
//! its key; a free name is never a tree path nor a folder of one.

use std::collections::BTreeSet;

/// The columns of `MANIFEST.tsv`, its first line.
pub const MANIFEST_HEADER: &str = "path\tsize\tsha256\tcommit\tauthor\tdate";

/// One written file, a line of `MANIFEST.tsv`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Its path below the export directory.
    pub path: String,
    /// Its size in bytes.
    pub size: u64,
    /// The SHA-256 of its bytes, lowercase hex.
    pub sha256: String,
    /// The last commit that changed it.
    pub commit: String,
    /// That commit's author.
    pub author: String,
    /// That commit's author date, ISO 8601.
    pub date: String,
}

/// The SHA-256 a `SHA256E` or `SHA256` key names, lowercase hex.
pub fn key_sha256(key: &str) -> Option<String> {
    let (fields, name) = key.split_once("--")?;
    let backend = fields.split('-').next()?;
    if backend != "SHA256E" && backend != "SHA256" {
        return None;
    }
    let hex = name.get(..64)?;
    let ext = name.get(64..).unwrap_or_default();
    let ok = hex.bytes().all(|b| b.is_ascii_hexdigit())
        && (backend == "SHA256E" && (ext.is_empty() || ext.starts_with('.')) || ext.is_empty());
    ok.then(|| hex.to_ascii_lowercase())
}

/// The size in bytes a key names (`…-s<size>-…--`), if it names one.
pub fn key_size(key: &str) -> Option<u64> {
    let fields = key.split_once("--")?.0;
    fields
        .split('-')
        .skip(1)
        .find_map(|f| f.strip_prefix('s'))
        .and_then(|n| n.parse().ok())
}

/// Whether content of `size` bytes with SHA-256 `digest` (hex) is what `key`
/// names. A key of another backend matches nothing: export checks every
/// object (ADR 0047).
pub fn matches(key: &str, size: u64, digest: &str) -> bool {
    key_sha256(key).is_some_and(|h| h.eq_ignore_ascii_case(digest))
        && key_size(key).is_none_or(|s| s == size)
}

/// The key an unlocked pointer blob names (`/annex/objects/KEY`).
pub fn pointer_key(blob: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(blob).ok()?;
    let line = text.lines().next()?;
    let key = line.strip_prefix("/annex/objects/")?;
    (!key.is_empty() && !key.contains('/') && text.trim_end() == line).then(|| key.to_owned())
}

/// The key a locked file's symlink target names (`…/annex/objects/…/KEY`).
pub fn link_key(target: &str) -> Option<String> {
    let (_, rest) = target.rsplit_once("annex/objects/")?;
    let key = rest.rsplit('/').next()?;
    (!key.is_empty() && !key.starts_with('.')).then(|| key.to_owned())
}

/// Whether `path`, read from a tree, may be written below the export
/// directory: relative, with no empty, `.`, `..` or `.git` component.
pub fn safe(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\0', '\\'])
        && path
            .split('/')
            .all(|c| !c.is_empty() && c != "." && c != ".." && !c.eq_ignore_ascii_case(".git"))
}

/// Whether a tree path is a person's file rather than pigeon's own (03 §3.3).
pub fn exported(path: &str) -> bool {
    !crate::authority::is_pigeon_path(path)
}

/// `base`, or `base-2`, `base-3`…: the first name that is neither a path of
/// `taken` nor the top folder of one.
pub fn free_name(base: &str, taken: &BTreeSet<String>) -> String {
    let busy = |n: &str| {
        taken
            .iter()
            .any(|p| p == n || p.strip_prefix(n).is_some_and(|r| r.starts_with('/')))
    };
    let mut n = 1u64;
    let mut name = base.to_owned();
    while busy(&name) {
        n += 1;
        name = format!("{base}-{n}");
    }
    name
}

fn escape(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    for c in field.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out
}

fn unescape(field: &str) -> Option<String> {
    let mut out = String::with_capacity(field.len());
    let mut it = field.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        out.push(match it.next()? {
            '\\' => '\\',
            't' => '\t',
            'n' => '\n',
            'r' => '\r',
            _ => return None,
        });
    }
    Some(out)
}

/// `MANIFEST.tsv`: the header, then one line per row, tabs, newlines and
/// backslashes escaped as `\t`, `\n`, `\r` and `\\`.
pub fn manifest(rows: &[Row]) -> String {
    let mut out = format!("{MANIFEST_HEADER}\n");
    for r in rows {
        let size = r.size.to_string();
        let fields = [&r.path, &size, &r.sha256, &r.commit, &r.author, &r.date];
        let line: Vec<String> = fields.iter().map(|f| escape(f)).collect();
        out.push_str(&line.join("\t"));
        out.push('\n');
    }
    out
}

/// The rows of a manifest [`manifest`] wrote, `None` if it is not one.
pub fn read_manifest(text: &str) -> Option<Vec<Row>> {
    let mut lines = text.lines();
    if lines.next()? != MANIFEST_HEADER {
        return None;
    }
    lines
        .map(|line| {
            let f: Vec<String> = line.split('\t').map(unescape).collect::<Option<_>>()?;
            let [path, size, sha256, commit, author, date] = <[String; 6]>::try_from(f).ok()?;
            Some(Row {
                path,
                size: size.parse().ok()?,
                sha256,
                commit,
                author,
                date,
            })
        })
        .collect()
}
