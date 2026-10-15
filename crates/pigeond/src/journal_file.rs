//! `.git/pigeon/install.ndjson` (11 §11.3) as lines, to and from
//! pigeon-core's journal records (07 §7.5). Each record is appended and
//! fsynced before the step it announces; a torn last line is ignored.
//!
//! Law (`tests/codecs.rs`): a record parses back from its line.

use std::path::Path;

use pigeon_core::journal::{Begin, End, Record, Stat};
use serde_json::{Value, json};

fn end_name(e: End) -> &'static str {
    match e {
        End::Done => "done",
        End::Skip => "skip",
        End::Raced => "raced",
        End::Recovered => "recovered",
    }
}

/// The line of a record, newline included.
pub fn line(r: &Record) -> String {
    let v = match r {
        Record::Begin(b) => json!({
            "format": 1,
            "seq": b.seq,
            "op": "begin",
            "path": b.path,
            "old_key": b.old_key,
            "old": b.stat.map(|s| json!([s.dev, s.ino, s.size, s.mtime_ns.to_string()])),
            "new_key": b.new_key,
            "tmp": b.stage,
            "aside": b.aside,
        }),
        Record::Proved(seqs) => json!({"format": 1, "op": "proved", "seqs": seqs}),
        Record::End(seq, e) => json!({"format": 1, "seq": seq, "op": end_name(*e)}),
        Record::Released(seq) => json!({"format": 1, "seq": seq, "op": "released"}),
        Record::Kept(seq, at) => json!({"format": 1, "seq": seq, "op": "kept", "at": at}),
        Record::Preserved(seq, path) => {
            json!({"format": 1, "seq": seq, "op": "preserved", "path": path})
        }
    };
    format!("{v}\n")
}

fn text(v: &Value, k: &str) -> Option<String> {
    v.get(k)?.as_str().map(str::to_owned)
}

/// The record of a line, `None` if it is not one.
pub fn parse(line: &str) -> Option<Record> {
    let v: Value = serde_json::from_str(line).ok()?;
    let seq = v.get("seq").and_then(Value::as_u64);
    Some(match v.get("op")?.as_str()? {
        "begin" => {
            let stat = match v.get("old") {
                Some(Value::Array(a)) => {
                    let n = |i: usize| a.get(i).and_then(Value::as_u64);
                    let mtime = a.get(3)?;
                    let mtime_ns = mtime
                        .as_str()
                        .and_then(|s| s.parse().ok())
                        .or_else(|| mtime.as_i64().map(i128::from))?;
                    Some(Stat {
                        dev: n(0)?,
                        ino: n(1)?,
                        size: n(2)?,
                        mtime_ns,
                    })
                }
                _ => None,
            };
            Record::Begin(Begin {
                seq: seq?,
                path: text(&v, "path")?,
                old_key: text(&v, "old_key"),
                new_key: text(&v, "new_key"),
                stat,
                stage: text(&v, "tmp"),
                aside: text(&v, "aside")?,
            })
        }
        "proved" => match v.get("seqs") {
            Some(Value::Array(a)) => Record::Proved(a.iter().filter_map(Value::as_u64).collect()),
            _ => Record::Proved(vec![seq?]),
        },
        "done" => Record::End(seq?, End::Done),
        "skip" => Record::End(seq?, End::Skip),
        "raced" => Record::End(seq?, End::Raced),
        "recovered" => Record::End(seq?, End::Recovered),
        "released" => Record::Released(seq?),
        "kept" => Record::Kept(seq?, v.get("at")?.as_i64()?),
        "preserved" => Record::Preserved(seq?, text(&v, "path")?),
        _ => return None,
    })
}

/// Every record of the journal at `path`; a torn or unknown line is skipped.
pub fn read(path: &Path) -> Vec<Record> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(parse)
        .collect()
}

/// Appends `r` and fsyncs it.
pub fn append(path: &Path, r: &Record) -> std::io::Result<()> {
    pigeon_sys::fs::append_durable(path, line(r).as_bytes())
        .map_err(|e| std::io::Error::other(e.to_string()))
}
