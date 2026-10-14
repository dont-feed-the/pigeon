//! `git status --porcelain=v2 -z -uall --ignored=traditional`, the changes
//! capture reads (03 §3.7, 07).

/// One entry of the status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// A tracked path changed in the index (`X`) or the working tree (`Y`),
    /// with its modes and object ids in HEAD and the index.
    Changed {
        /// The two status letters, such as `.M`.
        xy: String,
        /// The mode in the working tree, octal.
        mode_worktree: String,
        /// The object id in the index.
        oid_index: String,
        /// The path.
        path: String,
    },
    /// A rename or copy in the index, with its origin.
    Renamed {
        /// The two status letters.
        xy: String,
        /// The new path.
        path: String,
        /// The path it came from.
        from: String,
    },
    /// An unmerged path.
    Unmerged {
        /// The path.
        path: String,
    },
    /// A path git does not track.
    Untracked {
        /// The path.
        path: String,
    },
    /// A path git ignores.
    Ignored {
        /// The path.
        path: String,
    },
}

/// Parses the `-z` output. `Err` names the first entry it cannot read.
pub fn parse(out: &[u8]) -> Result<Vec<Entry>, String> {
    let text = String::from_utf8_lossy(out);
    let mut fields = text.split('\0').filter(|f| !f.is_empty());
    let mut entries = Vec::new();
    while let Some(field) = fields.next() {
        let bad = || format!("bad status entry {field:?}");
        let (tag, rest) = field.split_at(1.min(field.len()));
        let rest = rest.strip_prefix(' ').unwrap_or(rest);
        match tag {
            "1" => {
                let w: Vec<&str> = rest.splitn(8, ' ').collect();
                let [xy, _sub, _mh, _mi, mw, _hh, hi, path] = w.as_slice() else {
                    return Err(bad());
                };
                entries.push(Entry::Changed {
                    xy: (*xy).to_owned(),
                    mode_worktree: (*mw).to_owned(),
                    oid_index: (*hi).to_owned(),
                    path: (*path).to_owned(),
                });
            }
            "2" => {
                let w: Vec<&str> = rest.splitn(9, ' ').collect();
                let [xy, .., path] = w.as_slice() else {
                    return Err(bad());
                };
                let from = fields.next().ok_or_else(bad)?;
                entries.push(Entry::Renamed {
                    xy: (*xy).to_owned(),
                    path: (*path).to_owned(),
                    from: from.to_owned(),
                });
            }
            "u" => {
                let path = rest.splitn(10, ' ').nth(9).ok_or_else(bad)?;
                entries.push(Entry::Unmerged {
                    path: path.to_owned(),
                });
            }
            "?" => entries.push(Entry::Untracked {
                path: rest.to_owned(),
            }),
            "!" => entries.push(Entry::Ignored {
                path: rest.to_owned(),
            }),
            _ => return Err(bad()),
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_each_kind_of_entry() {
        let out = b"1 .M N... 100644 100644 100644 aaa bbb a b.txt\0\
                    2 R. N... 100644 100644 100644 aaa aaa R100 new.txt\0old.txt\0\
                    ? un tracked.txt\0! ignored.o\0";
        let e = parse(out).unwrap();
        assert_eq!(e.len(), 4);
        assert_eq!(
            e[0],
            Entry::Changed {
                xy: ".M".into(),
                mode_worktree: "100644".into(),
                oid_index: "bbb".into(),
                path: "a b.txt".into()
            }
        );
        assert_eq!(
            e[1],
            Entry::Renamed {
                xy: "R.".into(),
                path: "new.txt".into(),
                from: "old.txt".into()
            }
        );
        assert_eq!(
            e[2],
            Entry::Untracked {
                path: "un tracked.txt".into()
            }
        );
        assert_eq!(
            e[3],
            Entry::Ignored {
                path: "ignored.o".into()
            }
        );
        assert!(parse(b"1 .M short\0").is_err());
        assert!(parse(b"x what\0").is_err());
    }
}
