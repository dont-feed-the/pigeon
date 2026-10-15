//! `pigeon held` (07 §7.4, 10 §10.1): the held mass deletions and names,
//! and the person's choice for each batch. *Delete them for everyone*
//! releases its paths, which the next capture deletes whatever their
//! number; *Bring them back* takes each path whose file is still gone out
//! of the index, so that install writes `main`'s version again, as for a
//! new file. *Rename to the suggested name* renames a held name's prefix
//! at fault on disk, never over a file (05 §5.4).

use pigeon_proto::Response;
use pigeon_proto::api::{Arg, HeldChoice, HeldEntry};

use crate::api::problem;
use crate::daemon::Daemon;
use crate::held_file::{Batch, NAME};
use crate::status::msg;
use crate::util::rfc3339;

fn number(n: usize) -> Arg {
    Arg::Number(i64::try_from(n).unwrap_or(i64::MAX))
}

fn entry(b: &Batch) -> HeldEntry {
    let first = || Arg::Text(b.paths.iter().next().cloned().unwrap_or_default());
    let reason = match (&b.folder, &b.to) {
        (_, Some(to)) if b.why == NAME => msg(
            "held-name",
            &[("path", first()), ("suggested", Arg::Text(to.clone()))],
        ),
        _ if b.why == NAME => msg("held-name-long", &[("path", first())]),
        (Some(f), _) if b.why == "top-folder" => {
            msg("held-top-folder", &[("folder", Arg::Text(f.clone()))])
        }
        _ => msg("held-mass-deletion", &[("files", number(b.paths.len()))]),
    };
    HeldEntry {
        id: b.id.clone(),
        reason,
        files: u64::try_from(b.paths.len()).unwrap_or(u64::MAX),
        since: rfc3339(b.since),
        paths: b.paths.iter().take(10).cloned().collect(),
        suggested: b.to.clone().filter(|_| b.why == NAME),
    }
}

/// `pigeon held`.
pub fn list(d: &Daemon) -> Vec<Response> {
    let Some(g) = d.default_group() else {
        return vec![problem("not-in-group", &[], None)];
    };
    let table = crate::held_file::read(&g);
    vec![Response::Held {
        held: table.batches.iter().map(entry).collect(),
    }]
}

/// `pigeon held ID --share | --bring-back`.
pub async fn release(d: &Daemon, id: &str, choice: HeldChoice) -> Vec<Response> {
    let Some(g) = d.default_group() else {
        return vec![problem("not-in-group", &[], None)];
    };
    let _busy = g.busy.lock().await;
    let mut table = crate::held_file::read(&g);
    let Some(batch) = table.take(id) else {
        return vec![problem(
            "held-unknown",
            &[("id", Arg::Text(id.to_owned()))],
            None,
        )];
    };
    let renames = choice == HeldChoice::Rename;
    if renames != (batch.why == NAME) || renames && batch.to.is_none() {
        return vec![problem(
            "held-choice",
            &[("id", Arg::Text(id.to_owned()))],
            None,
        )];
    }
    let files = number(batch.paths.len());
    let said = match choice {
        HeldChoice::Share => {
            table.released.extend(batch.paths);
            "held-shared"
        }
        HeldChoice::BringBack => {
            let worktree = g.worktree.clone().unwrap_or_default();
            let gone: Vec<&str> = batch
                .paths
                .iter()
                .filter(|p| matches!(pigeon_sys::fs::lstat(&worktree.join(p.as_str())), Ok(None)))
                .map(String::as_str)
                .collect();
            for chunk in gone.chunks(64) {
                let mut args = vec!["update-index", "--force-remove", "--"];
                args.extend(chunk);
                if let Err(e) = g.git.run(&args).await {
                    return vec![problem("internal-error", &[], Some(e.to_string()))];
                }
            }
            "held-brought-back"
        }
        HeldChoice::Rename => {
            let worktree = g.worktree.clone().unwrap_or_default();
            let (from, to) = (batch.from.unwrap_or_default(), batch.to.unwrap_or_default());
            if !matches!(pigeon_sys::fs::lstat(&worktree.join(&to)), Ok(None)) {
                return vec![problem("held-rename-taken", &[("to", Arg::Text(to))], None)];
            }
            if let Err(e) = std::fs::rename(worktree.join(&from), worktree.join(&to)) {
                return vec![problem("internal-error", &[], Some(e.to_string()))];
            }
            table
                .batches
                .retain(|b| b.why != NAME || !b.from.as_ref().is_some_and(|f| *f == from));
            if let Err(e) = crate::held_file::write(&g, &table) {
                return vec![problem("internal-error", &[], Some(e.to_string()))];
            }
            tracing::info!(group = %g.id, id, from = %from, to = %to, "held-renamed");
            d.wake.notify_one();
            return vec![Response::Said {
                message: msg(
                    "held-renamed",
                    &[("from", Arg::Text(from)), ("to", Arg::Text(to))],
                ),
            }];
        }
    };
    if let Err(e) = crate::held_file::write(&g, &table) {
        return vec![problem("internal-error", &[], Some(e.to_string()))];
    }
    tracing::info!(group = %g.id, id, choice = ?choice, "held-released");
    d.wake.notify_one();
    vec![Response::Said {
        message: msg(said, &[("files", files)]),
    }]
}
