//! What a suggestion changes, file by file (ADR 0077, 04 §4.5), for the
//! Suggestions page and `pigeon suggestions`: each path of the proposal's
//! change on its base is a new file, a changed one or a deletion, with its
//! size before and after, read from its git-annex key; a deleted path and a
//! new one with the same key are one rename, decided together. For a small
//! text file whose two versions this computer holds, the lines added and
//! removed are counted, by the longest common sequence of lines; nothing is
//! fetched for it.

use std::collections::BTreeMap;

use pigeon_core::authority::{Tree, delta};
use pigeon_proto::api::{FileChange, SuggestedFile};

use crate::group::Group;

/// The largest file whose lines are counted, in bytes.
const MAX_TEXT: u64 = 64 * 1024;
/// The most lines of either version whose differences are counted.
const MAX_LINES: usize = 2000;

/// The files the proposal of `tree` on the base `at` changes, by path.
pub async fn files(g: &Group, at: &Tree, tree: &Tree) -> Vec<SuggestedFile> {
    let paths: Vec<String> = delta(at, tree).into_iter().collect();
    let mut keys: BTreeMap<(bool, String), Option<String>> = BTreeMap::new();
    let mut oids = Vec::new();
    for p in &paths {
        for (after, t) in [(false, at), (true, tree)] {
            if let Some(e) = t.get(p) {
                oids.push(((after, p.clone()), e.oid.clone()));
            }
        }
    }
    let found = g
        .git
        .objects(&oids.iter().map(|(_, o)| o.clone()).collect::<Vec<_>>())
        .await
        .unwrap_or_default();
    for ((slot, _), blob) in oids.into_iter().zip(found) {
        let key = blob.and_then(|(_, b)| crate::git::pointer_key(&b));
        keys.insert(slot, key);
    }
    let key = |after: bool, p: &str| keys.get(&(after, p.to_owned())).cloned().flatten();
    let mut rows = Vec::new();
    let mut gone: Vec<(String, Option<String>)> = paths
        .iter()
        .filter(|p| !tree.contains_key(*p))
        .map(|p| (p.clone(), key(false, p)))
        .collect();
    for p in &paths {
        let (before, after) = (key(false, p), key(true, p));
        let row = match (at.contains_key(p), tree.contains_key(p)) {
            (false, true) => {
                let moved = gone
                    .iter()
                    .position(|(_, k)| k.is_some() && *k == after)
                    .map(|i| gone.remove(i).0);
                match moved {
                    Some(from) => row(p, FileChange::Renamed, Some(from), &after, &after),
                    None => row(p, FileChange::Added, None, &None, &after),
                }
            }
            (true, true) => {
                let mut r = row(p, FileChange::Changed, None, &before, &after);
                r.lines = lines(g, before.as_deref(), after.as_deref()).await;
                r
            }
            _ => continue,
        };
        rows.push(row);
    }
    for (p, before) in gone {
        rows.push(row(&p, FileChange::Deleted, None, &before, &None));
    }
    rows.sort_by(|a, b| a.path.cmp(&b.path));
    rows
}

fn row(
    path: &str,
    change: FileChange,
    from: Option<String>,
    before: &Option<String>,
    after: &Option<String>,
) -> SuggestedFile {
    let size = |k: &Option<String>| k.as_deref().and_then(crate::restore::key_size);
    SuggestedFile {
        path: path.to_owned(),
        change,
        from,
        before: size(before),
        after: size(after),
        lines: None,
    }
}

/// Each path of `files`, a rename by its old name too, with every path of
/// the proposal decided with it: a rename's two.
pub fn together(files: &[SuggestedFile]) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for f in files {
        let all: Vec<String> = std::iter::once(f.path.clone())
            .chain(f.from.iter().cloned())
            .collect();
        for p in &all {
            out.insert(p.clone(), all.clone());
        }
    }
    out
}

/// The lines added and removed between the contents of `before` and
/// `after`, when both are small texts this computer holds.
async fn lines(g: &Group, before: Option<&str>, after: Option<&str>) -> Option<(u64, u64)> {
    let (before, after) = (before?, after?);
    let small = |k: &str| crate::restore::key_size(k).is_some_and(|s| s <= MAX_TEXT);
    if !small(before) || !small(after) {
        return None;
    }
    let old = std::fs::read(crate::custody::present(g, before).await?).ok()?;
    let new = std::fs::read(crate::custody::present(g, after).await?).ok()?;
    counts(&old, &new)
}

/// The lines of `b`, if it is a text of at most [`MAX_LINES`] lines.
fn text_lines(b: &[u8]) -> Option<Vec<&str>> {
    let t = std::str::from_utf8(b).ok()?;
    let lines: Vec<&str> = t.lines().collect();
    (!t.contains('\0') && lines.len() <= MAX_LINES).then_some(lines)
}

/// The lines of `new` not in a longest common sequence with `old`, and
/// those of `old`; `None` when either is not text or is too long.
pub fn counts(old: &[u8], new: &[u8]) -> Option<(u64, u64)> {
    let (a, b) = (text_lines(old)?, text_lines(new)?);
    let mut row = vec![0usize; b.len() + 1];
    for x in &a {
        let mut diagonal = 0;
        for (j, y) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = if x == y {
                diagonal + 1
            } else {
                above.max(row[j])
            };
            diagonal = above;
        }
    }
    let common = row[b.len()];
    let n = |k: usize| u64::try_from(k - common).unwrap_or(u64::MAX);
    Some((n(b.len()), n(a.len())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_added_and_removed_are_counted_around_what_both_keep() {
        assert_eq!(counts(b"a\nb\nc\n", b"a\nB\nc\nd\n"), Some((2, 1)));
        assert_eq!(counts(b"", b"one\ntwo\n"), Some((2, 0)));
        assert_eq!(counts(b"same\n", b"same\n"), Some((0, 0)));
        assert_eq!(counts(b"x\0y", b"x"), None);
        assert_eq!(counts(&[0xff, 0xfe], b"x"), None);
        let long = "l\n".repeat(MAX_LINES + 1);
        assert_eq!(counts(long.as_bytes(), b"x"), None);
    }

    #[test]
    fn a_rename_is_decided_with_its_old_name() {
        let file = |path: &str, from: Option<&str>| SuggestedFile {
            path: path.into(),
            change: if from.is_some() {
                FileChange::Renamed
            } else {
                FileChange::Changed
            },
            from: from.map(str::to_owned),
            before: None,
            after: None,
            lines: None,
        };
        let t = together(&[file("new.txt", Some("old.txt")), file("soup.txt", None)]);
        assert_eq!(t["old.txt"], ["new.txt", "old.txt"]);
        assert_eq!(t["new.txt"], ["new.txt", "old.txt"]);
        assert_eq!(t["soup.txt"], ["soup.txt"]);
    }
}
