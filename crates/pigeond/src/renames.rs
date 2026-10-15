//! Renames at capture (ADR 0072, 07 §7.3). In one batch, a file deleted
//! from `main` and a file `main` lacks are one file renamed when they hold
//! the same content (the SHA256 their keys name, whatever the extension),
//! or else, for files of at most [`SIMILAR_MAX`] bytes whose contents are
//! here, when at least half of the larger is in both, as git estimates it
//! for its rename detection (diffcore-delta.c: chunks of a line or 64
//! bytes). Each pair becomes a `Pigeon-Renamed: "old" "new"` trailer of the
//! capture commit, which the catalogue follows (05 §5.2). Files in a
//! package and pigeon's own files are never paired.

use std::collections::{BTreeMap, BTreeSet};

use pigeon_core::authority::Tree;
use pigeon_core::names;

use crate::git::{is_pigeon_path, pointer_key};
use crate::group::Group;
use crate::suggest::Changes;

/// The largest file whose rename with an edit is looked for: 1 MiB.
pub const SIMILAR_MAX: u64 = 1 << 20;

/// How many unpaired deleted files, and new ones, may be compared.
const SIMILAR_FILES: usize = 16;

/// The content a key names, without its extension: the hash of
/// `SHA256E-s12--HASH.ext`, else the whole key.
pub fn digest(key: &str) -> &str {
    match key.split_once("--") {
        Some((_, rest)) => rest.split_once('.').map_or(rest, |(h, _)| h),
        None => key,
    }
}

/// The bytes of each chunk of `data`: a line, or 64 bytes of one.
fn chunks(data: &[u8]) -> BTreeMap<&[u8], usize> {
    let mut out: BTreeMap<&[u8], usize> = BTreeMap::new();
    let mut start = 0;
    for (i, b) in data.iter().enumerate() {
        if *b == b'\n' || i + 1 - start == 64 {
            let chunk = data.get(start..=i).unwrap_or_default();
            *out.entry(chunk).or_default() += chunk.len();
            start = i + 1;
        }
    }
    if let Some(rest) = data.get(start..).filter(|r| !r.is_empty()) {
        *out.entry(rest).or_default() += rest.len();
    }
    out
}

/// How much of the larger of `a` and `b` is in both, in thousandths.
pub fn similarity(a: &[u8], b: &[u8]) -> u64 {
    let (ca, cb) = (chunks(a), chunks(b));
    let common: usize = ca
        .iter()
        .map(|(k, n)| cb.get(k).map_or(0, |m| (*n).min(*m)))
        .sum();
    let max = a.len().max(b.len());
    if max == 0 {
        return 1000;
    }
    u64::try_from(common.saturating_mul(1000) / max).unwrap_or(0)
}

/// The renames among `allowed`, the changes a capture commits on `main`:
/// each new file paired with a deleted one of the same content, else with
/// the most similar small one, in path order.
pub async fn detect(g: &Group, main: &Tree, allowed: &Changes) -> Vec<(String, String)> {
    let plain = |p: &str| names::unit(p) == p && !is_pigeon_path(p);
    let gone: Vec<&String> = allowed
        .iter()
        .filter(|(p, e)| e.is_none() && main.contains_key(*p) && plain(p))
        .map(|(p, _)| p)
        .collect();
    let new: Vec<(&String, String)> = allowed
        .iter()
        .filter(|(p, _)| !main.contains_key(*p) && plain(p))
        .filter_map(|(p, e)| Some((p, e.as_ref()?.oid.clone())))
        .collect();
    if gone.is_empty() || new.is_empty() {
        return Vec::new();
    }
    let oids: Vec<String> = (gone.iter().filter_map(|p| main.get(*p)))
        .map(|e| e.oid.clone())
        .chain(new.iter().map(|(_, oid)| oid.clone()))
        .collect();
    let Ok(blobs) = g.git.objects(&oids).await else {
        return Vec::new();
    };
    let key = |b: Option<&Option<(String, Vec<u8>)>>| pointer_key(&b?.as_ref()?.1);
    let old: Vec<(&String, String)> = (gone.iter().enumerate())
        .filter_map(|(i, p)| Some((*p, key(blobs.get(i))?)))
        .collect();
    let added: Vec<(&String, String)> = (new.iter().enumerate())
        .filter_map(|(i, (q, _))| Some((*q, key(blobs.get(gone.len() + i))?)))
        .collect();
    let mut out = Vec::new();
    let mut paired = BTreeSet::new();
    let mut left = Vec::new();
    for (q, k) in &added {
        let same = old
            .iter()
            .find(|(p, o)| !paired.contains(*p) && digest(o) == digest(k));
        match same {
            Some((p, _)) => {
                paired.insert(*p);
                out.push(((*p).clone(), (*q).clone()));
            }
            None => left.push((*q, k)),
        }
    }
    let small = |k: &str| pigeon_core::capacity::key_size(k).is_some_and(|n| n <= SIMILAR_MAX);
    let rest: Vec<&(&String, String)> = old
        .iter()
        .filter(|(p, k)| !paired.contains(*p) && small(k))
        .collect();
    left.retain(|(_, k)| small(k));
    if rest.is_empty()
        || left.is_empty()
        || rest.len() > SIMILAR_FILES
        || left.len() > SIMILAR_FILES
    {
        return out;
    }
    let mut olds = Vec::new();
    for (p, k) in rest {
        if let Some(data) = content(g, k).await {
            olds.push((*p, data));
        }
    }
    for (q, k) in left {
        let Some(data) = content(g, k).await else {
            continue;
        };
        let best = (olds.iter())
            .filter(|(p, _)| !paired.contains(*p))
            .map(|(p, d)| (similarity(d, &data), *p))
            .filter(|(s, _)| *s >= 500)
            .min_by_key(|(s, p)| (std::cmp::Reverse(*s), *p));
        if let Some((score, p)) = best {
            tracing::info!(group = %g.id, from = %p, to = %q, score, "renamed-similar");
            paired.insert(p);
            out.push((p.clone(), q.clone()));
        }
    }
    out
}

/// The content of `key`, if it is here.
async fn content(g: &Group, key: &str) -> Option<Vec<u8>> {
    let path = crate::custody::present(g, key).await?;
    std::fs::read(path).ok()
}

/// The message of a capture commit with `renames`.
pub fn message(renames: &[(String, String)]) -> String {
    let mut m = "pigeon: capture".to_owned();
    if !renames.is_empty() {
        m.push_str("\n\n");
        let lines: Vec<String> = (renames.iter())
            .map(|(a, b)| pigeon_core::authority::renamed_trailer(a, b))
            .collect();
        m.push_str(&lines.join("\n"));
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_digest_ignores_the_extension() {
        let a = "SHA256E-s5--0123abcd.txt";
        assert_eq!(digest(a), "0123abcd");
        assert_eq!(digest("SHA256E-s5--0123abcd.tar.gz"), digest(a));
        assert_eq!(digest("SHA256E-s5--0123abcd"), digest(a));
        assert_eq!(digest("WORM"), "WORM");
    }

    #[test]
    fn similarity_counts_the_chunks_both_hold() {
        let a = b"one\ntwo\nthree\nfour\n";
        assert_eq!(similarity(a, a), 1000);
        assert!(similarity(a, b"one\ntwo\nthree\nfive\n") >= 700);
        assert!(similarity(a, b"zero\n") < 500);
        assert_eq!(similarity(b"", b""), 1000);
        let long = vec![b'x'; 200];
        assert_eq!(similarity(&long, &long[..128]), 640);
    }

    #[test]
    fn a_message_carries_one_trailer_per_rename() {
        let m = message(&[("a b.txt".to_owned(), "c\"d.txt".to_owned())]);
        let raw = format!("tree x\n\n{m}\n");
        let got = pigeon_core::authority::renamed(raw.as_bytes());
        assert_eq!(got, vec![("a b.txt".to_owned(), "c\"d.txt".to_owned())]);
        assert_eq!(message(&[]), "pigeon: capture");
    }
}
