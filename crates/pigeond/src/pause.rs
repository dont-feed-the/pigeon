//! `pigeon pause [DURATION]` and `pigeon resume` (15 §15.6, ux.md §6.5):
//! `paused_until` of `[resources]` in `config.toml`, a time or `"resume"`
//! for a pause without end, so it survives a restart. While it holds, a
//! round only captures, and the status says `paused(person)` (ADR 0027).

use std::path::Path;

use pigeon_proto::{Arg, Response};

use crate::api::{problem, said};
use crate::config;
use crate::daemon::Daemon;

/// The key in `[resources]`.
const KEY: &str = "paused_until";

/// The value of a pause without end.
const UNTIL_RESUME: &str = "resume";

/// A pause in force.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paused {
    /// Until this time, in seconds since the epoch.
    Until(i64),
    /// Until `pigeon resume`.
    UntilResume,
}

/// The seconds of a duration such as `45m`, `2h` or `3d`; a bare number
/// is minutes. At most a year.
pub fn duration(text: &str) -> Option<i64> {
    let text = text.trim();
    let (number, unit) = match text.char_indices().last()? {
        (i, c) if c.is_ascii_alphabetic() => (&text[..i], c),
        _ => (text, 'm'),
    };
    let n: i64 = number.trim().parse().ok().filter(|n| *n > 0)?;
    let seconds = n.checked_mul(match unit {
        's' => 1,
        'm' => 60,
        'h' => 3600,
        'd' => 86_400,
        _ => return None,
    })?;
    (seconds <= 366 * 86_400).then_some(seconds)
}

/// The pause the file at `path` holds at `now`, if any.
pub fn paused(path: &Path, now: i64) -> Option<Paused> {
    let doc = config::read_doc(path).ok()?;
    let value = doc.get("resources")?.get(KEY)?;
    if value.as_str() == Some(UNTIL_RESUME) {
        return Some(Paused::UntilResume);
    }
    let text = match value.as_datetime() {
        Some(t) => t.to_string(),
        None => value.as_str()?.to_owned(),
    };
    let until = pigeon_proto::time::parse_rfc3339(&text)?;
    (until > now).then_some(Paused::Until(until))
}

/// The pause of this computer now.
pub fn now(d: &Daemon) -> Option<Paused> {
    paused(&d.dirs.config_file(), crate::util::now())
}

/// `Pause {duration}`.
pub fn pause(d: &Daemon, duration_text: Option<&str>) -> Vec<Response> {
    let value = match duration_text {
        None => toml_edit::Value::from(UNTIL_RESUME),
        Some(t) => match duration(t) {
            Some(s) => {
                let until = pigeon_proto::time::rfc3339(crate::util::now() + s);
                match until.parse::<toml_edit::Datetime>() {
                    Ok(dt) => toml_edit::Value::from(dt),
                    Err(e) => return vec![problem("internal-error", &[], Some(e.to_string()))],
                }
            }
            None => {
                return vec![problem(
                    "pause-duration",
                    &[("duration", Arg::Text(t.to_owned()))],
                    None,
                )];
            }
        },
    };
    if let Err(e) = config::set_resource(&d.dirs.config_file(), KEY, Some(value)) {
        return vec![problem("internal-error", &[], Some(e.to_string()))];
    }
    d.node.resources.set_person_paused(now(d).is_some());
    tracing::info!(until = ?now(d), "paused");
    vec![answer(now(d))]
}

/// `Resume`.
pub fn resume(d: &Daemon) -> Vec<Response> {
    if let Err(e) = config::set_resource(&d.dirs.config_file(), KEY, None) {
        return vec![problem("internal-error", &[], Some(e.to_string()))];
    }
    d.node.resources.set_person_paused(false);
    tracing::info!("resumed");
    crate::api::wake_soon(d);
    vec![said("resumed", &[])]
}

/// What a pause is told as.
fn answer(p: Option<Paused>) -> Response {
    match p {
        Some(Paused::Until(t)) => said(
            "paused-until",
            &[("until", Arg::Text(pigeon_proto::time::rfc3339(t)))],
        ),
        Some(Paused::UntilResume) => said("paused-until-resume", &[]),
        None => said("resumed", &[]),
    }
}

/// Says the pause in force in `report`: always as its `paused` line, and
/// as its headline unless something needs the person (15 §15.6).
pub fn tell(d: &Daemon, report: &mut pigeon_proto::api::StatusReport) {
    let Some(p) = now(d) else {
        return;
    };
    report.paused = Some(headline(p));
    if report.level != pigeon_proto::api::Level::NeedsYou {
        report.headline = headline(p);
    }
}

/// `Paused`: the pause in force, for the banner of every web page; `Done`
/// while sharing.
pub fn state(d: &Daemon) -> Vec<Response> {
    vec![match now(d) {
        Some(p) => Response::Said {
            message: headline(p),
        },
        None => Response::Done,
    }]
}

/// The headline of a pause.
pub fn headline(p: Paused) -> pigeon_proto::Message {
    match p {
        Paused::Until(t) => crate::api::message(
            "headline-paused",
            &[("until", Arg::Text(pigeon_proto::time::rfc3339(t)))],
        ),
        Paused::UntilResume => crate::api::message("headline-paused-until-resume", &[]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(duration("45m"), Some(2700));
        assert_eq!(duration("2h"), Some(7200));
        assert_eq!(duration("3d"), Some(259_200));
        assert_eq!(duration("10"), Some(600));
        assert_eq!(duration("0h"), None);
        assert_eq!(duration("2w"), None);
        assert_eq!(duration("400d"), None);
        assert_eq!(duration(""), None);
    }

    #[test]
    fn a_pause_is_read_back_and_ends() {
        let dir = std::env::temp_dir().join(format!("pigeon-pause-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "format = 1\nlanguage = \"fr\"\n").unwrap();
        assert_eq!(paused(&path, 0), None);
        let dt: toml_edit::Datetime = "2026-10-13T18:00:00Z".parse().unwrap();
        config::set_resource(&path, KEY, Some(dt.into())).unwrap();
        let until = pigeon_proto::time::parse_rfc3339("2026-10-13T18:00:00Z").unwrap();
        assert_eq!(paused(&path, until - 1), Some(Paused::Until(until)));
        assert_eq!(paused(&path, until), None);
        config::set_resource(&path, KEY, Some(UNTIL_RESUME.into())).unwrap();
        assert_eq!(paused(&path, until), Some(Paused::UntilResume));
        config::set_resource(&path, KEY, None).unwrap();
        assert_eq!(paused(&path, 0), None);
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("language = \"fr\"")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
