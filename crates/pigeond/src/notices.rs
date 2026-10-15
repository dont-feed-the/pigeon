//! Desktop notifications (10 §10.4, ADR 0029): after each round of the
//! loops, what this person must hear about becomes a
//! [`pigeon_core::notify::Situation`], and `pigeon_core::notify::step`
//! decides what to show, from what was shown before (kept in
//! `<state>/notified`, so a restart does not repeat it).
//!
//! Shown through `notify-rust`, in the person's language; a click opens
//! the web interface (Linux). Off with `notifications = false`, on backup
//! machines and without a desktop. The lab sets `PIGEON_NOTIFY_LOG`: each
//! notification is then a line of that file instead.

use std::path::{Path, PathBuf};

use pigeon_core::notify::{Kind, Last, Memory, Situation, step};
use pigeon_core::time::Time;
use pigeon_proto::Response;
use pigeon_proto::api::{Level, State};

use crate::daemon::Daemon;

/// FNV-1a, a fingerprint that stays the same across builds.
fn fingerprint<'a>(items: impl Iterator<Item = &'a str>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for item in items {
        for b in item.bytes().chain(std::iter::once(0)) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// The situation of this person in the default group, at `now`.
async fn situation(d: &Daemon, now: i64) -> Option<Situation> {
    let g = d.default_group()?;
    if g.role != pigeon_core::filter::Role::Laptop {
        return None;
    }
    let report = crate::status::report(&g, &d.device, "", false).await;
    let needs_you = (report.level == Level::NeedsYou).then(|| {
        let args = report
            .headline
            .args
            .iter()
            .map(|(k, v)| format!("{k}={v:?}"));
        let all: Vec<String> = std::iter::once(report.headline.id.clone())
            .chain(args)
            .collect();
        fingerprint(all.iter().map(String::as_str))
    });
    let day_ago = now - pigeon_core::notify::RATE.0;
    let old: Vec<String> = report
        .versions
        .iter()
        .filter(|v| matches!(v.state, State::Local | State::Published))
        .filter(|v| pigeon_proto::time::parse_rfc3339(&v.saved_at).is_some_and(|t| t < day_ago))
        .map(|v| format!("{}@{}", v.path, v.saved_at))
        .collect();
    let not_safe_long = (!old.is_empty()).then(|| fingerprint(old.iter().map(String::as_str)));
    let mut waiting: Vec<String> = Vec::new();
    for r in crate::decide::list(d).await {
        if let Response::Proposals { proposals } = r {
            waiting.extend(
                proposals
                    .into_iter()
                    .filter(|p| p.decide && !p.mine)
                    .map(|p| p.id),
            );
        }
    }
    for r in crate::resolve::list(d).await {
        if let Response::Conflicts { conflicts } = r {
            waiting.extend(
                conflicts
                    .into_iter()
                    .filter(|c| c.state == "conflict")
                    .map(|c| c.path),
            );
        }
    }
    waiting.sort();
    let waiting = (!waiting.is_empty()).then(|| fingerprint(waiting.iter().map(String::as_str)));
    Some(Situation {
        needs_you,
        not_safe_long,
        waiting,
    })
}

fn memory_file(d: &Daemon) -> PathBuf {
    d.dirs.state.join("notified")
}

/// `<state>/notified`: one line per kind, `AT FINGERPRINT`, `-` for none.
fn load(path: &Path) -> Memory {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut m = Memory::default();
    for (slot, line) in m.0.iter_mut().zip(text.lines()) {
        let mut w = line.split_whitespace();
        let at = w.next().and_then(|t| t.parse().ok()).map(Time);
        let fingerprint = w.next().and_then(|f| f.parse().ok());
        *slot = Last { at, fingerprint };
    }
    m
}

fn save(path: &Path, m: &Memory) {
    let show = |x: Option<String>| x.unwrap_or_else(|| "-".to_owned());
    let text: String =
        m.0.iter()
            .map(|l| {
                format!(
                    "{} {}\n",
                    show(l.at.map(|t| t.0.to_string())),
                    show(l.fingerprint.map(|f| f.to_string()))
                )
            })
            .collect();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = pigeon_sys::fs::write_atomic(path, text.as_bytes());
}

/// Whether there is a desktop to show notifications on.
fn desktop() -> bool {
    cfg!(target_vendor = "apple")
        || ["WAYLAND_DISPLAY", "DISPLAY", "DBUS_SESSION_BUS_ADDRESS"]
            .iter()
            .any(|k| std::env::var_os(k).is_some_and(|v| !v.is_empty()))
}

fn message_id(kind: Kind) -> &'static str {
    match kind {
        Kind::NeedsYou => "notify-needs-you",
        Kind::NotSafeLong => "notify-not-safe-long",
        Kind::Waiting => "notify-waiting",
    }
}

/// When `check` last looked, in seconds: it looks every five minutes at
/// most, as reading the state costs (ADR 0037), and every round in the lab.
static LOOKED: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(i64::MIN);

/// Decides and shows this round's notifications.
pub async fn check(d: &Daemon) {
    use std::sync::atomic::Ordering;
    let now = crate::util::now_ns() / 1_000_000_000;
    let lab = std::env::var_os("PIGEON_NOTIFY_LOG").is_some();
    let looked = LOOKED.load(Ordering::Relaxed);
    if !lab && looked != i64::MIN && now - looked < 300 {
        return;
    }
    LOOKED.store(now, Ordering::Relaxed);
    let Some(situation) = situation(d, now).await else {
        return;
    };
    let config = crate::config::load(&d.dirs.config_file()).unwrap_or_default();
    let log = std::env::var_os("PIGEON_NOTIFY_LOG").map(PathBuf::from);
    let enabled = config.notifications.unwrap_or(true) && (log.is_some() || desktop());
    let file = memory_file(d);
    let (memory, shown) = step(load(&file), &situation, Time(now), enabled);
    save(&file, &memory);
    if shown.is_empty() {
        return;
    }
    let lang = crate::web::language(config.language.as_deref());
    let cat = pigeon_i18n::Catalogue::new(&lang, false);
    let texts: Vec<String> = shown
        .iter()
        .map(|k| cat.format(&pigeon_i18n::Message::new(message_id(*k))))
        .collect();
    tracing::info!(count = texts.len(), "notified");
    if let Some(log) = log {
        let lines: String = shown
            .iter()
            .map(|k| format!("{}\n", message_id(*k)))
            .collect();
        let mut old = std::fs::read_to_string(&log).unwrap_or_default();
        old.push_str(&lines);
        let _ = std::fs::write(&log, old);
        return;
    }
    let url = d.web.get().map(|w| w.login_url());
    let open = cat.format(&pigeon_i18n::Message::new("notify-open"));
    // Grouped: one notification for all kinds of this round.
    let body = texts.join("\n");
    tokio::task::spawn_blocking(move || show(&body, &open, url));
}

/// Shows one notification; a click opens `url` (Linux).
fn show(body: &str, open: &str, url: Option<String>) {
    let mut n = notify_rust::Notification::new();
    n.appname("pigeon").summary("pigeon").body(body);
    #[cfg(all(unix, not(target_vendor = "apple")))]
    {
        n.action("default", open);
        if let Ok(handle) = n.show() {
            handle.wait_for_action(|action| {
                if action == "default"
                    && let Some(url) = &url
                {
                    let _ = std::process::Command::new("xdg-open")
                        .arg(url)
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn();
                }
            });
        }
    }
    #[cfg(target_vendor = "apple")]
    {
        let _ = (open, url);
        let _ = n.show();
    }
}
