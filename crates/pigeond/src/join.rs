//! A new device's join (09 §9.6, steps 1 and 5): it makes its repository,
//! in an empty folder on a laptop and bare on a backup machine, sends
//! `Join` to an admitter, and once welcomed fetches `main` from it and
//! checks its history and that the welcome commit adds it. A laptop then
//! takes `main`'s tree with every annexed file hidden, so that keep brings
//! the files back (08 §8.5) and nothing reads as deleted. The first build
//! keeps every folder: choosing them at the join is not in it (13 §13.12).
//!
//! The device is in the group once that check passed, and only then:
//! `config.toml` records the group and its loops start last. Until then
//! `<git_dir>/pigeon/joining` names the group, and a join that failed or
//! was cut short resumes in that repository with its annex UUID, which the
//! admitter's roster may already hold (`Admission::Again`). A group that
//! `config.toml` lists without `main`, as earlier releases left a failed
//! join, becomes such a join again when the daemon starts ([`recover`]).
//! Ordinary failures clean up what the attempt made ([`crate::join_attempt`]).

use std::path::{Path, PathBuf};

use pigeon_core::filter::Role;
use pigeon_git::GitEnv;

use crate::config::{self, GroupEntry};
use crate::daemon::Daemon;
use crate::group::Group;
use crate::invite::{Joining, Welcomed, admitter_addresses, dial, fetch_welcome};
use crate::join_attempt::{Attempt, remember_uuid, reuse_uuid, serve_verified};
use crate::settings::{ensure_remote, init_annex};

/// Joins the group of `j` in `folder`, by default the group's name in the
/// home directory on a laptop and `<config>/<group>.git` on a backup
/// machine: answers the group's name.
pub async fn join(d: &Daemon, j: &Joining, folder: Option<String>) -> Result<String, String> {
    let inv = &j.inv;
    let path = folder.map_or_else(|| default_path(d, j), PathBuf::from);
    crate::filesystem::check(&path, role(j))?;
    let attempt = Attempt::begin(&path)?;
    crate::filesystem::check(&attempt.path, role(j))?;
    if crate::join_in_place::applies(d, j, &attempt.path).await {
        return crate::join_in_place::join(d, j, &attempt.path).await;
    }
    if unfinished(&attempt.path) != Some((role(j), inv.group.clone())) && !attempt.empty()? {
        return Err("folder-not-empty".into());
    }
    attempt.run(join_in(d, j, &attempt.path)).await?;
    tracing::info!(group = %inv.group, "joined-group");
    d.wake.notify_one();
    Ok(inv.name.clone())
}

/// Completes a new or interrupted attempt. A crash or ordinary failure
/// retains the repository; the same group can resume its joining marker.
async fn join_in(d: &Daemon, j: &Joining, path: &Path) -> Result<(), String> {
    let g = repository(d, j, path).await?;
    let git_dir = g.git_dir.clone();
    welcome(d, j, g, path).await?;
    let _ = std::fs::remove_file(marker(&git_dir));
    Ok(())
}

fn default_path(d: &Daemon, j: &Joining) -> PathBuf {
    match role(j) {
        Role::Laptop => std::env::var_os("HOME")
            .map_or_else(|| PathBuf::from("."), PathBuf::from)
            .join(&j.inv.name),
        Role::Custodian => d.dirs.config.join(format!("{}.git", j.inv.group)),
    }
}

/// How the repository of `j` holds the group.
fn role(j: &Joining) -> Role {
    match j.role {
        pigeon_core::roster::Role::Custodian => Role::Custodian,
        pigeon_core::roster::Role::Laptop | pigeon_core::roster::Role::Recovery => Role::Laptop,
    }
}

fn marker(git_dir: &Path) -> PathBuf {
    git_dir.join("pigeon").join("joining")
}

/// The group whose join the repository at `path` has not finished, with
/// how that repository holds it.
fn unfinished(path: &Path) -> Option<(Role, String)> {
    let (role, git_dir, _) = crate::group::locate(path)?;
    let id = std::fs::read_to_string(marker(&git_dir)).ok()?;
    Some((role, id.trim().to_owned()))
}

fn mark(git_dir: &Path, group: &str) -> Result<(), String> {
    std::fs::create_dir_all(git_dir.join("pigeon")).map_err(|e| e.to_string())?;
    pigeon_sys::fs::write_atomic(&marker(git_dir), format!("{group}\n").as_bytes())
        .map_err(|e| e.to_string())
}

/// The repository of the join of `j` at `path`: the one an unfinished
/// join of the same group left there, else a new one in the empty folder.
/// Its annex is set up once; the token is written last.
async fn repository(d: &Daemon, j: &Joining, path: &Path) -> Result<Group, String> {
    let inv = &j.inv;
    let git = crate::git::Git::new(path, d.env.clone());
    let fail = |e: crate::git::Error| e.to_string();
    if unfinished(path) != Some((role(j), inv.group.clone())) {
        if std::fs::read_dir(path)
            .map_err(|e| e.to_string())?
            .next()
            .is_some()
        {
            return Err("folder-not-empty".into());
        }
        let init: &[&str] = match role(j) {
            Role::Laptop => &["init", "-q", "-b", "main"],
            Role::Custodian => &["init", "-q", "--bare", "-b", "main"],
        };
        git.run(init).await.map_err(fail)?;
        let (_, git_dir, _) = crate::group::locate(path).ok_or("not-a-repository")?;
        mark(&git_dir, &inv.group)?;
    }
    let g = Group::open(&inv.group, path, &d.env)
        .await
        .ok_or("not-a-repository")?;
    if g.lock().token.is_some() {
        let uuid = g.lock().annex_uuid.clone().ok_or("not-a-repository")?;
        remember_uuid(d, &inv.group, &uuid);
        return Ok(g);
    }
    git.run(&["config", "user.name", &j.name])
        .await
        .map_err(fail)?;
    let email = format!("{}@pigeon", d.device);
    git.run(&["config", "user.email", &email])
        .await
        .map_err(fail)?;
    crate::settings::check(&g).await;
    reuse_uuid(d, &inv.group, &git).await?;
    let (uuid, token) = init_annex(&git, role(j), &j.name, inv.k)
        .await
        .map_err(fail)?;
    remember_uuid(d, &inv.group, &uuid);
    if role(j) == Role::Laptop {
        let k = inv.k.to_string();
        git.run(&["annex", "numcopies", &k]).await.map_err(fail)?;
        git.run(&["annex", "mincopies", &k]).await.map_err(fail)?;
    }
    g.lock().annex_uuid = Some(uuid);
    g.set_token(&token);
    Ok(g)
}

/// Asks for the welcome of `j` and takes it into `g`'s repository. `g` is
/// among the daemon's groups only meanwhile, so that the helper finds its
/// repository, busy, so that no round reads it before every annexed file
/// is hidden, and open to its admitter alone, as a staged join (09 §9.9).
async fn welcome(d: &Daemon, j: &Joining, g: Group, path: &Path) -> Result<(), String> {
    let uuid = g.lock().annex_uuid.clone().ok_or("not-a-repository")?;
    let w = dial(d, j, &uuid).await?;
    pigeon_sys::lab::crash_point("join.after-welcome");
    let admitter = &w.admitter.device;
    ensure_remote(&g.git, admitter, &w.annex_uuid)
        .await
        .map_err(|e| e.to_string())?;
    g.set_addresses(admitter, admitter_addresses(&w.admitter));
    g.learn_peers(&w.peers);
    g.lock().bootstrap_peer = Some((admitter.clone(), w.annex_uuid.clone()));
    serve_verified(d, g, path, |g| async move {
        take(d, j, &g, &w).await?;
        pigeon_sys::lab::crash_point("join.before-config");
        Ok(())
    })
    .await?;
    Ok(())
}

/// Fetches and checks the welcome `w` of `j`, then makes its `main`
/// `g`'s, with a laptop's annexed files hidden.
async fn take(d: &Daemon, j: &Joining, g: &Group, w: &Welcomed) -> Result<(), String> {
    let theirs = fetch_welcome(g, &w.admitter.device, &j.inv.group, &w.commit, &d.device).await?;
    // A join cut short after this step left the welcome it had then.
    let ours = g.git.rev("refs/heads/main").await;
    g.git
        .update_ref("refs/heads/main", &theirs, ours.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    g.reload_roster().await;
    if g.worktree.is_some() {
        crate::trace_roster::moved(&d.tracer, g, "Merge", None).await;
        crate::recovery::hide_all(g, &theirs).await?;
    }
    tracing::info!(group = %g.id, commit = %w.commit, "welcome-taken");
    Ok(())
}

/// Makes a group that `config.toml` lists without `main` a join again:
/// earlier releases recorded a join before checking its welcome, and kept
/// it when the check failed, so that neither joining nor leaving it could
/// go on. The group leaves the file once its repository is marked. One
/// whose `main` git cannot tell stays as it is.
pub(crate) async fn recover(entry: &GroupEntry, configuration: &Path, env: &GitEnv) {
    let Some((_, git_dir, worktree)) = crate::group::locate(&entry.path) else {
        return;
    };
    let git = crate::git::Git::new(worktree.as_deref().unwrap_or(&git_dir), env.clone());
    let unmade = match git
        .run(&["for-each-ref", "--format=%(objectname)", "refs/heads/main"])
        .await
    {
        Ok(main) if main.trim().is_empty() => mark(&git_dir, &entry.id).and_then(|()| {
            config::remove_group(configuration, &entry.id).map_err(|e| e.to_string())
        }),
        Ok(_) => return,
        Err(e) => Err(e.to_string()),
    };
    match unmade {
        Ok(()) => {
            tracing::warn!(group = %entry.id, path = %entry.path.display(), "join-unfinished")
        }
        Err(error) => tracing::warn!(group = %entry.id, %error, "join-unfinished-kept"),
    }
}
