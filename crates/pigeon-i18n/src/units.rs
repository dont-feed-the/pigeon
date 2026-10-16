//! Times to come and sizes, as people read them (ux.md §4, 10 §10.5): a
//! deadline in this computer's local time, "until 14:38" today, "until
//! tomorrow at 09:00", else a date; and a size as "29.3 GB" with the
//! language's decimal mark and digit grouping. Every word and mark comes
//! from the catalogue (ADR 0028); the code only counts.

use crate::{Catalogue, Message};

/// The kinds of deadline a message argument names, by argument name, and
/// the message that words each.
pub(crate) const DEADLINES: &[(&str, &str)] =
    &[("until", "time-until"), ("expires", "time-expires")];

/// `at` as the argument `name` words it (one of [`DEADLINES`]), seen at
/// `now`; both in Unix seconds, shown in the local time zone that
/// `offset` gives in minutes east of UTC.
pub(crate) fn deadline(
    cat: &Catalogue,
    name: &str,
    at: i64,
    now: i64,
    offset: fn(i64) -> i32,
) -> String {
    let id = DEADLINES
        .iter()
        .find(|(arg, _)| *arg == name)
        .map_or("time-until", |(_, id)| id);
    let local = |t: i64| t.saturating_add(i64::from(offset(t)) * 60);
    let (at, now) = (local(at), local(now));
    let (day, today) = (at.div_euclid(86_400), now.div_euclid(86_400));
    let seconds = at.rem_euclid(86_400);
    let clock = cat.format(
        &Message::new("time-clock")
            .arg("hour", format!("{:02}", seconds / 3600))
            .arg("minute", format!("{:02}", seconds / 60 % 60)),
    );
    let moment = match day - today {
        0 => "today",
        1 => "tomorrow",
        _ => "other",
    };
    cat.format(
        &Message::new(id)
            .arg("moment", moment)
            .arg("clock", clock)
            .arg("date", date(cat, day, today)),
    )
}

/// The local date of `at`, seen at `now` ("16 October"), as [`deadline`]
/// reads them.
pub(crate) fn day(cat: &Catalogue, at: i64, now: i64, offset: fn(i64) -> i32) -> String {
    let local = |t: i64| t.saturating_add(i64::from(offset(t)) * 60);
    date(
        cat,
        local(at).div_euclid(86_400),
        local(now).div_euclid(86_400),
    )
}

/// The date of `day`, with its year when it is not the year of `today`
/// (both days since 1970-01-01).
fn date(cat: &Catalogue, day: i64, today: i64) -> String {
    let (year, month, d) = pigeon_proto::time::civil(day);
    let month = cat.format(&Message::new("time-month").arg("index", month));
    if year == pigeon_proto::time::civil(today).0 {
        cat.format(&Message::new("time-date").arg("day", d).arg("month", month))
    } else {
        cat.format(
            &Message::new("time-date-year")
                .arg("day", d)
                .arg("month", month)
                .arg("year", year.to_string()),
        )
    }
}

/// The ids of the units, each a thousand times the one before.
const UNITS: [&str; 5] = ["size-bytes", "size-kb", "size-mb", "size-gb", "size-tb"];

/// `bytes` in the largest unit below it, to a tenth below 100 of that
/// unit and whole above, as git-annex counts (a GB is 10⁹ bytes).
pub(crate) fn size(cat: &Catalogue, bytes: u64) -> String {
    let b = u128::from(bytes);
    let mut unit = 0;
    let mut base: u128 = 1;
    while unit < UNITS.len() - 1 && b >= base * 1000 {
        unit += 1;
        base *= 1000;
    }
    let tenths = |base: u128| (b * 10 + base / 2) / base;
    // 999.96 kB rounds to 1000 kB: say 1 MB instead.
    if unit > 0 && unit < UNITS.len() - 1 && tenths(base) >= 10_000 {
        unit += 1;
        base *= 1000;
    }
    let t = tenths(base);
    let text = if unit > 0 && t < 1000 && t % 10 != 0 {
        cat.format(
            &Message::new("number-decimal")
                .arg("whole", grouped(cat, t / 10))
                .arg("tenth", (t % 10).to_string()),
        )
    } else {
        grouped(cat, (b + base / 2) / base)
    };
    let message = Message::new(UNITS[unit]).arg("size", text);
    // Bytes are said in words, "1 byte", "300 bytes", which need a plural.
    let message = if unit == 0 {
        message.arg("plural", i64::try_from(bytes).unwrap_or(i64::MAX))
    } else {
        message
    };
    cat.format(&message)
}

/// `n` with the language's digit grouping, by thousands.
fn grouped(cat: &Catalogue, n: u128) -> String {
    if n < 1000 {
        return n.to_string();
    }
    cat.format(
        &Message::new("number-thousands")
            .arg("high", grouped(cat, n / 1000))
            .arg("low", format!("{:03}", n % 1000)),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "tests")]

    use super::*;

    fn cat(lang: &str) -> Catalogue {
        Catalogue::new(&lang.parse().unwrap(), false)
    }

    fn utc(_: i64) -> i32 {
        0
    }

    fn paris(_: i64) -> i32 {
        120
    }

    #[test]
    fn sizes_take_the_largest_unit_and_the_language_marks() {
        let (en, fr) = (cat("en"), cat("fr"));
        assert_eq!(size(&en, 0), "0 bytes");
        assert_eq!(size(&en, 1), "1 byte");
        assert_eq!(size(&fr, 1), "1 octet");
        assert_eq!(size(&en, 999), "999 bytes");
        assert_eq!(size(&en, 1000), "1 kB");
        assert_eq!(size(&en, 1500), "1.5 kB");
        assert_eq!(size(&en, 29_339_021_312), "29.3 GB");
        assert_eq!(size(&fr, 29_339_021_312), "29,3 Go");
        assert_eq!(size(&en, 512_000_000), "512 MB");
        assert_eq!(size(&en, 999_960), "1 MB");
        assert_eq!(size(&en, 99_960_000), "100 MB");
        assert_eq!(size(&en, 1_234_000_000_000_000), "1,234 TB");
        assert_eq!(size(&fr, 1_234_000_000_000_000), "1\u{202f}234 To");
        assert_eq!(size(&en, u64::MAX), "18,446,744 TB");
    }

    #[test]
    fn deadlines_say_today_tomorrow_or_a_date_in_local_time() {
        let en = cat("en");
        let fr = cat("fr");
        // 2026-10-16T12:38:00Z, a Friday.
        let now = pigeon_proto::time::parse_rfc3339("2026-10-16T10:00:00Z").unwrap();
        let at = |t: &str| pigeon_proto::time::parse_rfc3339(t).unwrap();
        assert_eq!(
            deadline(&en, "until", at("2026-10-16T12:38:00Z"), now, utc),
            "until 12:38"
        );
        assert_eq!(
            deadline(&en, "until", at("2026-10-16T12:38:00Z"), now, paris),
            "until 14:38"
        );
        assert_eq!(
            deadline(&en, "until", at("2026-10-17T07:00:00Z"), now, paris),
            "until tomorrow at 09:00"
        );
        assert_eq!(
            deadline(&en, "expires", at("2026-10-17T11:35:00Z"), now, paris),
            "expires tomorrow at 13:35"
        );
        assert_eq!(
            deadline(&en, "until", at("2026-10-19T07:00:00Z"), now, utc),
            "until 19 October at 07:00"
        );
        assert_eq!(
            deadline(&en, "until", at("2027-01-01T07:00:00Z"), now, utc),
            "until 1 January 2027 at 07:00"
        );
        // Late in the evening UTC is already tomorrow in Paris.
        assert_eq!(
            deadline(&en, "until", at("2026-10-16T22:30:00Z"), now, paris),
            "until tomorrow at 00:30"
        );
        assert_eq!(
            deadline(&fr, "until", at("2026-10-16T12:38:00Z"), now, paris),
            "jusqu’à 14 h 38"
        );
        assert_eq!(
            deadline(&fr, "expires", at("2026-10-17T11:35:00Z"), now, paris),
            "expire demain à 13 h 35"
        );
        assert_eq!(
            deadline(&fr, "until", at("2026-11-01T08:00:00Z"), now, utc),
            "jusqu’au 1er novembre à 08 h 00"
        );
    }
}
