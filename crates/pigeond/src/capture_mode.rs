//! Correct executable bits without reading working contents a second time.
//! A bounded, complete stage-zero index reading supplies existing pointer OIDs;
//! NUL index-info updates modes only, with no annex clean-filter invocation.

use std::collections::{BTreeMap, BTreeSet};

use crate::git::Error;
use crate::group::Group;
use pigeon_sys::fs::lstat;

fn invalid(detail: &str) -> Error {
    Error::Parse {
        argv: vec!["capture index mode".into()],
        detail: detail.into(),
    }
}

fn index_info(output: &[u8], modes: &BTreeMap<&str, &str>) -> Result<Vec<u8>, Error> {
    if !output.ends_with(&[0]) {
        return Err(invalid("staged entries are incomplete"));
    }
    let text = std::str::from_utf8(output).map_err(|_| invalid("invalid staged UTF-8"))?;
    let mut seen = BTreeSet::new();
    let mut input = Vec::new();
    for record in text[..text.len() - 1].split('\0') {
        let (meta, path) = record
            .split_once('\t')
            .ok_or_else(|| invalid("invalid staged entry"))?;
        let Some(mode) = modes.get(path) else {
            return Err(invalid("unrequested staged entry"));
        };
        if !seen.insert(path) {
            return Err(invalid("duplicate staged entry"));
        }
        let parts: Vec<_> = meta.split(' ').collect();
        let [old_mode @ ("100644" | "100755"), oid, "0"] = parts.as_slice() else {
            return Err(invalid("staged entry is not one regular stage-zero file"));
        };
        if !matches!(oid.len(), 40 | 64)
            || !oid.bytes().all(|b| b.is_ascii_hexdigit())
            || oid.bytes().all(|b| b == b'0')
        {
            return Err(invalid("invalid staged object ID"));
        }
        if old_mode != mode {
            input.extend_from_slice(format!("{mode} {oid}\t{path}\0").as_bytes());
        }
    }
    if seen.len() != modes.len() {
        return Err(invalid("missing staged entry"));
    }
    Ok(input)
}

pub(crate) async fn correct(g: &Group, chunk: &[String]) -> Result<(), Error> {
    if chunk.is_empty() {
        return Ok(());
    }
    // git-annex 10.20261006 Annex/Ingest.hs ingestAdd' reads mode from
    // contentLocation, not the working file (Annex/Link.hs stagePointerFile).
    // update-index --chmod may re-clean a later save: retain the staged OID.
    let mut modes = BTreeMap::new();
    for path in chunk {
        if path.is_empty() || path.contains('\0') || modes.contains_key(path.as_str()) {
            return Err(invalid("invalid or repeated capture path"));
        }
        let stat = g
            .worktree
            .as_ref()
            .and_then(|wt| lstat(&wt.join(path)).ok().flatten())
            .filter(|stat| stat.is_regular())
            .ok_or_else(|| invalid("saved file mode is unavailable"))?;
        modes.insert(
            path.as_str(),
            if stat.mode & 0o111 != 0 {
                "100755"
            } else {
                "100644"
            },
        );
    }
    let literal: Vec<_> = chunk
        .iter()
        .map(|path| format!(":(literal){path}"))
        .collect();
    let mut args = vec!["ls-files", "--stage", "-z", "--"];
    args.extend(literal.iter().map(String::as_str));
    let output = g
        .git
        .repo
        .run(&args, None, pigeon_git::DEFAULT_TIMEOUT)
        .await?;
    let input = index_info(&output.stdout, &modes)?;
    if !input.is_empty() {
        g.git
            .run_in(&["update-index", "-z", "--index-info"], &input)
            .await?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "capture_mode_tests.rs"]
mod tests;
