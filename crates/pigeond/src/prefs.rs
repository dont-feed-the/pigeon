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
    let c = match config::load(&d.dirs.config_file()) {
        Ok(c) => c,
        Err(e) => return vec![problem("internal-error", &[], Some(e.to_string()))],
    };
    let settings = vec![
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
