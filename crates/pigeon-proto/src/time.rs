//! Times as the protocol writes them: RFC 3339 in UTC, to the second
//! (10 §10.1), and the civil-date arithmetic behind them, so that `pigeond`
//! and the `pigeon` CLI read and write the same text.

/// The civil date (year, month from 1, day from 1) of a day counted from
/// 1970-01-01, in the proleptic Gregorian calendar.
pub fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The UTC date of `t`, `YYYY-MM-DD`.
pub fn date(t: i64) -> String {
    let (y, m, d) = civil(t.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// `t` in RFC 3339, UTC, to the second.
pub fn rfc3339(t: i64) -> String {
    let s = t.rem_euclid(86_400);
    format!(
        "{}T{:02}:{:02}:{:02}Z",
        date(t),
        s / 3600,
        s / 60 % 60,
        s % 60
    )
}

/// The seconds of an RFC 3339 time with `Z` or a `±HH:MM` offset, without
/// fractions.
pub fn parse_rfc3339(text: &str) -> Option<i64> {
    let num = |a: usize, b: usize| -> Option<i64> {
        let s = text.get(a..b)?;
        s.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| s.parse().ok())?
    };
    let shape = text.get(4..5)? == "-"
        && text.get(7..8)? == "-"
        && matches!(text.get(10..11)?, "T" | "t" | " ")
        && text.get(13..14)? == ":"
        && text.get(16..17)? == ":";
    if !shape {
        return None;
    }
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, s) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 60 {
        return None;
    }
    let offset = match text.get(19..)? {
        "Z" | "z" => 0,
        o if o.len() == 6 && o.get(3..4)? == ":" => {
            let sign = match o.get(0..1)? {
                "+" => 1,
                "-" => -1,
                _ => return None,
            };
            let oh: i64 = o.get(1..3)?.parse().ok()?;
            let om: i64 = o.get(4..6)?.parse().ok()?;
            sign * (oh * 3600 + om * 60)
        }
        _ => return None,
    };
    Some(days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + s - offset)
}
