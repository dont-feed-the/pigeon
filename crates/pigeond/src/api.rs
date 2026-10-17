//! The requests of the local socket (10 §10.1) that the loops answer.
//!
//! Each answer is a list: progress, then one result. A request that
//! changes a repository holds its lock, so it never runs during a round.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pigeon_core::roster::{Device, DeviceId, Person, Role as DeviceRole, Roster};
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
        Box::pin(async move {
            let mut span = pigeon_telemetry::span("api.request");
            span.set("device", d.device.clone());
            if let Some(g) = d.default_group() {
                span.set("group", g.id.clone());
            }
            span.in_scope(handle(&d, request)).await
        })
    }

    fn recovery_bundle(
        &self,
        group: String,
        device: String,
    ) -> Answer<Option<crate::net::recovery::Bundle>> {
        let d = self.0.clone();
        Box::pin(async move { crate::recovery::bundle(&d, &group, &device).await })
    }

    fn diagnostics(&self, group: String, device: String) -> Answer<Option<serde_json::Value>> {
        let d = self.0.clone();
        Box::pin(async move { crate::debug::share(&d, &group, &device).await })
    }

    fn telemetry(
        &self,
        group: String,
        device: String,
        records: Vec<serde_json::Value>,
    ) -> Answer<bool> {
        let d = self.0.clone();
        Box::pin(async move { crate::telemetry::receive(&d, &group, &device, records).await })
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

pub(crate) fn said(id: &str, args: &[(&str, Arg)]) -> Response {
    Response::Said {
        message: msg(id, args),
    }
}

fn text(s: &str) -> Arg {
    Arg::Text(s.to_owned())
}

/// The `who` of the `joined` message: what joins.
fn who(custodian: bool) -> &'static str {
    if custodian {
        "backup-machine"
    } else {
        "computer"
    }
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
    if g.shared.is_some() {
        // Sparse catalogues intentionally have no on-disk parent folders.
        // Reuse restore's lexical resolution and symlink/repository guards.
        return crate::restore::path::relative(g, path);
    }
    let wt = g.worktree.as_ref()?;
    // A path in a folder this computer does not keep is absent from disk
    // (08 §8.5): the nearest folder that exists, then the names below it.
    let mut missing = Vec::new();
    let mut existing = path;
    while !existing.exists() {
        missing.push(existing.file_name()?);
        existing = existing.parent()?;
    }
    let mut abs = std::fs::canonicalize(existing).ok()?;
    abs.extend(missing.iter().rev());
    let rel = abs.strip_prefix(wt).ok()?;
    Some(
        rel.components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

pub(crate) fn group_of(d: &Daemon, path: &Path) -> Option<Arc<Group>> {
    d.group_at(path).or_else(|| d.group_at(path.parent()?))
}

pub(crate) async fn handle(d: &Arc<Daemon>, request: Request) -> Vec<Response> {
    match request {
        Request::Erase { operation } => crate::erase::handle(d, operation).await,
        Request::FolderRules { folder } => crate::folder_rules::list(d, folder.as_deref()),
        Request::SetFolderRule {
            folder,
            rule,
            confirmed,
        } => crate::folder_rules::set(d, &folder, rule, confirmed).await,
        Request::Capacity => crate::capacity_view::report(d).await,
        Request::Health => crate::health::report(d).await,
        Request::Bridge { operation } => crate::bridge::handle(d, operation).await,
        Request::Create {
            name,
            folder,
            k,
            device_name,
            person,
        } => match in_another_group(d, &[]) {
            Some(refused) => vec![refused],
            None => {
                let new = New {
                    name: &name,
                    k,
                    device_name: &device_name,
                    person: person.as_deref(),
                };
                vec![create(d, &new, Path::new(&folder)).await]
            }
        },
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
                && let Err(e) = crate::capture::capture(&g, &d.device, &d.tracer).await
            {
                tracing::warn!(group = %g.id, error = %e, "capture-failed");
            }
            let mut report = crate::status::report(&g, &d.device, &prefix, not_shared).await;
            // A pause the person chose says so, in the headline unless
            // something needs them (15 §15.6, ADR 0027).
            crate::pause::tell(d, &mut report);
            vec![Response::Status(report)]
        }
        Request::Invite => {
            let Some(g) = d.default_group() else {
                return vec![problem("not-in-group", &[], None)];
            };
            crate::refound::remember(&g).await;
            match crate::invite::code(d, &g) {
                Some(code) => vec![invitation(&g, &code)],
                None => vec![problem("not-in-group", &[], None)],
            }
        }
        Request::Join {
            invitation,
            folder,
            name,
            person,
            existing,
            custodian,
            ..
        } => {
            if let Some(f) = folder.as_deref().filter(|f| !Path::new(f).is_absolute()) {
                return vec![problem("folder-not-absolute", &[("folder", text(f))], None)];
            }
            let code = pigeon_proto::join::invitation(&invitation);
            // A refounded group's invitation also names the old group,
            // which joining it in place replaces (09 §9.9).
            let asked: Vec<String> = pigeon_proto::join::decode(&code)
                .map(|i| [Some(i.group), i.refounded.map(|r| r.from)])
                .into_iter()
                .flatten()
                .flatten()
                .collect();
            if let Some(refused) = in_another_group(d, &asked) {
                return vec![refused];
            }
            let ask = crate::invite::Ask {
                code: code.clone(),
                folder,
                name: name.clone(),
                person: person.clone(),
                existing,
                custodian,
            };
            let group_id = pigeon_proto::join::decode(&code).map(|i| i.group);
            match crate::invite::join(d, ask).await {
                Ok(group) => {
                    let mut args = vec![("group", text(&group)), ("who", text(who(custodian)))];
                    match group_id.and_then(|id| renamed(d, &id, name)) {
                        Some((given, asked)) => {
                            args.extend([("name", text(&given)), ("asked", text(&asked))]);
                            vec![said("joined-renamed", &args)]
                        }
                        None => vec![said("joined", &args)],
                    }
                }
                Err(e) => vec![join_problem(&e, &code, person.as_deref())],
            }
        }
        Request::Evict { folder, confirmed } => free(d, Path::new(&folder), confirmed).await,
        Request::FreePreview { folder } => crate::free_check::preview(d, Path::new(&folder)).await,
        Request::Drop { path, confirmed } => {
            crate::drop::drop(d, Path::new(&path), confirmed).await
        }
        Request::Keep { folder } => keep(d, Path::new(&folder)).await,
        Request::Sync { folder } => crate::sync::list(d, folder.as_deref().map(Path::new)).await,
        Request::ForgetChoice { folder } => crate::sync::forget(d, Path::new(&folder)).await,
        Request::Machines => machines(d),
        Request::People => people(d).await,
        Request::OpenWeb => match d.web.get() {
            Some(web) => vec![said("web-login", &[("url", text(&web.login_url()))])],
            None => not_in_this_build("open"),
        },
        Request::Folders => match d.default_group() {
            Some(g) => vec![crate::folders::folders(&g, &d.device).await],
            None => vec![problem("not-in-group", &[], None)],
        },
        Request::Files { folder } => {
            let path = Path::new(&folder);
            match group_of(d, path) {
                Some(g) => {
                    let prefix = relative(&g, path).unwrap_or_default();
                    vec![crate::folders::files(&g, &d.device, &prefix).await]
                }
                None => vec![problem("not-in-group", &[], None)],
            }
        }
        Request::Deleted { path: None } => match d.default_group() {
            Some(g) => vec![crate::folders::deleted(&g, "").await],
            None => vec![problem("not-in-group", &[], None)],
        },
        Request::Deleted { path: Some(path) } => {
            let path = Path::new(&path);
            match group_of(d, path).and_then(|g| Some((relative(&g, path)?, g))) {
                Some((prefix, g)) => vec![crate::folders::deleted(&g, &prefix).await],
                None => vec![problem("not-in-group", &[], None)],
            }
        }
        Request::GetSettings { key } => crate::prefs::get(d, key.as_deref()),
        Request::SetSettings { key, value } if key == "telemetry-device" => {
            crate::telemetry::set_collector(d, value.trim()).await
        }
        Request::SetSettings { key, value } => crate::prefs::set(d, &key, &value),
        Request::Pause { duration } => crate::pause::pause(d, duration.as_deref()),
        Request::Resume => crate::pause::resume(d),
        Request::Paused => crate::pause::state(d),
        Request::History { path } => crate::history::history(d, Path::new(&path)).await,
        Request::ReadText { path, limit } => crate::view::text(d, Path::new(&path), limit).await,
        Request::ViewSuggestion { proposal, path } => {
            crate::view::suggestion(d, &proposal, Path::new(&path)).await
        }
        Request::View {
            path,
            version,
            group,
        } => crate::view::view(d, Path::new(&path), version, group).await,
        Request::RemoveDevice { name } => crate::history::remove_device(d, &name).await,
        Request::RemoveDeviceId { device } => crate::history::remove_device_id(d, &device).await,
        Request::Leave => crate::leave::leave(d).await,
        Request::Refound { repository } => {
            crate::refound::refound(d, repository.as_deref().map(Path::new)).await
        }
        Request::Held => crate::held::list(d),
        Request::ReleaseHeld { batch, choice } => crate::held::release(d, &batch, choice).await,
        Request::Doctor { details } => crate::history::doctor(d, details).await,
        Request::DebugBundle { peers } => crate::debug::collect(d, &peers).await,
        Request::AllowDebug { peer } => crate::debug::allow(d, &peer),
        Request::Restore { path, version, at } => {
            crate::restore::restore(d, Path::new(&path), version, at.as_deref()).await
        }
        Request::RecoverySheet => crate::recovery::sheet(d).await,
        Request::Recover {
            sheet,
            folder,
            from,
        } => {
            let asked: Vec<String> = crate::recovery::decode(&sheet)
                .map(|s| s.group)
                .into_iter()
                .collect();
            if let Some(refused) = in_another_group(d, &asked) {
                return vec![refused];
            }
            crate::recovery::recover(d, &sheet, folder, from).await
        }
        Request::Proposals => crate::decide::list_detailed(d).await,
        Request::Conflicts => crate::resolve::list(d).await,
        Request::Resolve { path, choice } => {
            crate::resolve::resolve(d, Path::new(&path), &choice).await
        }
        Request::Accept { proposal, reason } => {
            crate::decide::accept(d, &proposal, reason.as_deref()).await
        }
        Request::Reject { proposal, note } => {
            crate::decide::decline(d, &proposal, note.as_deref()).await
        }
        Request::DecideFiles {
            proposal,
            paths,
            accept,
            note,
            reason,
        } => {
            let (note, reason) = (note.as_deref(), reason.as_deref());
            crate::decide_files::decide(d, &proposal, &paths, accept, note, reason).await
        }
        Request::Hello { .. }
        | Request::Address { .. }
        | Request::Connect { .. }
        | Request::Listen { .. } => not_in_this_build("helper"),
    }
}

/// After a join, this computer's name in the roster of `group` and the
/// one asked for, when the admitter gave it another because a computer had
/// it (09 §9.6 step 3). A name not asked for is unique already.
fn renamed(d: &Daemon, group: &str, name: Option<String>) -> Option<(String, String)> {
    let asked = name
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty())?;
    let doc = d.group(group)?.roster()?;
    let given = doc
        .roster
        .devices
        .get(&DeviceId(d.device.clone()))?
        .name
        .clone();
    (given != asked).then_some((given, asked))
}

/// A computer is in one group only: the web pages and the commands without
/// a path show one group (09 §9.6). Refuses creating, joining or
/// recovering another group while this computer is in one, saying how to
/// leave it. A request about a group this computer is in, `asked` (the
/// group, or the one a refounded group replaces), goes on: it is
/// `already-in-group`, or a join in place.
fn in_another_group(d: &Daemon, asked: &[String]) -> Option<Response> {
    let g = d.default_group()?;
    if asked.iter().any(|id| d.group(id).is_some()) {
        return None;
    }
    let name = g.name();
    let hint = if crate::leave::leaving(&g).is_some() {
        "leaving-wait"
    } else {
        "hint-in-another-group"
    };
    match problem("in-another-group", &[("group", text(&name))], None) {
        Response::Problem(mut p) => {
            p.hint = Some(msg(hint, &[]));
            Some(Response::Problem(p))
        }
        other => Some(other),
    }
}

/// The `invitation` message: the line that joins `g` with `code`.
fn invitation(g: &Group, code: &str) -> Response {
    said(
        "invitation",
        &[
            ("group", text(&g.name())),
            ("command", text(&pigeon_proto::join::command(code))),
        ],
    )
}

/// The problem of a failed join, with the arguments its messages take:
/// `inviter` is the person of the invitation's first admitter.
fn join_problem(code: &str, invitation: &str, person: Option<&str>) -> Response {
    let inviter = pigeon_proto::join::decode(invitation)
        .and_then(|i| i.admitters.into_iter().next())
        .map(|a| a.person)
        .unwrap_or_default();
    let who = [("inviter", text(&inviter))];
    match code {
        "inviter-offline" => match problem(code, &who, None) {
            Response::Problem(mut p) => {
                p.hint = Some(msg("hint-inviter-offline", &who));
                Response::Problem(p)
            }
            other => other,
        },
        "invite-not-allowed" => problem(code, &who, None),
        "person-exists" => {
            let person = person.map_or_else(crate::invite::account, |p| p.trim().to_owned());
            problem(code, &[("person", text(&person))], None)
        }
        "join-history-unverified"
        | "update-needed"
        | "invite-unknown"
        | "already-in-group"
        | "not-in-group"
        | "not-allowed"
        | "join-incomplete"
        | "repository-busy"
        | "folder-not-empty" => problem(code, &[], None),
        "not-in-this-build" => problem(code, &[("command", text("join"))], None),
        other => problem("internal-error", &[], Some(other.to_owned())),
    }
}

/// What `pigeon create` asks for.
struct New<'a> {
    /// The group's name.
    name: &'a str,
    /// Copies on distinct backup machines before a version is safe.
    k: u32,
    /// This computer's name in the roster.
    device_name: &'a str,
    /// The creator's name, if they gave one.
    person: Option<&'a str>,
}

async fn create(d: &Daemon, new: &New<'_>, folder: &Path) -> Response {
    // Refused before anything is written, so a corrected try starts clean.
    if new.name.trim().is_empty() || new.device_name.trim().is_empty() {
        return problem("name-empty", &[], None);
    }
    if new.k == 0 {
        return problem("copies-zero", &[], None);
    }
    if !folder.is_absolute() {
        let folder = folder.to_string_lossy();
        return problem("folder-not-absolute", &[("folder", text(&folder))], None);
    }
    match create_inner(d, new, folder).await {
        Ok(g) => {
            let code = crate::invite::code(d, &g).unwrap_or_default();
            said(
                "created",
                &[
                    ("group", text(new.name)),
                    ("command", text(&pigeon_proto::join::command(&code))),
                ],
            )
        }
        Err(e) => problem("internal-error", &[], Some(e)),
    }
}

async fn create_inner(d: &Daemon, new: &New<'_>, folder: &Path) -> Result<Arc<Group>, String> {
    let (name, k, device_name) = (new.name, new.k, new.device_name);
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
    let mut roster = Roster {
        rules: 1,
        group: name.to_owned(),
        invitation: crate::invite::new_secret().map_err(|e| e.to_string())?,
        k,
        retention_days: 30,
        telemetry_device: None,
        relay: None,
        persons: BTreeMap::new(),
        devices: BTreeMap::new(),
        removed: BTreeMap::new(),
        folders: Vec::new(),
    };
    // The creator is the person they named, else the account's, the
    // group's first admin; never a placeholder the others would see.
    let creator = new
        .person
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map_or_else(crate::invite::account, str::to_owned);
    let person = roster.fresh_person_id(&creator);
    roster.persons.insert(
        person.clone(),
        Person {
            name: creator,
            admin: true,
        },
    );
    roster.devices.insert(
        DeviceId(d.device.clone()),
        Device {
            person,
            name: device_name.to_owned(),
            role: DeviceRole::Laptop,
            annex_uuid: Some(uuid),
            maxsize: None,
        },
    );
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
    d.add_group(g, &folder).await
}

/// Starts a round shortly after the answer is on its way, so that a crash
/// in the round never takes the answer with it (13 §13.12, S9).
pub(crate) fn wake_soon(d: &Daemon) {
    let wake = d.wake.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        wake.notify_one();
    });
}

/// `pigeon free`: records the target; the loops hide each protected file
/// and drop its content, and the others once they are protected (08 §8.5).
/// While no backup machine has been heard from recently, only once
/// `confirmed` (`free_check.rs`).
async fn free(d: &Daemon, path: &Path, confirmed: bool) -> Vec<Response> {
    let Some(g) = group_of(d, path) else {
        return vec![problem("not-in-group", &[], None)];
    };
    let Some(prefix) = relative(&g, path) else {
        return vec![problem("not-in-group", &[], None)];
    };
    if !confirmed && let Some(ask) = crate::free_check::ask_again(d, &g) {
        return vec![ask];
    }
    if let Err(error) = crate::evict::set_target(&g, &prefix, true) {
        return vec![problem("internal-error", &[], Some(error))];
    }
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
    if let Err(error) = crate::evict::set_target(&g, &prefix, false) {
        return vec![problem("internal-error", &[], Some(error))];
    }
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
            recovery: x.role == DeviceRole::Recovery,
            heard_at: (x.role != DeviceRole::Recovery)
                .then(|| heard.get(&id.0).map(|t| crate::util::rfc3339(*t)))
                .flatten(),
            this_device: id.0 == d.device,
        })
        .collect();
    // A removed computer's list is the group as it last knew it (09 §9.7).
    crate::removed::noted(
        &g,
        Response::Machines {
            machines,
            copies: Some(doc.roster.k),
        },
    )
}

/// `pigeon people`: each person of the roster, with their machines.
async fn people(d: &Daemon) -> Vec<Response> {
    let Some(g) = d.default_group() else {
        return vec![problem("not-in-group", &[], None)];
    };
    let Some(doc) = g.roster() else {
        return vec![problem("not-in-group", &[], None)];
    };
    let r = &doc.roster;
    let refresh = crate::recovery_advice::refresh(&g, r).await;
    let people = r
        .persons
        .iter()
        .map(|(id, p)| pigeon_proto::api::PersonEntry {
            name: p.name.clone(),
            admin: p.admin,
            this_person: r
                .devices
                .get(&DeviceId(d.device.clone()))
                .is_some_and(|x| x.person == *id),
            recovery_refresh_recommended: p.admin && refresh.contains(id),
            recovery_recommended: p.admin
                && r.devices
                    .values()
                    .filter(|x| x.person == *id && x.role != DeviceRole::Recovery)
                    .count()
                    == 1
                && !r
                    .devices
                    .values()
                    .any(|x| x.person == *id && x.role == DeviceRole::Recovery),
            machines: r
                .devices
                .values()
                .filter(|x| x.person == *id && x.role != DeviceRole::Recovery)
                .map(|x| x.name.clone())
                .collect(),
        })
        .collect();
    crate::removed::noted(&g, Response::People { people })
}

/// The message of a text, for the modules that answer with one.
pub fn message(id: &str, args: &[(&str, Arg)]) -> Message {
    msg(id, args)
}

/// Where a debug bundle is written.
pub fn bundle_dir(d: &Daemon) -> PathBuf {
    d.dirs.state.join("bundles")
}
