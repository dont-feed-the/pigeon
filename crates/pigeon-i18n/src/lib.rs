//! Fluent bundles, language choice and the completeness test (ADR 0028, 10 §10.5).
//!
//! All text meant for people lives in `i18n/<lang>/pigeon.ftl`, embedded at
//! build time, never in Rust code (ADR 0028). Code names a message by its id
//! and arguments ([`Message`], the `message` and `hint` of a Problem,
//! 10 §10.6), and renders it at the edge with a [`Catalogue`]. [`ids`] lists
//! every id; the completeness test checks it against every bundle.

pub mod ids;
pub mod lang;
pub mod recovery;
pub mod render;

use std::collections::BTreeMap;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use serde::{Deserialize, Serialize};
pub use unic_langid::LanguageIdentifier;

pub use lang::{AVAILABLE, Setting, choose, system_languages};

/// The embedded bundles: a language tag of [`AVAILABLE`] and its source.
pub const BUNDLES: &[(&str, &str)] = &[
    ("en", include_str!("../i18n/en/pigeon.ftl")),
    ("fr", include_str!("../i18n/fr/pigeon.ftl")),
];

/// One argument of a message: a count or a text (a name, a path, a time
/// already rendered).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Arg {
    /// A number, which selects plural forms.
    Number(i64),
    /// A text, inserted as it is.
    Text(String),
}

impl From<i64> for Arg {
    fn from(n: i64) -> Arg {
        Arg::Number(n)
    }
}

impl From<u64> for Arg {
    fn from(n: u64) -> Arg {
        Arg::Number(i64::try_from(n).unwrap_or(i64::MAX))
    }
}

impl From<&str> for Arg {
    fn from(s: &str) -> Arg {
        Arg::Text(s.to_owned())
    }
}

impl From<String> for Arg {
    fn from(s: String) -> Arg {
        Arg::Text(s)
    }
}

/// A message id with its arguments, as a Problem carries it and `--json`
/// shows it: `{"id": "problem-invite-expired", "args": {"until": "…"}}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// The Fluent id, one of [`ids::MESSAGES`].
    pub id: String,
    /// The arguments by variable name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, Arg>,
}

impl Message {
    /// A message without arguments yet.
    pub fn new(id: impl Into<String>) -> Message {
        Message {
            id: id.into(),
            args: BTreeMap::new(),
        }
    }

    /// Adds the argument `name`.
    #[must_use]
    pub fn arg(mut self, name: &str, value: impl Into<Arg>) -> Message {
        self.args.insert(name.to_owned(), value.into());
        self
    }
}

/// Why a message does not fit the catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mismatch {
    /// No message has this id.
    UnknownId(String),
    /// The arguments given differ from the variables the message takes.
    Args {
        /// The id.
        id: String,
        /// The variables it takes.
        expected: Vec<String>,
        /// The arguments given.
        given: Vec<String>,
    },
}

/// Checks that `msg` names a message of the catalogue with exactly its
/// variables. Renderers call it in their tests.
pub fn check(msg: &Message) -> Result<(), Mismatch> {
    let entry = ids::entry(&msg.id).ok_or_else(|| Mismatch::UnknownId(msg.id.clone()))?;
    let mut expected: Vec<String> = entry.vars.iter().map(|v| (*v).to_owned()).collect();
    expected.sort();
    let given: Vec<String> = msg.args.keys().cloned().collect();
    if expected == given {
        Ok(())
    } else {
        Err(Mismatch::Args {
            id: msg.id.clone(),
            expected,
            given,
        })
    }
}

/// The source of the bundle for `lang`, English when there is none.
pub fn source(lang: &LanguageIdentifier) -> &'static str {
    let tag = lang.language.as_str();
    BUNDLES
        .iter()
        .find(|(t, _)| *t == tag)
        .or_else(|| BUNDLES.first())
        .map_or("", |(_, src)| *src)
}

/// One language's messages, ready to render.
pub struct Catalogue {
    lang: LanguageIdentifier,
    bundle: FluentBundle<FluentResource>,
}

impl Catalogue {
    /// The catalogue of `lang`, from [`choose`]. `isolate` wraps each
    /// argument in Unicode isolation marks, for the web interface; a
    /// terminal wants none.
    pub fn new(lang: &LanguageIdentifier, isolate: bool) -> Catalogue {
        let resource = match FluentResource::try_new(source(lang).to_owned()) {
            Ok(r) => r,
            // The completeness test keeps the bundles free of errors; a
            // resource with errors still holds every message it could read.
            Err((r, _)) => r,
        };
        let mut bundle = FluentBundle::new_concurrent(vec![lang.clone()]);
        bundle.set_use_isolating(isolate);
        // Ids are unique, as the completeness test checks; on a duplicate
        // the first message stays.
        let _ = bundle.add_resource(resource);
        Catalogue {
            lang: lang.clone(),
            bundle,
        }
    }

    /// The language of this catalogue.
    pub fn lang(&self) -> &LanguageIdentifier {
        &self.lang
    }

    /// Whether the catalogue has a message `id`.
    pub fn has(&self, id: &str) -> bool {
        self.bundle.has_message(id)
    }

    /// Renders `msg`, with the errors Fluent reports (an unknown variable,
    /// for example). An unknown id renders as the id itself.
    pub fn render(&self, msg: &Message) -> (String, Vec<String>) {
        let Some(pattern) = self.bundle.get_message(&msg.id).and_then(|m| m.value()) else {
            return (msg.id.clone(), vec![format!("unknown message {}", msg.id)]);
        };
        let mut args = FluentArgs::new();
        for (name, value) in &msg.args {
            let v = match value {
                Arg::Number(n) => FluentValue::from(*n),
                Arg::Text(t) => FluentValue::from(t.as_str()),
            };
            args.set(name.as_str(), v);
        }
        let mut errors = Vec::new();
        let text = self
            .bundle
            .format_pattern(pattern, Some(&args), &mut errors)
            .into_owned();
        (text, errors.iter().map(ToString::to_string).collect())
    }

    /// Renders `msg` for a person.
    pub fn format(&self, msg: &Message) -> String {
        self.render(msg).0
    }
}

#[cfg(test)]
mod tests;
