//! `status.toml` (06 §6.3): the report a device publishes of its state, as
//! text. Times are RFC 3339 in UTC, sizes are bytes.
//!
//! The schema is closed (ADR 0048): a report of another `format`, or with a
//! field this release does not know, is [`Read::Unknown`] and does not
//! count; one that is not TOML or names no format is [`Read::Malformed`].
//!
//! Law (tests below): parsing a rendered report gives it back.

use serde::{Deserialize, Serialize};
use toml_edit::Datetime;

/// The format this release writes and reads.
pub const FORMAT: u32 = 1;

/// A status report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    /// Its format.
    pub format: u32,
    /// The device it is of, which signs it.
    pub device: String,
    /// When it was true.
    pub at: Datetime,
    /// The commit of its `main`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main: Option<String>,
    /// Tips of the proposal chains it keeps outside `main` (ADR 0046).
    #[serde(default)]
    pub proposals: Vec<String>,
    /// Custodians: while a restored repository is re-checked (ADR 0046).
    #[serde(default)]
    pub checking: bool,
    /// Whether a lab build wrote it (13 §13.12): a release build never
    /// counts it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lab: bool,
    /// The versions it runs.
    pub versions: Versions,
    /// Its disk.
    pub disk: Disk,
    /// Custodians: its repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custody: Option<Custody>,
    /// Custodians: the last scrub.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scrub: Option<Scrub>,
    /// Custodians: purge seals (08 §8.6).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub purge: Vec<Purge>,
    /// Its queues.
    pub queue: Queue,
}

/// The versions a device runs.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Versions {
    /// pigeon's.
    pub pigeon: String,
    /// git-annex's.
    pub git_annex: String,
    /// git's.
    pub git: String,
}

/// A device's disk, in bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Disk {
    /// What its annex objects take.
    pub used: u64,
    /// What an unprivileged writer may still use.
    pub free: u64,
    /// Its roster `maxsize`, 0 for none.
    pub maxsize: u64,
    /// Its `annex.diskreserve` (08 §8.9).
    pub reserve: u64,
    /// Whether writes are paused: less free than the reserve (08 §8.9).
    pub full: bool,
}

/// A custodian's repository.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Custody {
    /// `"ok"`, or `"lost"`: its repository, UUID or storage is gone, and
    /// it counts for nothing.
    pub repository: String,
}

/// A custodian's last scrub.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scrub {
    /// When its last pass completed.
    pub last_pass_completed: Datetime,
    /// The keys it found bad.
    pub keys_failed: u64,
}

/// A custodian's purge seal (08 §8.6).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Purge {
    /// The decision.
    pub decision: String,
    /// Its `main` when it sealed.
    pub main: String,
    /// Covered keys it keeps for a reference outside `main`.
    #[serde(default)]
    pub held: Vec<String>,
}

/// A device's queues.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Queue {
    /// Bytes it still sends.
    pub bytes_to_send: u64,
    /// Bytes it still receives.
    pub bytes_to_receive: u64,
    /// `""`, `"person"`, `"battery"`, `"metered"` or `"disk-busy"` (ADR 0037).
    #[serde(default)]
    pub paused: String,
    /// Custodians: `"all"`, or `"endangered"` while they keep room for
    /// repair (08 §8.9).
    pub admits: String,
    /// Custodians: the keys admission deferred (08 §8.9).
    #[serde(default)]
    pub deferred: u64,
    /// Custodians: the free space they miss, in bytes.
    #[serde(default)]
    pub missing: u64,
}

/// What reading a report found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Read {
    /// A report this release reads.
    Known(Box<Status>),
    /// Another format, or a field this release does not know.
    Unknown,
    /// Not TOML, or no format.
    Malformed,
}

impl Status {
    /// Its `at`, in Unix seconds, if it is a UTC time.
    pub fn at_unix(&self) -> Option<i64> {
        crate::util::parse_rfc3339(&self.at.to_string())
    }
}

/// The TOML time of Unix second `t`.
pub fn datetime(t: i64) -> Datetime {
    crate::util::rfc3339(t).parse().unwrap_or(Datetime {
        date: None,
        time: None,
        offset: None,
    })
}

/// Parses a report's text.
pub fn parse(text: &str) -> Read {
    let Ok(doc) = text.parse::<toml_edit::DocumentMut>() else {
        return Read::Malformed;
    };
    match doc.get("format").and_then(toml_edit::Item::as_integer) {
        Some(f) if f == i64::from(FORMAT) => {}
        Some(_) => return Read::Unknown,
        None => return Read::Malformed,
    }
    match toml_edit::de::from_str::<Status>(text) {
        Ok(s) if s.at_unix().is_some() => Read::Known(Box::new(s)),
        Ok(_) => Read::Malformed,
        Err(_) => Read::Unknown,
    }
}

/// Renders a report.
pub fn render(s: &Status) -> Result<String, toml_edit::ser::Error> {
    toml_edit::ser::to_string_pretty(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Status {
        Status {
            format: FORMAT,
            device: "b71e".into(),
            at: datetime(1_791_892_920),
            main: Some("c0ffee".into()),
            proposals: vec![],
            checking: false,
            lab: false,
            versions: Versions {
                pigeon: "0.1.0".into(),
                git_annex: "10.20261006".into(),
                git: "2.47.0".into(),
            },
            disk: Disk {
                used: 812,
                free: 1100,
                maxsize: 0,
                reserve: 22,
                full: false,
            },
            custody: Some(Custody {
                repository: "ok".into(),
            }),
            scrub: None,
            purge: vec![],
            queue: Queue {
                admits: "all".into(),
                ..Queue::default()
            },
        }
    }

    #[test]
    fn parsing_a_rendered_report_gives_it_back() {
        let s = sample();
        let text = render(&s).unwrap();
        assert_eq!(parse(&text), Read::Known(Box::new(s.clone())));
        assert_eq!(s.at_unix(), Some(1_791_892_920));
        assert!(text.contains("at = 2026-"), "{text}");
    }

    #[test]
    fn another_format_or_an_unknown_field_does_not_count() {
        let text = render(&sample()).unwrap();
        let newer = text.replace("format = 1", "format = 2");
        assert_eq!(parse(&newer), Read::Unknown);
        let extra = text.replace("format = 1", "format = 1\nmood = \"happy\"");
        assert_eq!(parse(&extra), Read::Unknown);
        let deep = text.replace("full = false", "full = false\nsmart = true");
        assert_eq!(parse(&deep), Read::Unknown);
        assert_eq!(parse("not = [toml"), Read::Malformed);
        assert_eq!(parse("device = \"x\""), Read::Malformed);
        let local = text.replace("Z\n", "\n");
        assert_eq!(parse(&local), Read::Malformed);
    }
}
