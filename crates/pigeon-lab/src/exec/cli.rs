//! The `pigeon` command line as the executor uses it: `--json` on every
//! command, the response on stdout, a problem on stderr (10 §10.2, §10.6).
//!
//! Every argument list the executor builds is here, so a change of the
//! command line is one edit. Responses are read as JSON values, through the
//! types of `pigeon-proto` where they fit, and leniently where the slice's
//! commands still grow (a missing field is a skipped check, not a crash).

use std::path::Path;
use std::process::Command;

use pigeon_core::headline::Level;
use serde_json::Value;

use crate::step::{Kind, Outcome, level_name};

/// What one `pigeon` command answered.
#[derive(Clone, Debug, Default)]
pub struct Answer {
    /// It exited 0.
    pub ok: bool,
    /// Its stdout, parsed: the last JSON value on it.
    pub json: Option<Value>,
    /// The problem's code, from stderr.
    pub problem: Option<String>,
    /// Its stdout.
    pub stdout: String,
    /// Its stderr.
    pub stderr: String,
}

fn last_json(text: &str) -> Option<Value> {
    if let Ok(v) = serde_json::from_str(text.trim()) {
        return Some(v);
    }
    text.lines()
        .rev()
        .find_map(|l| serde_json::from_str(l.trim()).ok())
}

/// Runs `cmd` (a `pigeon` invocation) and reads its answer.
pub fn answer(cmd: &mut Command) -> Result<Answer, String> {
    let out = cmd.output().map_err(|e| format!("pigeon: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let problem =
        last_json(&stderr).and_then(|v| v.get("code").and_then(Value::as_str).map(str::to_owned));
    Ok(Answer {
        ok: out.status.success(),
        json: last_json(&stdout),
        problem,
        stdout,
        stderr,
    })
}

/// The problem code a `pigeon` command printed on stderr.
pub fn answer_code(stderr: &str) -> Option<String> {
    last_json(stderr).and_then(|v| v.get("code").and_then(Value::as_str).map(str::to_owned))
}

impl Answer {
    /// The outcome it amounts to.
    pub fn outcome(&self) -> Outcome {
        match (&self.ok, &self.problem) {
            (true, _) => Outcome::Done,
            (false, Some(code)) => Outcome::Refused(code.clone()),
            (false, None) => {
                Outcome::Refused(format!("exit-without-problem: {}", self.stderr.trim()))
            }
        }
    }
}

fn s(x: impl AsRef<str>) -> String {
    x.as_ref().to_owned()
}

fn p(path: &Path) -> String {
    path.display().to_string()
}

/// `pigeon create GROUP DIR --k K --name DEVICE`.
pub fn create(group: &str, folder: &Path, k: u32, device: &str) -> Vec<String> {
    vec![
        s("create"),
        s(group),
        p(folder),
        s("--k"),
        k.to_string(),
        s("--name"),
        s(device),
        s("--json"),
    ]
}

/// `pigeon invite`: the line that joins the group.
pub fn invite() -> Vec<String> {
    vec![s("invite"), s("--json")]
}

/// The group's `pgn2-` invitation in an answer: inside the line that
/// joins, in its JSON or on its standard output.
pub fn invitation(json: Option<&Value>, stdout: &str) -> Option<String> {
    json.and_then(|v| find_containing(v, "pgn2-"))
        .or_else(|| stdout.contains("pgn2-").then(|| stdout.to_owned()))
        .map(|line| pigeon_proto::join::invitation(&line))
}

/// `pigeon join --backup-machine INVITATION --name NAME [--person PERSON]
/// --yes`: a backup machine of `person`, else of the account's person.
pub fn join_custodian(invitation: &str, name: &str, person: Option<&str>) -> Vec<String> {
    let mut a = vec![
        s("join"),
        s("--backup-machine"),
        s(invitation),
        s("--name"),
        s(name),
    ];
    if let Some(p) = person {
        a.extend([s("--person"), s(p)]);
    }
    a.extend([s("--yes"), s("--json")]);
    a
}

/// `pigeon join INVITATION FOLDER --name NAME --person PERSON`, with
/// `--yes` to join a person the group `has`.
pub fn join_laptop(
    invitation: &str,
    folder: &Path,
    name: &str,
    person: &str,
    has: bool,
) -> Vec<String> {
    let mut a = vec![
        s("join"),
        s(invitation),
        p(folder),
        s("--name"),
        s(name),
        s("--person"),
        s(person),
    ];
    if has {
        a.push(s("--yes"));
    }
    a.push(s("--json"));
    a
}

/// `pigeon resolve PATH --take-group`.
pub fn take_group(path: &Path) -> Vec<String> {
    vec![s("resolve"), p(path), s("--take-group"), s("--json")]
}

/// `pigeon accept ID`, `pigeon decline ID`.
pub fn decide(verb: &str, id: &str) -> Vec<String> {
    vec![s(verb), s(id), s("--json")]
}

/// `pigeon status [PATH] [--not-shared]`.
pub fn status(path: Option<&Path>, not_shared: bool) -> Vec<String> {
    let mut a = vec![s("status")];
    a.extend(path.map(p));
    if not_shared {
        a.push(s("--not-shared"));
    }
    a.push(s("--json"));
    a
}

/// `pigeon free PATH --yes`, `pigeon keep PATH`: a scenario frees whether
/// or not the backup machines were heard from recently (08 §8.5).
pub fn free_or_keep(verb: &str, path: &Path) -> Vec<String> {
    let mut a = vec![s(verb), p(path)];
    if verb == "free" {
        a.push(s("--yes"));
    }
    a.push(s("--json"));
    a
}

/// `pigeon restore PATH --version N`.
pub fn restore(path: &Path, version: u64) -> Vec<String> {
    vec![
        s("restore"),
        p(path),
        s("--version"),
        version.to_string(),
        s("--json"),
    ]
}

/// `pigeon remove-computer NAME --yes`.
pub fn remove_computer(name: &str) -> Vec<String> {
    vec![s("remove-computer"), s(name), s("--yes"), s("--json")]
}

/// `pigeon recovery-sheet`.
pub fn recovery_sheet() -> Vec<String> {
    vec![s("recovery-sheet"), s("--json")]
}

/// `pigeon recover SHEET FOLDER --yes`.
pub fn recover(sheet: &Path, folder: &Path) -> Vec<String> {
    vec![s("recover"), p(sheet), p(folder), s("--yes"), s("--json")]
}

/// `pigeon machines`, `pigeon doctor`.
pub fn plain(verb: &str) -> Vec<String> {
    vec![s(verb), s("--json")]
}

/// The first string in `v` that holds `part`.
pub fn find_containing(v: &Value, part: &str) -> Option<String> {
    match v {
        Value::String(t) if t.contains(part) => Some(t.clone()),
        Value::Array(xs) => xs.iter().find_map(|x| find_containing(x, part)),
        Value::Object(m) => m.values().find_map(|x| find_containing(x, part)),
        _ => None,
    }
}

/// Whether some object in `v` has `"code": code`.
pub fn has_code(v: &Value, code: &str) -> bool {
    match v {
        Value::Array(xs) => xs.iter().any(|x| has_code(x, code)),
        Value::Object(m) => {
            m.get("code").and_then(Value::as_str) == Some(code)
                || m.values().any(|x| has_code(x, code))
        }
        _ => false,
    }
}

fn kind_of(state: &str) -> Option<Kind> {
    Some(match state {
        "local" => Kind::Local,
        "published" => Kind::Published,
        "protected" => Kind::Protected,
        "uncertain" => Kind::Uncertain,
        "missing" | "erased" => Kind::Missing,
        _ => return None,
    })
}

/// The kind of the newest version of `path` in a status report for `path`.
/// A path not listed among the versions takes the report's own state, the
/// least safe of the versions under it; a report that lists no version at
/// all says nothing of it (its state, protected, holds of nothing).
pub fn version_kind(status: &Value, path: &str) -> Option<Kind> {
    let body = status.get("Status").unwrap_or(status);
    let versions = body.get("versions").and_then(Value::as_array)?;
    if versions.is_empty() {
        return None;
    }
    let newest = versions
        .iter()
        .filter(|v| {
            v.get("path")
                .and_then(Value::as_str)
                .is_some_and(|p| p == path || p.ends_with(&format!("/{path}")))
        })
        .max_by_key(|v| v.get("number").and_then(Value::as_u64).unwrap_or(0));
    match newest {
        Some(v) => v.get("state").and_then(Value::as_str).and_then(kind_of),
        None => body.get("state").and_then(Value::as_str).and_then(kind_of),
    }
}

/// The headline level of a status report: its `level`, else the level its
/// headline message names.
pub fn level(status: &Value) -> Option<Level> {
    let body = status.get("Status").unwrap_or(status);
    if let Some(l) = body.get("level").and_then(Value::as_str) {
        return level_name::parse(l);
    }
    let id = body.get("headline")?.get("id")?.as_str()?;
    ["needs-you", "not-safe", "not-checked", "working", "safe"]
        .into_iter()
        .find(|n| id.contains(n))
        .and_then(level_name::parse)
}

/// Whether a status report lists `path` as not shared.
pub fn not_shared(status: &Value, path: &str) -> bool {
    let body = status.get("Status").unwrap_or(status);
    body.get("not_shared")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|n| {
            n.get("path")
                .and_then(Value::as_str)
                .is_some_and(|p| p == path || p.ends_with(&format!("/{path}")))
        })
}

/// The `(ID, paths)` of each suggestion `pigeon suggestions` lists.
pub fn suggestions(v: &Value) -> Vec<(String, Vec<String>)> {
    let body = v.get("Proposals").unwrap_or(v);
    body.get("proposals")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let paths = p.get("paths")?.as_array()?;
            Some((
                p.get("id")?.as_str()?.to_owned(),
                paths
                    .iter()
                    .filter_map(|x| x.as_str().map(str::to_owned))
                    .collect(),
            ))
        })
        .collect()
}

/// The `(name, device ID)` of each machine `pigeon machines` lists.
pub fn machines(v: &Value) -> Vec<(String, String)> {
    let body = v.get("Machines").unwrap_or(v);
    body.get("machines")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|m| {
            Some((
                m.get("name")?.as_str()?.to_owned(),
                m.get("device")?.as_str()?.to_owned(),
            ))
        })
        .collect()
}
