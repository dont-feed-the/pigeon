//! Choosing the language (10 §10.5): the `language` setting, then the
//! system's languages, then English. The system's languages are read from
//! the environment as gettext reads them, through a lookup the caller
//! passes, so this crate reads no state of its own.

use fluent_langneg::{NegotiationStrategy, negotiate_languages};
use unic_langid::LanguageIdentifier;

/// The languages this build has a bundle for. English comes first: it is
/// the fallback, and the only bundle of the slice (ADR 0057).
pub const AVAILABLE: &[&str] = &["en"];

/// The `language` setting of `config.toml` (11 §11.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Setting {
    /// Follow the system.
    Auto,
    /// A language the person chose.
    Named(LanguageIdentifier),
}

impl Setting {
    /// Parses `auto` or a language tag such as `fr` or `en-GB`.
    pub fn parse(text: &str) -> Option<Setting> {
        let text = text.trim();
        if text.eq_ignore_ascii_case("auto") {
            return Some(Setting::Auto);
        }
        text.parse().ok().map(Setting::Named)
    }
}

/// A POSIX locale name such as `fr_FR.UTF-8@euro` as a language, or `None`
/// for `C`, `POSIX` and names that are no language.
pub fn from_posix(locale: &str) -> Option<LanguageIdentifier> {
    let name = locale.split(['.', '@']).next()?.trim();
    if name.is_empty() || name == "C" || name == "POSIX" {
        return None;
    }
    name.replace('_', "-").parse().ok()
}

/// The system's languages, most preferred first, read as gettext does: the
/// first of `LC_ALL`, `LC_MESSAGES` and `LANG` that is set gives the locale;
/// unless it is `C`, the list in `LANGUAGE` comes before it. `var` looks a
/// variable up, as `|k| std::env::var(k).ok()`. macOS settings for
/// applications started outside a terminal are not read yet.
pub fn system_languages(var: impl Fn(&str) -> Option<String>) -> Vec<LanguageIdentifier> {
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|k| var(k))
        .find(|v| !v.is_empty());
    let Some(locale) = locale else {
        return Vec::new();
    };
    let Some(main) = from_posix(&locale) else {
        return Vec::new();
    };
    let mut out: Vec<LanguageIdentifier> = var("LANGUAGE")
        .unwrap_or_default()
        .split(':')
        .filter_map(from_posix)
        .collect();
    if !out.contains(&main) {
        out.push(main);
    }
    out
}

fn available() -> Vec<LanguageIdentifier> {
    AVAILABLE.iter().filter_map(|t| t.parse().ok()).collect()
}

/// The language to show: the setting if it names one, else the system's
/// languages, each matched against [`AVAILABLE`]; English if none matches.
pub fn choose(setting: &Setting, system: &[LanguageIdentifier]) -> LanguageIdentifier {
    let requested: Vec<LanguageIdentifier> = match setting {
        Setting::Named(lang) => vec![lang.clone()],
        Setting::Auto => system.to_vec(),
    };
    let available = available();
    negotiate_languages(
        &requested,
        &available,
        available.first(),
        NegotiationStrategy::Lookup,
    )
    .first()
    .map(|l| (*l).clone())
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |k| map.get(k).cloned()
    }

    fn tags(langs: &[LanguageIdentifier]) -> Vec<String> {
        langs.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn posix_names_become_languages() {
        assert_eq!(
            from_posix("fr_FR.UTF-8@euro").map(|l| l.to_string()),
            Some("fr-FR".into())
        );
        assert_eq!(from_posix("en").map(|l| l.to_string()), Some("en".into()));
        assert_eq!(from_posix("C.UTF-8"), None);
        assert_eq!(from_posix("POSIX"), None);
        assert_eq!(from_posix(""), None);
    }

    #[test]
    fn the_first_set_variable_wins_and_language_comes_first() {
        let v = env(&[("LANG", "en_US.UTF-8"), ("LC_MESSAGES", "fr_CH.UTF-8")]);
        assert_eq!(tags(&system_languages(v)), ["fr-CH"]);
        let v = env(&[
            ("LC_ALL", "de_DE"),
            ("LANG", "fr_FR"),
            ("LANGUAGE", "fr:en"),
        ]);
        assert_eq!(tags(&system_languages(v)), ["fr", "en", "de-DE"]);
        let v = env(&[("LC_ALL", "C"), ("LANGUAGE", "fr")]);
        assert!(system_languages(v).is_empty());
        assert!(system_languages(env(&[])).is_empty());
    }

    #[test]
    fn the_setting_then_the_system_then_english() {
        let tag = |t: &str| t.parse::<LanguageIdentifier>().unwrap();
        let fr = vec![tag("fr-FR")];
        let en_gb = vec![tag("en-GB")];
        let english = tag("en");
        assert_eq!(choose(&Setting::Auto, &en_gb), english);
        assert_eq!(choose(&Setting::Auto, &[]), english);
        // No French bundle in the slice (ADR 0057): French falls back.
        assert_eq!(choose(&Setting::Auto, &fr), english);
        assert_eq!(choose(&Setting::parse("fr").unwrap(), &en_gb), english);
        assert_eq!(Setting::parse(" Auto "), Some(Setting::Auto));
        assert_eq!(Setting::parse("not a tag!"), None);
    }
}
