//! A computer removed from its group learns it (09 §9.7, 06 §6.5): its
//! peers refuse it as `not-in-roster` (09 §9.3), so it can neither send
//! nor receive, and its copies count for nothing.
//!
//! It is removed when the roster of its accepted `main` no longer lists
//! it, or when a peer refuses it as `not-in-roster` although the roster
//! last fetched from that peer listed it: the peer knew it, so a signed
//! change removed it since. A peer whose roster never learned of this
//! computer's join refuses it too, and proves nothing (as for `leave`).
//! A computer that is leaving, or completing an in-place join, is not
//! removed. Each round notes it after its fetches; status then says
//! *Needs you* and nothing counts as safe here.

use pigeon_core::roster::{DeviceId, Role as DeviceRole, Roster};
use pigeon_proto::Response;
use pigeon_proto::stream::Refusal;

use crate::daemon::Daemon;
use crate::group::Group;
use crate::status::msg;

/// The devices of `roster` other than `me` that serve git (not recovery
/// sheets).
pub(crate) fn peers(roster: &Roster, me: &DeviceId) -> Vec<String> {
    roster
        .devices
        .iter()
        .filter(|(id, x)| *id != me && x.role != DeviceRole::Recovery)
        .map(|(id, _)| id.0.clone())
        .collect()
}

/// Whether `peer` refuses `me` as not in the group although the roster of
/// its `main`, as last fetched, lists `me`: it knew this computer, and no
/// longer does.
pub(crate) async fn refuses_after_knowing(
    d: &Daemon,
    g: &Group,
    peer: &str,
    me: &DeviceId,
) -> bool {
    if d.node.refusal(peer, &g.id) != Some(Refusal::NotInRoster) {
        return false;
    }
    let Some(bytes) = g
        .git
        .blob(&format!(
            "refs/pigeon/peers/{peer}/main:.pigeon/roster.toml"
        ))
        .await
    else {
        return false;
    };
    crate::roster_file::parse(&String::from_utf8_lossy(&bytes))
        .is_ok_and(|doc| doc.roster.devices.contains_key(me))
}

/// Whether the group `g` removed this computer `me`.
async fn removed(d: &Daemon, g: &Group, me: &DeviceId) -> bool {
    if crate::leave::leaving(g).is_some() || crate::join_transition::pending(g) {
        return false;
    }
    let Some(doc) = g.roster() else {
        return false;
    };
    if !doc.roster.devices.contains_key(me) {
        return true;
    }
    for peer in peers(&doc.roster, me) {
        if refuses_after_knowing(d, g, &peer, me).await {
            return true;
        }
    }
    false
}

/// Notes whether `g` removed this computer, once a round fetched.
pub async fn note(d: &Daemon, g: &Group) {
    let now = removed(d, g, &DeviceId(d.device.clone())).await;
    let mut k = g.lock();
    if k.removed != now {
        if now {
            tracing::warn!(group = %g.id, "removed-from-group");
        } else {
            tracing::info!(group = %g.id, "back-in-group");
        }
        k.removed = now;
    }
}

/// Whether `g` removed this computer, as the last round noted.
pub fn is_removed(g: &Group) -> bool {
    g.lock().removed
}

/// What a list of the group's people or machines says with it on a
/// removed computer: that the list is no longer the group's.
pub fn noted(g: &Group, list: Response) -> Vec<Response> {
    let note = is_removed(g).then(|| msg("removed-from-group", &[]));
    crate::status_why::noted(note, list)
}
