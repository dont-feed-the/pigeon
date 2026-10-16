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

use crate::api::problem;
use crate::daemon::Daemon;
use crate::git::pointer_key;
use crate::group::Group;
use crate::status::msg;
use crate::util::rfc3339;

#[cfg(test)]
mod tests;

fn text(s: &str) -> Arg {
    Arg::Text(s.to_owned())
}

/// A catalogue path for read-only commands. Bare repositories have no
/// working files: resolve their virtual paths lexically, without reading
/// similarly named repository state files or requiring parent folders.
pub(crate) fn relative(g: &Group, path: &Path) -> Option<String> {
    if g.worktree.is_some() {
        return crate::api::relative(g, path);
    }
    let rel = path.strip_prefix(&g.git_dir).ok()?;
    let mut parts = Vec::new();
    for part in rel.components() {
        match part {
            std::path::Component::Normal(part) => parts.push(part.to_str()?),
            std::path::Component::CurDir => {}
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Current history followed by the verified archived histories named by
/// signed refounding trailers. Each archive gets its own in-memory judge.
/// Successful immutable tips are cached without changing the live fold.
pub async fn tips(g: &Group) -> Vec<String> {
    let Some(mut tip) = g.git.rev("refs/heads/main").await else {
        return Vec::new();
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut tips = Vec::new();
    for _ in 0..32 {
        if !seen.insert(tip.clone()) {
            break;
        }
        tips.push(tip.clone());
        let Some(root) = g.git.root(&tip).await else {
            break;
        };
        let Ok(raw) = g.git.object(&root).await else {
            break;
        };
        if pigeon_core::sig::verify(&raw).is_err() {
            break;
        }
        let Some((old, old_main)) = crate::refound::refounded(&raw) else {
            break;
        };
        if g.git
            .rev(&crate::refound::archive_ref(&old))
            .await
            .as_deref()
            != Some(&old_main)
            || g.git.root(&old_main).await.as_deref() != Some(&old)
        {
            break;
        }
        if !g.lock().verified_archives.contains(&old_main) {
            let judge = g.history_judge(&old);
            if !crate::verify::judge(&judge, None, &old_main)
                .await
                .is_ok_and(|j| j.accepted.as_deref() == Some(&old_main))
            {
                break;
            }
            g.lock().verified_archives.insert(old_main.clone());
        }
        tip = old_main;
    }
    tips
}

/// The versions of `rel`, newest first, across main and its verified
/// archive chain, among the last 100 commits touching the path.
pub async fn versions(g: &Group, rel: &str) -> Vec<(String, String)> {
    let mut remaining = 100;
    let mut keys: Vec<(String, String)> = Vec::new();
    let mut last_mode = None;
    for tip in tips(g).await {
        if remaining == 0 {
            break;
        }
        let limit = format!("--max-count={remaining}");
        let out = g
            .git
            .run(&["rev-list", &limit, &tip, "--", rel])
            .await
            .unwrap_or_default();
        for c in out.lines() {
            remaining -= 1;
            let Some(blob) = g.git.blob(&format!("{c}:{rel}")).await else {
                continue;
            };
            let Some(key) = pointer_key(&blob) else {
                continue;
            };
            let Some(mode) = g
                .git
                .tree(c)
                .await
                .ok()
                .and_then(|tree| tree.get(rel).map(|entry| entry.mode.clone()))
            else {
                continue;
            };
            if let Some(last) = keys.last_mut()
                && last.1 == key
                && last_mode.as_ref() == Some(&mode)
            {
                c.clone_into(&mut last.0);
            } else {
                keys.push((c.to_owned(), key));
                last_mode = Some(mode);
            }
        }
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
    vec![Response::History {
        versions: saved_versions(&g, &d.device, &rel).await,
    }]
}

/// Saved-version status shared by the public history command and recovery
/// checks. Confirmed release takes precedence over stale annex copy logs.
pub async fn saved_versions(g: &Group, me: &str, rel: &str) -> Vec<VersionStatus> {
    let k = crate::status::settings(g).k;
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
    let current = crate::status::observe(g, me, rel).await;
    let keys = versions(g, rel).await;
    let n = keys.len();
    let mut versions = Vec::new();
    for (i, (commit, key)) in keys.iter().enumerate() {
        let saved = g.git.commit_time(commit).await.unwrap_or(0);
        let held = crate::custody::holders(&g.git, key).await;
        let copies = custodians.iter().filter(|u| held.contains(*u)).count();
        let policy = g.git.purge_gate.policy();
        let state = match current.iter().find(|o| o.key == *key) {
            _ if !policy.waiting && policy.released.contains(key) => api::State::Erased,
            Some(o) => match o.state {
                pigeon_core::state::State::Protected(_)
                | pigeon_core::state::State::Uncertain(_) => api::State::Protected,
                pigeon_core::state::State::Published(_) => api::State::Published,
                pigeon_core::state::State::Local(_) => api::State::Local,
                pigeon_core::state::State::Missing(_) => api::State::Missing,
            },
            None if copies >= k => api::State::Protected,
            None if copies > 0 => api::State::Published,
            None if crate::custody::present(g, key).await.is_some() => api::State::Local,
            None => api::State::Missing,
        };
        versions.push(VersionStatus {
            path: rel.to_owned(),
            number: u64::try_from(n - i).unwrap_or(0),
            saved_at: rfc3339(saved),
            state,
            observed_at: None,
            reason: None,
        });
    }
    versions
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
    let recovery = doc
        .roster
        .devices
        .get(&id)
        .is_some_and(|d| d.role == DeviceRole::Recovery);
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
        message: msg(
            if recovery {
                "revoked-sheet"
            } else {
                "removed-computer"
            },
            &[("name", text(name))],
        ),
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
    if let Err(detail) = crate::doctor::helper(&d.env).await {
        out.push(problem("transport-helper-unavailable", &[], Some(detail)));
    }
    for g in d.groups() {
        if crate::status::disk_full(&g) {
            let name = crate::status::this_name(&g, &d.device);
            out.push(problem("disk-full", &[("machine", text(&name))], None));
        }
        if g.role == pigeon_core::filter::Role::Custodian {
            match crate::scrub::status(&g) {
                Ok(scrub) => {
                    let name = crate::status::this_name(&g, &d.device);
                    out.push(crate::api::said(
                        "scrub-machine",
                        &[("machine", text(&name))],
                    ));
                    if let Some(at) = scrub.last_pass_completed {
                        out.push(crate::api::said(
                            "scrub-last-pass",
                            &[
                                ("time", text(&at)),
                                (
                                    "failed",
                                    Arg::Number(
                                        i64::try_from(scrub.keys_failed).unwrap_or(i64::MAX),
                                    ),
                                ),
                            ],
                        ));
                    } else {
                        out.push(crate::api::said("scrub-no-pass", &[]));
                    }
                    if let Some(remaining) = scrub.remaining {
                        out.push(crate::api::said(
                            "scrub-progress",
                            &[
                                (
                                    "remaining",
                                    Arg::Number(i64::try_from(remaining).unwrap_or(i64::MAX)),
                                ),
                                (
                                    "failed",
                                    Arg::Number(
                                        i64::try_from(scrub.current_failures.unwrap_or(0))
                                            .unwrap_or(i64::MAX),
                                    ),
                                ),
                            ],
                        ));
                    }
                }
                Err(detail) => {
                    let name = crate::status::this_name(&g, &d.device);
                    out.push(problem(
                        "scrub-ledger",
                        &[("machine", text(&name))],
                        Some(detail),
                    ));
                }
            }
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
        for (peer, report) in crate::report::current(&g) {
            if peer == d.device
                || !names
                    .get(&DeviceId(peer.clone()))
                    .is_some_and(|d| d.role == DeviceRole::Custodian)
            {
                continue;
            }
            out.push(crate::api::said(
                "scrub-machine",
                &[("machine", text(&name(&peer)))],
            ));
            out.push(crate::api::said(
                "scrub-report-time",
                &[("when", text(&report.at.to_string()))],
            ));
            if let Some(scrub) = report.scrub {
                out.push(crate::api::said(
                    "scrub-last-pass",
                    &[
                        ("time", text(&scrub.last_pass_completed.to_string())),
                        (
                            "failed",
                            Arg::Number(i64::try_from(scrub.keys_failed).unwrap_or(i64::MAX)),
                        ),
                    ],
                ));
            } else {
                out.push(crate::api::said("scrub-no-pass", &[]));
            }
        }
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
        None if !out.iter().any(|r| matches!(r, Response::Problem(_))) => {
            out.push(Response::Said {
                message: msg("doctor-ok", &[]),
            });
        }
        None => {}
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

/// This computer’s local diagnostic snapshot (12).
pub async fn snapshot(d: &Daemon) -> serde_json::Value {
    pigeon_telemetry::flush();
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
    serde_json::json!({
        "format": 1,
        "device": d.device,
        "time": rfc3339(now),
        "lab": pigeon_sys::lab::LAB,
        "groups": groups,
        "log_tail": tail,
        "telemetry_tail": pigeon_telemetry::tail(&d.dirs.state.join("telemetry"), 500),
    })
}

/// `pigeon debug bundle`: this computer's local part, saved privately.
pub async fn debug_bundle(d: &Daemon) -> Vec<Response> {
    let bundle = snapshot(d).await;
    write_bundle(d, &bundle)
}

/// Saves a diagnostic bundle locally; no network exporter reads it.
pub fn write_bundle(d: &Daemon, bundle: &serde_json::Value) -> Vec<Response> {
    let now = crate::util::now();
    let dir = crate::api::bundle_dir(d);
    let path = dir.join(format!("bundle-{now}.json"));
    let _ = std::fs::create_dir_all(&dir);
    match serde_json::to_vec_pretty(bundle)
        .map_err(std::io::Error::other)
        .and_then(|b| crate::util::write_private(&path, &b))
    {
        Ok(()) => vec![Response::Said {
            message: msg("bundle-written", &[("path", text(&path.to_string_lossy()))]),
        }],
        Err(e) => vec![problem("internal-error", &[], Some(e.to_string()))],
    }
}
