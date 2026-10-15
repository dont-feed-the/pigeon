//! Names (05 §5.3–5.4): portable folding, portable paths, packages, and the
//! conflict names the catalogue merge gives (`catalogue`).
//!
//! Everything here is a pure function of its arguments and of the Unicode
//! 16.0 tables pinned in the core (`caseless` 0.2.2, `unicode-normalization`
//! 0.1.24); changing them is a format change (issue #15). Laws
//! (`tests/names.rs`): [`fold`] is idempotent; [`portable`] accepts every
//! [`conflict_name`] of a portable path; [`suggest`] is portable and differs
//! from every sibling after folding; names that do not collide never change.

use std::collections::{BTreeMap, BTreeSet};

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::is_nfc;

/// The longest component, in bytes (05 §5.4).
pub const MAX_COMPONENT: usize = 255;
/// The longest path, in bytes (05 §5.4).
pub const MAX_PATH: usize = 1024;

/// Folder extensions that make a package, one slot (07 §7.9).
pub const PACKAGES: [&str; 14] = [
    ".app",
    ".band",
    ".bundle",
    ".framework",
    ".key",
    ".logicx",
    ".numbers",
    ".pages",
    ".playground",
    ".rtfd",
    ".scriv",
    ".textbundle",
    ".xcodeproj",
    ".xcworkspace",
];

const RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Portable folding: NFC, Unicode full case folding, NFC again; two names
/// are one slot when they fold alike (05 §5.1).
pub fn fold(s: &str) -> String {
    let nfc: String = s.nfc().collect();
    caseless::default_case_fold_str(&nfc).nfc().collect()
}

/// The outermost package above `path` (07 §7.9), or `path` itself: the
/// unit the catalogue merges as one slot.
pub fn unit(path: &str) -> &str {
    let mut end = 0;
    for (i, part) in path.split('/').enumerate() {
        let start = end;
        end = start + part.len() + usize::from(i > 0);
        let last = end == path.len();
        if !last && is_package(part) {
            return &path[..end];
        }
    }
    path
}

/// Whether a folder named `name` is a package (07 §7.9, case-insensitive).
pub fn is_package(name: &str) -> bool {
    let folded = fold(name);
    PACKAGES
        .iter()
        .any(|ext| folded.len() > ext.len() && folded.ends_with(ext))
}

/// Why a path is not portable (05 §5.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotPortable {
    /// An empty component.
    Empty,
    /// A component not in NFC.
    NotNfc,
    /// A Windows reserved name.
    Reserved,
    /// A component ending with a dot or a space.
    Trailing,
    /// A forbidden or control character.
    Character,
    /// A component over 255 bytes, or a path over 1024.
    Length,
    /// `.git`, or `.pigeon` that is not pigeon's folder.
    Own,
    /// Two names alike after folding, or a file and a folder.
    Collision,
}

impl NotPortable {
    /// Its reason, as status and notices name it.
    pub fn code(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::NotNfc => "not-nfc",
            Self::Reserved => "reserved",
            Self::Trailing => "trailing",
            Self::Character => "character",
            Self::Length => "length",
            Self::Own => "own",
            Self::Collision => "collision",
        }
    }
}

/// Whether one component is portable, siblings aside.
pub fn component(name: &str) -> Result<(), NotPortable> {
    if name.is_empty() {
        return Err(NotPortable::Empty);
    }
    if name.len() > MAX_COMPONENT {
        return Err(NotPortable::Length);
    }
    if !is_nfc(name) {
        return Err(NotPortable::NotNfc);
    }
    if name
        .chars()
        .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
    {
        return Err(NotPortable::Character);
    }
    if name.ends_with(['.', ' ']) {
        return Err(NotPortable::Trailing);
    }
    let base = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if RESERVED.contains(&base.trim_end_matches(' ')) {
        return Err(NotPortable::Reserved);
    }
    if fold(name) == ".git" {
        return Err(NotPortable::Own);
    }
    Ok(())
}

/// Whether `path` is portable, siblings aside: each component is, the path
/// is short enough, and a root `.pigeon` is exactly pigeon's folder.
pub fn portable(path: &str) -> Result<(), NotPortable> {
    if path.len() > MAX_PATH {
        return Err(NotPortable::Length);
    }
    let parts: Vec<&str> = path.split('/').collect();
    for p in &parts {
        component(p)?;
    }
    if let Some(first) = parts.first()
        && fold(first) == ".pigeon"
        && (*first != ".pigeon" || parts.len() == 1)
    {
        return Err(NotPortable::Own);
    }
    Ok(())
}

/// The folded names that two spellings, or a file and a folder, share in a
/// tree's paths: empty when its names are injective after folding.
pub fn clashes<'a>(paths: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut seen: BTreeMap<String, BTreeSet<(&str, bool)>> = BTreeMap::new();
    for path in paths {
        let mut end = 0;
        let n = path.split('/').count();
        for (i, part) in path.split('/').enumerate() {
            end += part.len() + usize::from(i > 0);
            let prefix = &path[..end];
            seen.entry(fold(prefix))
                .or_default()
                .insert((prefix, i + 1 == n));
        }
    }
    seen.into_iter()
        .filter(|(_, s)| s.len() > 1)
        .map(|(f, _)| f)
        .collect()
}

/// Whether every path of a tree is portable and no two collide (rule 5 of
/// 04 §4.4); the first path that is not, and why.
pub fn portable_tree<'a>(
    paths: impl IntoIterator<Item = &'a str> + Clone,
) -> Result<(), (String, NotPortable)> {
    for p in paths.clone() {
        portable(p).map_err(|e| (p.to_owned(), e))?;
    }
    match clashes(paths).into_iter().next() {
        Some(f) => Err((f, NotPortable::Collision)),
        None => Ok(()),
    }
}

/// `dir/`, the stem and the extension of a path's last component; a name
/// starting with its only dot has no extension.
pub fn split_name(path: &str) -> (&str, &str, &str) {
    let cut = path.rfind('/').map_or(0, |i| i + 1);
    let (head, last) = path.split_at(cut);
    match last.rfind('.') {
        Some(dot) if dot > 0 => (head, &last[..dot], &last[dot..]),
        _ => (head, last, ""),
    }
}

/// A recorded time: seconds since the epoch, and the offset east of UTC, in
/// minutes, that the commit recorded (05 §5.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Stamp {
    /// Seconds since 1970-01-01 UTC.
    pub utc: i64,
    /// Minutes east of UTC.
    pub offset: i32,
}

impl Stamp {
    /// `YYYY-MM-DD HHhMM` in the recorded offset.
    pub fn label(self) -> String {
        let local = self.utc + i64::from(self.offset) * 60;
        let (days, secs) = (local.div_euclid(86_400), local.rem_euclid(86_400));
        let (y, m, d) = civil(days);
        format!(
            "{y:04}-{m:02}-{d:02} {:02}h{:02}",
            secs / 3600,
            secs % 3600 / 60
        )
    }
}

/// The proleptic Gregorian date of a day count from 1970-01-01.
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// A display name made portable for a conflict name: NFC, every forbidden
/// or control character and every parenthesis a `-`, never empty.
pub fn portable_person(who: &str) -> String {
    let s: String = who
        .nfc()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*()".contains(c) {
                '-'
            } else {
                c
            }
        })
        .collect();
    let s = s.trim().to_owned();
    if s.is_empty() { "-".to_owned() } else { s }
}

/// The conflict name of 05 §5.3, `dir/stem (Person, 2026-10-13 14h02).ext`,
/// with ` n` inside the parentheses from the second; the stem is shortened,
/// at a character boundary, so that the component fits in 255 bytes.
pub fn conflict_name(path: &str, who: &str, when: Stamp, n: u32) -> String {
    let (head, stem, ext) = split_name(path);
    let count = if n > 1 {
        format!(" {n}")
    } else {
        String::new()
    };
    let tag = format!(" ({}, {}{count})", portable_person(who), when.label());
    let room = MAX_COMPONENT.saturating_sub(tag.len() + ext.len());
    let mut cut = stem.len().min(room);
    while !stem.is_char_boundary(cut) {
        cut -= 1;
    }
    let stem: String = stem[..cut].nfc().collect();
    format!("{head}{stem}{tag}{ext}")
}

/// Whether `candidate` is named as a conflict copy of `path`, after folding.
pub fn is_copy_of(candidate: &str, path: &str) -> bool {
    let folded = fold(path);
    let (head, stem, ext) = split_name(&folded);
    let (pre, post) = (format!("{head}{stem} ("), format!("){ext}"));
    let c = fold(candidate);
    c.len() >= pre.len() + post.len() && c.starts_with(&pre) && c.ends_with(&post)
}

/// A portable name for `name` among `siblings`, suggested at capture when
/// `name` is held back (05 §5.4): each offending character a `-`, a
/// reserved or trailing name an `_` added, and ` 2`, ` 3`, … before the
/// extension until it differs from every sibling after folding.
pub fn suggest(name: &str, siblings: &[&str]) -> String {
    let mut s: String = name
        .nfc()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '-'
            } else {
                c
            }
        })
        .collect();
    let mut cut = s.len().min(MAX_COMPONENT - 8);
    while !s.is_char_boundary(cut) {
        cut -= 1;
    }
    s.truncate(cut);
    while s.ends_with(['.', ' ']) {
        s.pop();
    }
    if s.is_empty() || component(&s) == Err(NotPortable::Reserved) || fold(&s) == ".git" {
        s.insert(0, '_');
    }
    let taken: BTreeSet<String> = siblings.iter().map(|x| fold(x)).collect();
    let (_, stem, ext) = split_name(&s);
    let (stem, ext) = (stem.to_owned(), ext.to_owned());
    let mut n = 1u32;
    let mut out = s.clone();
    while taken.contains(&fold(&out)) {
        n += 1;
        out = format!("{stem} {n}{ext}");
    }
    out.nfc().collect()
}

/// Why capture holds a new path back (05 §5.4), and how to rename it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HoldBack {
    /// The path's first prefix at fault: a name that is not portable, or
    /// another spelling of a name `main` has.
    pub from: String,
    /// The portable name suggested for `from`, beside it; none when only the
    /// path's length is at fault.
    pub to: Option<String>,
}

/// Records every prefix of `path` in `taken`, folded name to spelling, the
/// first spelling winning.
pub fn take_prefixes(taken: &mut BTreeMap<String, String>, path: &str) {
    let mut end = 0;
    for (i, part) in path.split('/').enumerate() {
        end += part.len() + usize::from(i > 0);
        taken
            .entry(fold(&path[..end]))
            .or_insert_with(|| path[..end].to_owned());
    }
}

/// Whether capture holds back the new `path`, given the spellings `taken`
/// already has (see [`take_prefixes`]) and the names in each folder
/// (`siblings`, for a parent such as `a/b/`, or `""` at the root).
pub fn hold_back(
    path: &str,
    taken: &BTreeMap<String, String>,
    siblings: impl Fn(&str) -> Vec<String>,
) -> Option<HoldBack> {
    let mut end = 0;
    for (i, part) in path.split('/').enumerate() {
        let start = end + usize::from(i > 0);
        end = start + part.len();
        let prefix = &path[..end];
        let own = i == 0 && fold(part) == ".pigeon";
        let other = taken.get(&fold(prefix)).filter(|s| s.as_str() != prefix);
        if component(part).is_err() || own || other.is_some() {
            let parent = &path[..start];
            let mut names = siblings(parent);
            names.retain(|n| n != part);
            names.extend(other.and_then(|s| s.rsplit('/').next()).map(str::to_owned));
            let refs: Vec<&str> = names.iter().map(String::as_str).collect();
            let mut name = suggest(part, &refs);
            if own {
                name.insert(0, '_');
            }
            let to = format!("{parent}{name}");
            let whole = format!("{to}{}", &path[end..]);
            return Some(HoldBack {
                from: prefix.to_owned(),
                to: (whole.len() <= MAX_PATH).then_some(to),
            });
        }
    }
    (path.len() > MAX_PATH).then(|| HoldBack {
        from: path.to_owned(),
        to: None,
    })
}
