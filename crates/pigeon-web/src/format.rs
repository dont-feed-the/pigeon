//! Small pieces every page shows the same way (ux.md §2.1, §4): sizes in
//! the person's language, times relative under a day and as a date beyond
//! (which the client shows in local time), and a state as an icon, a word
//! and a colour.

use maud::{Markup, html};
use pigeon_i18n::Message;
use pigeon_proto::api::State;

use crate::Renderer;
use crate::icons::icon;

/// `bytes` for people: `512 B`, `1.5 MB`, `16 GB` (decimal units).
pub(crate) fn size(r: &Renderer, bytes: u64) -> String {
    r.size(bytes)
}

/// A past time: "4 minutes ago" under a day, else its date, which the
/// client rewrites in local time; the exact time is its `datetime`.
pub(crate) fn when(r: &Renderer, rfc3339: &str) -> Markup {
    let Some(at) = pigeon_proto::time::parse_rfc3339(rfc3339) else {
        return html! { (rfc3339) };
    };
    let age = crate::now().saturating_sub(at);
    html! {
        @if age < 86_400 {
            time datetime=(rfc3339) { (r.ago(age)) }
        } @else {
            time datetime=(rfc3339) data-date { (pigeon_proto::time::date(at)) }
        }
    }
}

/// The icon of a state (ux.md §2.1).
fn state_icon(s: State) -> &'static str {
    match s {
        State::Protected => "safe",
        State::Published => "clock",
        State::Local => "laptop",
        State::Missing => "alert",
        State::Erased => "erased",
    }
}

/// The class of a state's badge.
pub(crate) fn state_class(s: State) -> &'static str {
    match s {
        State::Protected => "st-protected",
        State::Published => "st-published",
        State::Local => "st-local",
        State::Missing => "st-missing",
        State::Erased => "st-erased",
    }
}

/// A state as a badge with `label`.
pub(crate) fn badge(s: State, label: &str) -> Markup {
    html! {
        span class={ "state " (state_class(s)) } { (icon(state_icon(s), "")) (label) }
    }
}

/// A version's state as a badge with its own words.
pub(crate) fn state(r: &Renderer, s: State) -> Markup {
    badge(s, &r.state(s))
}

/// How bad a state is, the worst first, for a folder's summary.
pub(crate) fn rank(s: State) -> u8 {
    match s {
        State::Missing => 0,
        State::Local => 1,
        State::Published => 2,
        State::Erased => 3,
        State::Protected => 4,
    }
}

/// A current file's badge also says when its space has been freed.
pub(crate) fn file_state(r: &Renderer, state: State, freed: bool) -> Markup {
    let label = if freed {
        r.catalogue()
            .format(&Message::new("state-not-here").arg("state", r.state(state)))
    } else {
        r.state(state)
    };
    badge(state, &label)
}

/// A folder's state from its files: "All safe" or "3 not safe yet".
pub(crate) fn folder_state(r: &Renderer, worst: State, not_safe: u64) -> Markup {
    if not_safe == 0 && worst == State::Protected {
        return badge(worst, &r.id("web-folder-all-safe"));
    }
    if not_safe == 0 {
        return state(r, worst);
    }
    let label = r.catalogue().format(
        &Message::new("web-folder-not-safe")
            .arg("count", i64::try_from(not_safe).unwrap_or(i64::MAX)),
    );
    badge(worst, &label)
}

#[cfg(test)]
mod tests {
    fn renderer(lang: &str) -> crate::Renderer {
        crate::Renderer::new(
            pigeon_i18n::Catalogue::new(&lang.parse().unwrap(), false),
            0,
            None,
        )
    }

    #[test]
    fn sizes_use_decimal_units_and_the_languages_separator() {
        let en = renderer("en");
        assert_eq!(super::size(&en, 0), "0 bytes");
        assert_eq!(super::size(&en, 999), "999 bytes");
        assert_eq!(super::size(&en, 1500), "1.5 kB");
        assert_eq!(super::size(&en, 16_200_000_000), "16.2 GB");
        let fr = renderer("fr");
        assert_eq!(super::size(&fr, 1_500_000), "1,5 Mo");
    }
}
