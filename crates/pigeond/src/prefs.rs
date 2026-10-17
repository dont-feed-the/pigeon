//! `pigeon settings` and the web's Settings page (10 §10.1, ux.md §6.5):
//! the settings of `config.toml` a person changes (11 §11.2), read afresh
//! from the file and written back with `toml_edit`. A new `language`
//! applies to the web interface at once.

use pigeon_proto::api::Setting;
use pigeon_proto::{Arg, Response};

use crate::api::problem;
use crate::config;
use crate::daemon::Daemon;

/// The values `language` may take.
fn languages() -> Vec<String> {
    std::iter::once("auto")
        .chain(pigeon_i18n::AVAILABLE.iter().copied())
        .map(str::to_owned)
        .collect()
}

fn yes_no() -> Vec<String> {
    vec!["true".to_owned(), "false".to_owned()]
}

/// `GetSettings`: all of them, or only `key`.
pub fn get(d: &Daemon, key: Option<&str>) -> Vec<Response> {
    // Operator setting: requested explicitly, since it changes group-wide
    // diagnostic sharing rather than this computer's preferences.
    if key == Some("telemetry-device") {
        let Some(doc) = d.default_group().and_then(|g| g.roster()) else {
            return vec![problem("not-in-group", &[], None)];
        };
        let value = doc
            .roster
            .telemetry_device
            .as_ref()
            .and_then(|id| doc.roster.devices.get(id))
            .map_or_else(|| "off".to_owned(), |device| device.name.clone());
        let choices = std::iter::once("off".to_owned())
            .chain(
                doc.roster
                    .devices
                    .values()
                    .filter(|d| d.role != pigeon_core::roster::Role::Recovery)
                    .map(|d| d.name.clone()),
            )
            .collect();
        return vec![Response::Settings {
            settings: vec![Setting {
                key: "telemetry-device".into(),
                value,
                choices,
            }],
        }];
    }

    let c = match config::load(&d.dirs.config_file()) {
        Ok(c) => c,
        Err(e) => return vec![problem("internal-error", &[], Some(e.to_string()))],
    };
    let mut settings = vec![
        Setting {
            key: "language".to_owned(),
            value: c.language.unwrap_or_else(|| "auto".to_owned()),
            choices: languages(),
        },
        Setting {
            key: "notifications".to_owned(),
            value: c.notifications.unwrap_or(true).to_string(),
            choices: yes_no(),
        },
    ];
    if let Ok(rows) = crate::resource_prefs::rows(&c.resources) {
        settings.extend(rows);
    }
    match key {
        None => vec![Response::Settings { settings }],
        Some(k) => match settings.into_iter().find(|s| s.key == k) {
            Some(s) => vec![Response::Settings { settings: vec![s] }],
            None => vec![problem(
                "setting-unknown",
                &[("key", Arg::Text(k.to_owned()))],
                None,
            )],
        },
    }
}

/// `SetSettings`: one key, to one of its values.
pub fn set(d: &Daemon, key: &str, value: &str) -> Vec<Response> {
    let value = value.trim();
    if let Some(detail) = key.strip_prefix("resources.") {
        return match crate::resource_prefs::set(&d.dirs.config_file(), detail, value) {
            Ok(()) => {
                if let Ok(config) = config::load(&d.dirs.config_file())
                    && let Ok(settings) = config.resources.effective()
                {
                    d.node.resources.configure(settings);
                }
                vec![crate::api::said(
                    "setting-changed",
                    &[
                        ("key", Arg::Text(key.into())),
                        ("value", Arg::Text(value.into())),
                    ],
                )]
            }
            Err(error) => vec![problem(
                "setting-value",
                &[
                    ("key", Arg::Text(key.into())),
                    ("value", Arg::Text(value.into())),
                ],
                Some(error),
            )],
        };
    }
    let new = match key {
        "language" if languages().iter().any(|l| l == value) => toml_edit::Value::from(value),
        "notifications" if yes_no().iter().any(|v| v == value) => {
            toml_edit::Value::from(value == "true")
        }
        "language" | "notifications" => {
            return vec![problem(
                "setting-value",
                &[
                    ("key", Arg::Text(key.to_owned())),
                    ("value", Arg::Text(value.to_owned())),
                ],
                None,
            )];
        }
        _ => {
            return vec![problem(
                "setting-unknown",
                &[("key", Arg::Text(key.to_owned()))],
                None,
            )];
        }
    };
    if let Err(e) = config::set_value(&d.dirs.config_file(), key, new) {
        return vec![problem("internal-error", &[], Some(e.to_string()))];
    }
    if key == "language"
        && let Some(web) = d.web.get()
    {
        web.set_lang(crate::web::language(Some(value)));
    }
    vec![crate::api::said(
        "setting-changed",
        &[
            ("key", Arg::Text(key.to_owned())),
            ("value", Arg::Text(value.to_owned())),
        ],
    )]
}
