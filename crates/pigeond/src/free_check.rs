//! Before `pigeon free` (08 §8.5, ux.md §8.5): what freeing a folder does,
//! for the confirmation of *Free up space* (`FreePreview`), and whether
//! the backup machines were heard from recently enough to confirm their
//! copies now. The window is the one of *Not checked recently*
//! (`stale_after`, 06 §6.7); while no backup machine was heard within it,
//! `free` answers `backups-quiet` and frees only once asked again.

use std::path::Path;

use pigeon_core::roster::Role as DeviceRole;
use pigeon_core::state::State;
use pigeon_core::time::{Span, Time};
use pigeon_proto::api::{Arg, FreePreview};
use pigeon_proto::{Problem, Response};

use crate::api::{group_of, problem, relative};
use crate::daemon::Daemon;
use crate::group::Group;
use crate::status::msg;

/// The names of `g`'s backup machines other than `me`, and when the last
/// heard from of them was, in seconds since the epoch.
fn backups(g: &Group, me: &str) -> (Vec<String>, Option<i64>) {
    let Some(doc) = g.roster() else {
        return (Vec::new(), None);
    };
    let heard = g.lock().heard.clone();
    let mut names = Vec::new();
    let mut last = None;
    for (id, device) in &doc.roster.devices {
        if device.role == DeviceRole::Custodian && id.0 != me {
            names.push(device.name.clone());
            last = last.max(heard.get(&id.0).copied());
        }
    }
    (names, last)
}

/// Whether none of the backup machines `names` was heard from within the
/// window of `g` at `now`, the last at `last`.
fn quiet(g: &Group, names: &[String], last: Option<i64>, now: i64) -> bool {
    quiet_for(
        names.len(),
        last,
        crate::status::settings(g).stale_after,
        now,
    )
}

/// Whether `backups` backup machines, the last heard from at `last`, are
/// quiet at `now` for a check `window`. A group without one has no copies
/// to confirm: nothing is freed there before one joins.
fn quiet_for(backups: usize, last: Option<i64>, window: Span, now: i64) -> bool {
    backups > 0 && !last.is_some_and(|t| Time(t).within(window, Time(now)))
}

fn count(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// `FreePreview {folder}`: the files still on this computer under it,
/// those safe on the backup machines, which would leave it, the backup
/// machines, and whether they are quiet.
pub async fn preview(d: &Daemon, path: &Path) -> Vec<Response> {
    let Some(g) = group_of(d, path) else {
        return vec![problem("not-in-group", &[], None)];
    };
    let Some(prefix) = relative(&g, path) else {
        return vec![problem("not-in-group", &[], None)];
    };
    let hidden = crate::evict::hidden_paths(&g).await;
    let here: Vec<_> = crate::status::observe(&g, &d.device, &prefix)
        .await
        .into_iter()
        .filter(|o| !hidden.contains(&o.path))
        .collect();
    let safe = here
        .iter()
        .filter(|o| matches!(o.state, State::Protected(_) | State::Uncertain(_)))
        .count();
    let (backups, last) = backups(&g, &d.device);
    let quiet = quiet(&g, &backups, last, crate::util::now());
    vec![Response::FreePreview(FreePreview {
        files: count(here.len()),
        safe: count(safe),
        backups,
        heard_at: last.map(crate::util::rfc3339),
        quiet,
    })]
}

/// The problem `backups-quiet`, when freeing `g` must be asked again: no
/// backup machine was heard from recently.
pub fn ask_again(d: &Daemon, g: &Group) -> Option<Response> {
    let (names, last) = backups(g, &d.device);
    if !quiet(g, &names, last, crate::util::now()) {
        return None;
    }
    let machines = Arg::Text(names.join(", "));
    let message = match last {
        Some(t) => msg(
            "problem-backups-quiet",
            &[
                ("machines", machines),
                ("heard", Arg::Text(crate::util::rfc3339(t))),
            ],
        ),
        None => msg("free-backups-never", &[("machines", machines)]),
    };
    Some(Response::Problem(Problem {
        code: "backups-quiet".to_owned(),
        message,
        hint: Some(msg("hint-backups-quiet", &[])),
        details: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backups_are_quiet_after_the_hour_that_makes_a_check_recent() {
        let hour = pigeon_core::state::Settings::default().stale_after;
        let now = 1_792_108_800;
        assert!(!quiet_for(2, Some(now - 60), hour, now));
        assert!(!quiet_for(1, Some(now - 3600), hour, now));
        assert!(quiet_for(1, Some(now - 3601), hour, now));
        assert!(quiet_for(2, None, hour, now), "never heard from");
        assert!(!quiet_for(0, None, hour, now), "no backup machine yet");
    }
}
