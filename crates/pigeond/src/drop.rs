//! `pigeon free --without-checking PATH` (08 §8.7): the Drop escape. It
//! says first what is at risk — files no backup machine holds yet, and
//! those whose only known copy is this computer's — and, once confirmed,
//! hides every file of PATH whose content is its version, drops that
//! content with `--force` even if it is the last copy, and commits a record
//! `.pigeon/records/drop-<device>-<id>.toml` (device, paths, keys, risk,
//! time) on `main`, which the authority rule lets any member add about its
//! own device (04 §4.3).

use std::collections::BTreeSet;
use std::path::Path;

use pigeon_core::authority;
use pigeon_core::roster::DeviceId;
use pigeon_core::state::State;
use pigeon_proto::Response;
use pigeon_proto::api::Arg;

use crate::api::{group_of, problem, relative, said};
use crate::daemon::Daemon;
use crate::group::Group;
use crate::status::{Observed, msg};

fn number(n: usize) -> Arg {
    Arg::Number(i64::try_from(n).unwrap_or(i64::MAX))
}

/// What dropping `observed` risks: files not protected, and among them
/// those only this computer holds.
fn risk(observed: &[Observed]) -> (usize, usize) {
    let unsafe_ = observed
        .iter()
        .filter(|o| !matches!(o.state, State::Protected(_)))
        .count();
    let alone = observed
        .iter()
        .filter(|o| matches!(o.state, State::Local(_)))
        .count();
    (unsafe_, alone)
}

/// `pigeon free --without-checking PATH`; only says the risk unless
/// `confirmed`.
pub async fn drop(d: &Daemon, path: &Path, confirmed: bool) -> Vec<Response> {
    let Some(g) = group_of(d, path) else {
        return vec![problem("not-in-group", &[], None)];
    };
    let Some(prefix) = relative(&g, path) else {
        return vec![problem("not-in-group", &[], None)];
    };
    let observed = crate::status::observe(&g, &d.device, &prefix).await;
    let (unsafe_, alone) = risk(&observed);
    let warning = Response::Progress {
        message: msg(
            "drop-risk",
            &[
                ("files", number(observed.len())),
                ("unsafe", number(unsafe_)),
                ("alone", number(alone)),
            ],
        ),
        done: None,
        total: None,
    };
    if !confirmed {
        let preview = said("drop-preview", &[("files", number(observed.len()))]);
        return vec![warning, preview];
    }
    let _busy = g.busy.lock().await;
    crate::evict::set_target(&g, &prefix, true);
    let freed = match crate::evict::free_with(&g, &d.device, &prefix, &d.tracer, false).await {
        Ok(f) => f,
        Err(e) => return vec![problem("internal-error", &[], Some(e))],
    };
    let keys: BTreeSet<String> = observed.iter().map(|o| o.key.clone()).collect();
    let why = format!("{unsafe_} files not on enough backup machines, {alone} only here");
    if let Err(e) = record(&g, &d.device, &prefix, &keys, &why).await {
        tracing::warn!(group = %g.id, error = %e, "drop-record-unwritten");
    }
    vec![
        warning,
        said(
            "drop-done",
            &[
                ("files", number(freed.hidden)),
                ("dropped", number(freed.dropped)),
            ],
        ),
    ]
}

fn quoted(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The record's text (08 §8.7).
pub fn render(device: &str, path: &str, keys: &BTreeSet<String>, risk: &str, at: i64) -> String {
    let keys: Vec<String> = keys.iter().map(|k| quoted(k)).collect();
    format!(
        "format = 1\ndevice = {}\npaths = [{}]\nkeys = [{}]\nrisk = {}\ntime = {}\n",
        quoted(device),
        quoted(path),
        keys.join(", "),
        quoted(risk),
        quoted(&crate::util::rfc3339(at)),
    )
}

/// Commits the record of a drop on `main`; the caller holds `busy`.
async fn record(
    g: &Group,
    me: &str,
    path: &str,
    keys: &BTreeSet<String>,
    risk: &str,
) -> Result<String, String> {
    let at = crate::util::now();
    let nonce = crate::util::random::<4>().map_err(|e| e.to_string())?;
    let id = format!("{at}{}", crate::util::hex(&nonce));
    let text = render(me, path, keys, risk, at);
    let main = g
        .git
        .rev("refs/heads/main")
        .await
        .ok_or_else(|| "no main".to_owned())?;
    let blob = g
        .git
        .run_in(&["hash-object", "-w", "--stdin"], text.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    let mut tree = g.git.tree(&main).await.map_err(|e| e.to_string())?;
    tree.insert(
        authority::drop_record_path(&DeviceId(me.to_owned()), &id),
        authority::Entry {
            mode: "100644".to_owned(),
            kind: "blob".to_owned(),
            oid: blob.trim().to_owned(),
        },
    );
    let oid = crate::merge::write_tree(&g.git, &tree)
        .await
        .map_err(|e| e.to_string())?;
    let commit = g
        .git
        .commit_tree(&oid, &[&main], "pigeon: drop without checking")
        .await
        .map_err(|e| e.to_string())?;
    crate::merge::judge_history(g, std::slice::from_ref(&commit))
        .await
        .map_err(|e| e.to_string())?;
    if !g.lock().engine.accepted(&commit) {
        let v = g.lock().verdicts.get(&commit).cloned();
        return Err(format!("not accepted: {v:?}"));
    }
    g.git
        .update_ref("refs/heads/main", &commit, Some(&main))
        .await
        .map_err(|e| e.to_string())?;
    Ok(commit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_names_the_device_path_keys_and_risk() {
        let keys: BTreeSet<String> = ["SHA256E-s1--ab.txt".to_owned()].into();
        let t = render("d1", "Photos \"old\"", &keys, "2 files only here", 0);
        assert!(t.starts_with("format = 1\ndevice = \"d1\"\n"), "{t}");
        assert!(t.contains("paths = [\"Photos \\\"old\\\"\"]"), "{t}");
        assert!(t.contains("keys = [\"SHA256E-s1--ab.txt\"]"), "{t}");
        assert!(t.contains("time = \"1970-01-01T00:00:00Z\""), "{t}");
    }
}
