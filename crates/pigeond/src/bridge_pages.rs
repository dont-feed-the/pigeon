//! Byte-bounded catalogue pages. Cursors name a revision and the last path,
//! so an intervening capture or merge cannot silently mix two visible trees.

use std::collections::BTreeMap;
use std::ops::Bound::{Excluded, Unbounded};

use pigeon_proto::bridge::{Cursor, File, MAX_PAGE_BYTES, MAX_PAGE_FILES, Reply, Snapshot};
use pigeon_proto::{Response, encode};

use super::{Result, err, internal, revision, valid_path};

pub(super) fn page(
    files: &BTreeMap<String, (File, String)>,
    cursor: Option<Cursor>,
    limit: usize,
) -> Result<Reply> {
    if !(1..=MAX_PAGE_FILES).contains(&limit) {
        return Err(err(400, "Invalid catalogue page size"));
    }
    let revision = revision(files);
    let after = if let Some(cursor) = cursor {
        if cursor.revision != revision {
            return Err(err(412, "The catalogue changed; restart the listing"));
        }
        if cursor
            .after
            .as_ref()
            .is_some_and(|p| !valid_path(p) || !files.contains_key(p))
        {
            return Err(err(400, "Invalid catalogue cursor"));
        }
        cursor.after
    } else {
        None
    };
    let start = after.as_ref().map_or(Unbounded, Excluded);
    let mut remaining = files.range::<String, _>((start, Unbounded)).peekable();
    let mut page = Snapshot {
        revision,
        files: Vec::new(),
        next: None,
    };
    // Reserve ample room for the envelope, revision and a maximally escaped
    // cursor path. Account for each file's actual JSON bytes, not its raw name.
    let mut bytes = 4096 + 6 * pigeon_core::names::MAX_PATH;
    while let Some((_, (file, _))) = remaining.peek() {
        let size = serde_json::to_vec(file).map_err(internal)?.len() + 1;
        if page.files.len() == limit || bytes + size > MAX_PAGE_BYTES {
            break;
        }
        bytes += size;
        page.files.push(file.clone());
        remaining.next();
    }
    if remaining.peek().is_some() {
        page.next = Some(Cursor {
            revision: page.revision.clone(),
            after: Some(
                page.files
                    .last()
                    .ok_or_else(|| internal("A catalogue entry exceeds the page budget"))?
                    .path
                    .clone(),
            ),
        });
    }
    let reply = Reply::Snapshot(page);
    // Check the complete wire representation as well, including the longest
    // request ID, to preserve the bound if fields are added in the future.
    if encode(
        u64::MAX,
        &Response::Bridge {
            result: reply.clone(),
        },
    )
    .map_err(internal)?
    .len()
        > MAX_PAGE_BYTES
    {
        return Err(internal("Catalogue page exceeds its byte budget"));
    }
    Ok(reply)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

    use super::*;
    use pigeon_proto::{LineDecoder, MAX_LINE};

    fn files(count: usize) -> BTreeMap<String, (File, String)> {
        (0..count)
            .map(|i| {
                let path = format!("{i:06}/{}/{}/file.txt", "é".repeat(100), "x".repeat(200));
                assert!(valid_path(&path));
                (
                    path.clone(),
                    (
                        File {
                            path,
                            etag: "a".repeat(64),
                            size: i as u64,
                            diverging: i % 2 == 0,
                        },
                        String::new(),
                    ),
                )
            })
            .collect()
    }

    fn snapshot(reply: Reply) -> Snapshot {
        match reply {
            Reply::Snapshot(s) => s,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn large_catalogue_roundtrips_in_byte_bounded_pages_without_gaps() {
        let files = files(6000);
        let unpaged = Reply::Snapshot(Snapshot {
            revision: revision(&files),
            files: files.values().map(|(f, _)| f.clone()).collect(),
            next: None,
        });
        assert!(
            encode(2, &Response::Bridge { result: unpaged })
                .unwrap()
                .len()
                > MAX_LINE
        );
        let mut cursor = None;
        let mut collected = Vec::new();
        let mut pages = 0;
        loop {
            let reply = page(&files, cursor, MAX_PAGE_FILES).unwrap();
            let wire = encode(u64::MAX, &Response::Bridge { result: reply }).unwrap();
            assert!(wire.len() <= MAX_PAGE_BYTES);
            let mut decoder = LineDecoder::new();
            decoder.push(&wire);
            let response = decoder.next_line::<Response>().unwrap().unwrap();
            let Response::Bridge { result } = response.body else {
                panic!("wrong response")
            };
            let page = snapshot(result);
            assert_eq!(page.revision, revision(&files));
            assert!(page.files.len() <= MAX_PAGE_FILES);
            collected.extend(page.files);
            pages += 1;
            cursor = page.next;
            if cursor.is_none() {
                break;
            }
        }
        assert!(pages > 6, "byte budget must shorten these long-name pages");
        assert_eq!(
            collected,
            files.into_values().map(|(f, _)| f).collect::<Vec<_>>()
        );
    }

    #[test]
    fn changed_paths_versions_and_divergences_invalidate_cursors() {
        let original = files(4);
        let cursor = snapshot(page(&original, None, 2).unwrap()).next.unwrap();
        let last = snapshot(page(&original, Some(cursor.clone()), 2).unwrap());
        assert_eq!(last.files.len(), 2);
        assert!(
            last.next.is_none(),
            "exact last page has no empty follow-up"
        );
        for change in 0..4 {
            let mut changed = original.clone();
            match change {
                0 => {
                    changed.pop_first();
                }
                1 => {
                    let mut file = changed.first_key_value().unwrap().1.clone();
                    file.0.path = "000000/new-file.txt".into();
                    changed.insert(file.0.path.clone(), file);
                }
                2 => {
                    changed.first_entry().unwrap().get_mut().0.etag = "b".repeat(64);
                }
                _ => {
                    changed.first_entry().unwrap().get_mut().0.diverging = false;
                }
            }
            assert_eq!(page(&changed, Some(cursor.clone()), 2).unwrap_err().0, 412);
        }
    }

    #[test]
    fn empty_listings_pinned_starts_and_invalid_cursors_are_explicit() {
        let empty = BTreeMap::new();
        let first = snapshot(page(&empty, None, 1).unwrap());
        assert!(first.files.is_empty());
        assert!(first.next.is_none());
        assert_eq!(
            snapshot(
                page(
                    &empty,
                    Some(Cursor {
                        revision: first.revision.clone(),
                        after: None,
                    }),
                    1
                )
                .unwrap()
            ),
            first
        );
        for limit in [0, MAX_PAGE_FILES + 1, usize::MAX] {
            assert_eq!(page(&empty, None, limit).unwrap_err().0, 400);
        }
        for after in ["../outside", "missing.txt"] {
            assert_eq!(
                page(
                    &empty,
                    Some(Cursor {
                        revision: first.revision.clone(),
                        after: Some(after.into()),
                    }),
                    1
                )
                .unwrap_err()
                .0,
                400
            );
        }
    }
}
