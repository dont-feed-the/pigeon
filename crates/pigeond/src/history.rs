//! The other commands of the slice: `history`, `remove-computer`, `doctor`
//! and `debug bundle` (10 §10.2). `doctor` says `disk-full` for each
//! repository whose disk is below its reserve (08 §8.9).
//!
//! `history` lists the versions of a file from `main`'s commits, newest
//! first; an older version's state is told from its copies only (slice,
//! 13 §13.12). `remove-computer` commits the removal; repair follows in the
//! next round ([`crate::repair`]).

use std::path::Path;

use pigeon_core::roster::{DeviceId, Edit, Role as DeviceRole, apply, may_sign};
use pigeon_proto::Response;
use pigeon_proto::api::{self, Arg, VersionStatus};

use crate::api::{problem, relative};
use crate::daemon::Daemon;
use crate::git::pointer_key;
use crate::group::Group;
use crate::status::msg;
use crate::util::rfc3339;

fn text(s: &str) -> Arg {
    Arg::Text(s.to_owned())
}

/// The versions of `rel` in `main`'s history, newest first, each with the
/// commit that saved it, among the last 100 that touch it.
pub async fn versions(g: &Group, rel: &str) -> Vec<(String, String)> {
    let out = g
        .git
        .run(&["rev-list", "--max-count=100", "refs/heads/main", "--", rel])
        .await
        .unwrap_or_default();
    let mut keys: Vec<(String, String)> = Vec::new();
    for c in out.lines() {
        let Some(blob) = g.git.blob(&format!("{c}:{rel}")).await else {
            continue;
        };
        let Some(key) = pointer_key(&blob) else {
            continue;
        };
        if let Some(last) = keys.last_mut()
            && last.1 == key
        {
            c.clone_into(&mut last.0);
            continue;
        }
        keys.push((c.to_owned(), key));
    }
    keys
}

/// `pigeon history PATH`.
pub async fn history(d: &Daemon, path: &Path) -> Vec<Response> {
    let Some(g) = d.group_at(path).or_else(|| d.group_at(path.parent()?)) else {
        return vec![problem("not-in-group", &[], None)];
    };
    let Some(rel) = relative(&g, path) else {
        return vec![problem("not-in-group", &[], None)];
    };
    let k = crate::status::settings(&g).k;
    let doc = g.roster();
    let custodians: Vec<String> = doc
        .map(|d| {
            d.roster
                .devices
                .values()
                .filter(|x| x.role == DeviceRole::Custodian)
                .filter_map(|x| x.annex_uuid.clone())
                .collect()
        })
        .unwrap_or_default();
    let current = crate::status::observe(&g, &d.device, &rel).await;
    let keys = versions(&g, &rel).await;
    let n = keys.len();
    let mut versions = Vec::new();
    for (i, (commit, key)) in keys.iter().enumerate() {
        let saved = g.git.commit_time(commit).await.unwrap_or(0);
        let held = crate::custody::holders(&g.git, key).await;
        let copies = custodians.iter().filter(|u| held.contains(*u)).count();
        let state = match current.iter().find(|o| o.key == *key) {
            Some(o) => match o.state {
                pigeon_core::state::State::Protected(_)
                | pigeon_core::state::State::Uncertain(_) => api::State::Protected,
                pigeon_core::state::State::Published(_) => api::State::Published,
                pigeon_core::state::State::Local(_) => api::State::Local,
                pigeon_core::state::State::Missing(_) => api::State::Missing,
            },
            None if copies >= k => api::State::Protected,
            None if copies > 0 => api::State::Published,
            None if crate::custody::present(&g, key).await.is_some() => api::State::Local,
            None => api::State::Missing,
        };
        versions.push(VersionStatus {
            path: rel.clone(),
            number: u64::try_from(n - i).unwrap_or(0),
            saved_at: rfc3339(saved),
            state,
            observed_at: None,
            reason: None,
        });
    }
    vec![Response::History { versions }]
}

/// `pigeon remove-computer NAME`.
pub async fn remove_device(d: &Daemon, name: &str) -> Vec<Response> {
    let Some(g) = d.default_group() else {
        return vec![problem("not-in-group", &[], None)];
    };
    let _busy = g.busy.lock().await;
    let Some(mut doc) = g.roster() else {
        return vec![problem("not-in-group", &[], None)];
    };
    let Some(id) = doc
        .roster
        .devices
        .iter()
        .find(|(_, x)| x.name == name)
        .map(|(id, _)| id.clone())
    else {
        return vec![problem("unknown-computer", &[("name", text(name))], None)];
    };
    let change = [Edit::RemoveDevice(id.clone())];
    if id.0 == d.device || !may_sign(&doc.roster, &DeviceId(d.device.clone()), &change) {
        return vec![problem("not-allowed", &[], None)];
    }
    doc.roster = apply(&doc.roster, &change).roster;
    let message = format!("pigeon: remove {name}");
    if let Err(e) = crate::change::commit_roster(&g, &doc, &message, &d.tracer).await {
        return vec![problem("internal-error", &[], Some(e))];
    }
    d.wake.notify_one();
    vec![Response::Said {
        message: msg("removed-computer", &[("name", text(name))]),
    }]
}

/// `pigeon doctor`: the versions of git and git-annex, and the build.
pub async fn doctor(d: &Daemon, _details: bool) -> Vec<Response> {
    let mut out = Vec::new();
    let git = crate::git::Git::new(d.dirs.state.clone(), d.env.clone());
    let _ = std::fs::create_dir_all(&d.dirs.state);
    let mut bad = None;
    let git_text = git.run(&["--version"]).await.unwrap_or_default();
    match pigeon_git::version::check_git(&git_text) {
        Ok((a, b, c)) => out.push(line("git", &format!("{a}.{b}.{c}"), "2.38.0")),
        Err(e) => bad = Some(problem("update-needed", &[], Some(e.to_string()))),
    }
    let annex_text = git
        .run(&["annex", "version", "--raw"])
        .await
        .unwrap_or_default();
    let (ma, mb) = pigeon_git::version::MIN_ANNEX;
    match pigeon_git::version::check_annex(&annex_text) {
        Ok((a, b)) => out.push(line(
            "git-annex",
            &format!("{a}.{b}"),
            &format!("{ma}.{mb}"),
        )),
        Err(e) => {
            let found = annex_text
                .lines()
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned();
            let minimum = format!("{ma}.{mb}");
            let args = [("found", text(&found)), ("minimum", text(&minimum))];
            bad = Some(problem("git-annex-too-old", &args, Some(e.to_string())));
        }
    }
    if pigeon_sys::lab::LAB {
        out.push(progress(msg("doctor-lab-build", &[])));
    }
    for g in d.groups() {
        if crate::status::disk_full(&g) {
            let name = crate::status::this_name(&g, &d.device);
            out.push(problem("disk-full", &[("machine", text(&name))], None));
        }
        let shortfall = g.lock().shortfall.clone();
        if let Some(s) = shortfall {
            let name = crate::status::this_name(&g, &d.device);
            let args = crate::admission::no_capacity_args(&s);
            let mut args: Vec<(&str, Arg)> = args.into_iter().collect();
            args.push(("machine", text(&name)));
            out.push(problem("no-capacity", &args, None));
        }
    }
    for g in d.groups() {
        let names = g.roster().map(|doc| doc.roster.devices).unwrap_or_default();
        let name = |peer: &str| {
            names
                .get(&DeviceId(peer.to_owned()))
                .map_or(peer.to_owned(), |x| x.name.clone())
        };
        for (code, peer, files, missing) in crate::report::problems(&g) {
            let n = |x: u64| Arg::Number(i64::try_from(x).unwrap_or(i64::MAX));
            let mut args = vec![("machine", text(&name(&peer)))];
            if code == "no-capacity" {
                args.push(("files", n(files)));
                args.push(("missing_mb", n(missing.div_ceil(1_000_000))));
            }
            out.push(problem(code, &args, Some(format!("reported by {peer}"))));
        }
    }
    for g in d.groups() {
        let refused = g.lock().refused.clone();
        let names = g.roster().map(|doc| doc.roster.devices).unwrap_or_default();
        for (peer, (commit, code)) in refused {
            let name = names
                .get(&DeviceId(peer.clone()))
                .map_or(peer.clone(), |x| x.name.clone());
            let args = [
                ("machine", text(&name)),
                ("commit", text(commit.get(..12).unwrap_or(&commit))),
            ];
            out.push(problem(code, &args, Some(format!("{peer} {commit}"))));
        }
    }
    match bad {
        Some(p) => out.push(p),
        None => out.push(Response::Said {
            message: msg("doctor-ok", &[]),
        }),
    }
    out
}

fn progress(message: api::Message) -> Response {
    Response::Progress {
        message,
        done: None,
        total: None,
    }
}

fn line(program: &str, found: &str, minimum: &str) -> Response {
    progress(msg(
        "doctor-version",
        &[
            ("program", text(program)),
            ("found", text(found)),
            ("minimum", text(minimum)),
        ],
    ))
}

/// `pigeon debug bundle`: this device's part, as one JSON file (12).
pub async fn debug_bundle(d: &Daemon) -> Vec<Response> {
    let now = crate::util::now();
    let log = d
        .dirs
        .logs()
        .join(format!("pigeond-{}.ndjson", crate::util::date(now)));
    let tail: Vec<String> = std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .rev()
        .take(500)
        .map(str::to_owned)
        .collect();
    let mut groups = Vec::new();
    for g in d.groups() {
        let roster = g.roster().and_then(|r| crate::roster_file::render(&r).ok());
        let heard = g.lock().heard.clone();
        groups.push(serde_json::json!({
            "id": g.id,
            "role": format!("{:?}", g.role),
            "git_dir": g.git_dir,
            "roster": roster,
            "heard": heard,
            "main": g.git.rev("refs/heads/main").await,
        }));
    }
    let bundle = serde_json::json!({
        "format": 1,
        "device": d.device,
        "time": rfc3339(now),
        "lab": pigeon_sys::lab::LAB,
        "groups": groups,
        "log_tail": tail,
        "telemetry_tail": pigeon_telemetry::tail(&d.dirs.state.join("telemetry"), 500),
    });
    let dir = crate::api::bundle_dir(d);
    let path = dir.join(format!("bundle-{now}.json"));
    let _ = std::fs::create_dir_all(&dir);
    match serde_json::to_vec_pretty(&bundle)
        .map_err(std::io::Error::other)
        .and_then(|b| crate::util::write_private(&path, &b))
    {
        Ok(()) => vec![Response::Said {
            message: msg("bundle-written", &[("path", text(&path.to_string_lossy()))]),
        }],
        Err(e) => vec![problem("internal-error", &[], Some(e.to_string()))],
    }
}
