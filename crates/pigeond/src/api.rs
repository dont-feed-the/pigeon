//! The requests of the local socket (10 §10.1) that the loops answer.
//!
//! Each answer is a list: progress, then one result. A request that
//! changes a repository holds its lock, so it never runs during a round.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pigeon_core::roster::{Device, DeviceId, Person, PersonId, Role as DeviceRole, Roster};
use pigeon_proto::api::{Arg, Machine, Message, Problem};
use pigeon_proto::join::{Join, JoinAnswer};
use pigeon_proto::{Request, Response};

use crate::daemon::Daemon;
use crate::group::Group;
use crate::net::{Answer, Loops};
use crate::roster_file::RosterDoc;
use crate::status::msg;

/// The loops, as the socket and the join exchange reach them.
pub struct Api(pub Arc<Daemon>);

impl Loops for Api {
    fn request(&self, request: Request) -> Answer<Vec<Response>> {
        let d = self.0.clone();
        Box::pin(async move { handle(&d, request).await })
    }

    fn join(&self, device: String, join: Join) -> Answer<JoinAnswer> {
        let d = self.0.clone();
        Box::pin(async move { crate::invite::welcome(&d, &device, join).await })
    }
}

/// A Problem with its message `problem-<code>`.
pub fn problem(code: &str, args: &[(&str, Arg)], details: Option<String>) -> Response {
    Response::Problem(Problem {
        code: code.to_owned(),
        message: msg(&format!("problem-{code}"), args),
        hint: None,
        details,
    })
}

fn said(id: &str, args: &[(&str, Arg)]) -> Response {
    Response::Said {
        message: msg(id, args),
    }
}

fn text(s: &str) -> Arg {
    Arg::Text(s.to_owned())
}

fn number(n: usize) -> Arg {
    Arg::Number(i64::try_from(n).unwrap_or(i64::MAX))
}

fn not_in_this_build(command: &str) -> Vec<Response> {
    vec![problem(
        "not-in-this-build",
        &[("command", text(command))],
        None,
    )]
}

/// The path of `path` in `g`'s folder, `/`-separated.
pub fn relative(g: &Group, path: &Path) -> Option<String> {
    let wt = g.worktree.as_ref()?;
    let abs = std::fs::canonicalize(path).ok().or_else(|| {
        let parent = std::fs::canonicalize(path.parent()?).ok()?;
        Some(parent.join(path.file_name()?))
    })?;
    let rel = abs.strip_prefix(wt).ok()?;
    Some(
        rel.components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

fn group_of(d: &Daemon, path: &Path) -> Option<Arc<Group>> {
    d.group_at(path).or_else(|| d.group_at(path.parent()?))
}

async fn handle(d: &Daemon, request: Request) -> Vec<Response> {
    match request {
        Request::Create {
            name,
            folder,
            k,
            device_name,
        } => vec![create(d, &name, Path::new(&folder), k, &device_name).await],
        Request::Status { path, not_shared } => {
            let (g, prefix) = match &path {
                Some(p) => match group_of(d, Path::new(p)) {
                    Some(g) => {
                        let prefix = relative(&g, Path::new(p)).unwrap_or_default();
                        (g, prefix)
                    }
                    None => return vec![problem("not-in-group", &[], None)],
                },
                None => match d.default_group() {
                    Some(g) => (g, String::new()),
                    None => return vec![problem("not-in-group", &[], None)],
                },
            };
            // Changes not yet captured are only on this computer: capture
            // them first, unless a round is already at work.
            if g.role == pigeon_core::filter::Role::Laptop
                && let Ok(_busy) = g.busy.try_lock()
                && let Err(e) = crate::capture::capture(&g, &d.tracer).await
            {
                tracing::warn!(group = %g.id, error = %e, "capture-failed");
            }
            vec![Response::Status(
                crate::status::report(&g, &d.device, &prefix, not_shared).await,
            )]
        }
        Request::Invite {
            person,
            custodian,
            name,
            ..
        } => {
            if !custodian {
                return not_in_this_build("invite");
            }
            let Some(g) = d.default_group() else {
                return vec![problem("not-in-group", &[], None)];
            };
            match crate::invite::invite(d, &g, &person, name.as_deref().unwrap_or_default()) {
                Ok(code) => vec![said("invitation", &[("code", text(&code))])],
                Err(code) => vec![problem(code, &[], None)],
            }
        }
        Request::Join {
            invitation, folder, ..
        } => match crate::invite::join(d, &invitation, folder).await {
            Ok(group) => vec![said("joined", &[("group", text(&group))])],
            Err(code) => vec![join_problem(&code, &invitation)],
        },
        Request::Evict { folder } => free(d, Path::new(&folder)).await,
        Request::Keep { folder } => keep(d, Path::new(&folder)).await,
        Request::Machines => machines(d),
        Request::History { path } => crate::history::history(d, Path::new(&path)).await,
        Request::RemoveDevice { name } => crate::history::remove_device(d, &name).await,
        Request::Doctor { details } => crate::history::doctor(d, details).await,
        Request::DebugBundle { .. } => crate::history::debug_bundle(d).await,
        Request::Restore { path, version, at } if at.is_none() => {
            crate::restore::restore(d, Path::new(&path), version).await
        }
        Request::Restore { .. } => not_in_this_build("restore"),
        Request::RecoverySheet => crate::recovery::sheet(d).await,
        Request::Recover {
            sheet,
            folder,
            from,
        } => crate::recovery::recover(d, &sheet, folder, from).await,
        Request::Hello { .. }
        | Request::Address { .. }
        | Request::Connect { .. }
        | Request::Listen { .. } => not_in_this_build("helper"),
    }
}

/// The problem of a failed join, with the arguments its messages take.
fn join_problem(code: &str, invitation: &str) -> Response {
    let inv = crate::invite::decode(invitation);
    let inviter = inv
        .as_ref()
        .map(|i| {
            if i.from.is_empty() {
                i.inviter.chars().take(8).collect()
            } else {
                i.from.clone()
            }
        })
        .unwrap_or_default();
    let who = [("inviter", text(&inviter))];
    let with_hint = |p: Response, hint: &str| match p {
        Response::Problem(mut p) => {
            p.hint = Some(msg(hint, &who));
            Response::Problem(p)
        }
        other => other,
    };
    match code {
        "invite-expired" => {
            let until = inv.map_or_else(String::new, |i| crate::util::rfc3339(i.expires));
            let p = problem(code, &[("until", text(&until))], None);
            with_hint(p, "hint-invite-expired")
        }
        "invite-used" => with_hint(problem(code, &[], None), "hint-invite-used"),
        "inviter-offline" => with_hint(problem(code, &who, None), "hint-inviter-offline"),
        "invite-not-allowed" => problem(code, &who, None),
        "invite-unknown" | "not-in-group" | "not-allowed" => problem(code, &[], None),
        "not-in-this-build" => problem(code, &[("command", text("join"))], None),
        other => problem("internal-error", &[], Some(other.to_owned())),
    }
}

fn person_id() -> (String, String) {
    let user = std::env::var("USER").unwrap_or_default();
    let id: String = user
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if id.is_empty() {
        ("me".to_owned(), "Me".to_owned())
    } else {
        (id, user)
    }
}

async fn create(d: &Daemon, name: &str, folder: &Path, k: u32, device_name: &str) -> Response {
    match create_inner(d, name, folder, k, device_name).await {
        Ok(()) => said("created", &[("group", text(name))]),
        Err(e) => problem("internal-error", &[], Some(e)),
    }
}

async fn create_inner(
    d: &Daemon,
    name: &str,
    folder: &Path,
    k: u32,
    device_name: &str,
) -> Result<(), String> {
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let folder = std::fs::canonicalize(folder).map_err(|e| e.to_string())?;
    if folder.join(".git").exists() {
        return Err("the folder already has a repository".into());
    }
    let git = crate::git::Git::new(&folder, d.env.clone());
    let fail = |e: crate::git::Error| e.to_string();
    git.run(&["init", "-q", "-b", "main"]).await.map_err(fail)?;
    git.run(&["config", "user.name", device_name])
        .await
        .map_err(fail)?;
    let email = format!("{}@pigeon", d.device);
    git.run(&["config", "user.email", &email])
        .await
        .map_err(fail)?;
    let pending = Group::open("pending", &folder, &d.env)
        .await
        .ok_or("no repository")?;
    crate::settings::check(&pending).await;
    let (uuid, token) =
        crate::settings::init_annex(&git, pigeon_core::filter::Role::Laptop, device_name, k)
            .await
            .map_err(fail)?;
    let k_text = k.to_string();
    git.run(&["annex", "numcopies", &k_text])
        .await
        .map_err(fail)?;
    git.run(&["annex", "mincopies", &k_text])
        .await
        .map_err(fail)?;
    let (pid, pname) = person_id();
    let roster = Roster {
        rules: 1,
        group: name.to_owned(),
        k,
        retention_days: 30,
        persons: BTreeMap::from([(
            PersonId(pid.clone()),
            Person {
                name: pname,
                admin: true,
            },
        )]),
        devices: BTreeMap::from([(
            DeviceId(d.device.clone()),
            Device {
                person: PersonId(pid),
                name: device_name.to_owned(),
                role: DeviceRole::Laptop,
                annex_uuid: Some(uuid),
                maxsize: None,
            },
        )]),
        removed: BTreeMap::new(),
        folders: Vec::new(),
    };
    roster.validate().map_err(|e| format!("{e:?}"))?;
    let doc = RosterDoc { roster };
    let text = crate::roster_file::render(&doc)?;
    let stage = |p: &'static str, b: Vec<u8>| {
        let git = git.clone();
        let folder = folder.clone();
        async move { crate::change::stage_file(&git, Some(&folder), p, &b).await }
    };
    stage(
        ".gitattributes",
        crate::settings::GITATTRIBUTES.as_bytes().to_vec(),
    )
    .await
    .map_err(fail)?;
    stage(".pigeon/roster.toml", text.into_bytes())
        .await
        .map_err(fail)?;
    let genesis = crate::change::commit_index(&git, None, &format!("pigeon: create {name}"))
        .await
        .map_err(fail)?;
    drop(pending);
    let g = Group::open(&genesis, &folder, &d.env)
        .await
        .ok_or("no repository")?;
    g.set_token(&token);
    tracing::info!(group = %genesis, "created");
    d.add_group(g, &folder).await;
    Ok(())
}

/// Starts a round shortly after the answer is on its way, so that a crash
/// in the round never takes the answer with it (13 §13.12, S9).
fn wake_soon(d: &Daemon) {
    let wake = d.wake.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        wake.notify_one();
    });
}

/// `pigeon free`: records the target; the loops hide each protected file
/// and drop its content, and the others once they are protected (08 §8.5).
async fn free(d: &Daemon, path: &Path) -> Vec<Response> {
    let Some(g) = group_of(d, path) else {
        return vec![problem("not-in-group", &[], None)];
    };
    let Some(prefix) = relative(&g, path) else {
        return vec![problem("not-in-group", &[], None)];
    };
    crate::evict::set_target(&g, &prefix, true);
    let observed = crate::status::observe(&g, &d.device, &prefix).await;
    let safe = observed
        .iter()
        .filter(|o| matches!(o.state, pigeon_core::state::State::Protected(_)))
        .count();
    let waiting = observed.len() - safe;
    wake_soon(d);
    let mut out = vec![said(
        "free-result",
        &[("safe", number(safe)), ("total", number(observed.len()))],
    )];
    if waiting > 0 {
        out.insert(
            0,
            Response::Progress {
                message: msg("free-waiting", &[("files", number(waiting))]),
                done: None,
                total: None,
            },
        );
    }
    out
}

/// `pigeon keep`: records the target; the loops bring the folder back.
async fn keep(d: &Daemon, path: &Path) -> Vec<Response> {
    let Some(g) = group_of(d, path) else {
        return vec![problem("not-in-group", &[], None)];
    };
    let Some(prefix) = relative(&g, path) else {
        return vec![problem("not-in-group", &[], None)];
    };
    crate::evict::set_target(&g, &prefix, false);
    wake_soon(d);
    vec![said("keep-result", &[("folder", text(&prefix))])]
}

fn machines(d: &Daemon) -> Vec<Response> {
    let Some(g) = d.default_group() else {
        return vec![problem("not-in-group", &[], None)];
    };
    let Some(doc) = g.roster() else {
        return vec![problem("not-in-group", &[], None)];
    };
    let heard = g.lock().heard.clone();
    let machines = doc
        .roster
        .devices
        .iter()
        .map(|(id, x)| Machine {
            name: x.name.clone(),
            device: id.0.clone(),
            custodian: x.role == DeviceRole::Custodian,
            heard_at: heard.get(&id.0).map(|t| crate::util::rfc3339(*t)),
            this_device: id.0 == d.device,
        })
        .collect();
    vec![Response::Machines { machines }]
}

/// The message of a text, for the modules that answer with one.
pub fn message(id: &str, args: &[(&str, Arg)]) -> Message {
    msg(id, args)
}

/// Where a debug bundle is written.
pub fn bundle_dir(d: &Daemon) -> PathBuf {
    d.dirs.state.join("bundles")
}
