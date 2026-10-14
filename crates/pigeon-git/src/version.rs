//! The versions of git and git-annex, and the minimums CI tests (14 §14.3).
//! `pigeond` refuses to run below them with `git-annex-too-old`.

use crate::error::{Error, Result};

/// The oldest git CI tests (ADR 0010).
pub const MIN_GIT: (u32, u32, u32) = (2, 38, 0);

/// The oldest git-annex CI tests: on Linux the pinned standalone build, on
/// macOS the version Homebrew provided when E20 ran (14 §14.3).
pub const MIN_ANNEX: (u32, u32) = if cfg!(target_vendor = "apple") {
    (10, 20260901)
} else {
    (10, 20261006)
};

/// Parses `git --version`: `git version 2.43.0` (with any suffix).
pub fn parse_git(text: &str) -> Option<(u32, u32, u32)> {
    let v = text.split_whitespace().nth(2)?;
    let mut it = v.split('.').map(|p| {
        p.chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
    });
    let major = it.next()?.parse().ok()?;
    let minor = it.next()?.parse().ok()?;
    let patch = it.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    Some((major, minor, patch))
}

/// Parses `git annex version --raw`, or the first line of `git annex
/// version`: `10.20261006`, possibly followed by `-g<commit>`.
pub fn parse_annex(text: &str) -> Option<(u32, u32)> {
    let line = text.lines().next()?.trim();
    let v = line
        .strip_prefix("git-annex version:")
        .unwrap_or(line)
        .trim();
    let v = v.split(['-', '~', '+']).next()?;
    let (major, date) = v.split_once('.')?;
    Some((major.parse().ok()?, date.parse().ok()?))
}

/// Refuses a git below [`MIN_GIT`].
pub fn check_git(text: &str) -> Result<(u32, u32, u32)> {
    let found = parse_git(text);
    match found {
        Some(v) if v >= MIN_GIT => Ok(v),
        _ => Err(Error::TooOld {
            program: "git",
            found: text.trim().to_owned(),
            minimum: format!("{}.{}.{}", MIN_GIT.0, MIN_GIT.1, MIN_GIT.2),
        }),
    }
}

/// Refuses a git-annex below [`MIN_ANNEX`].
pub fn check_annex(text: &str) -> Result<(u32, u32)> {
    match parse_annex(text) {
        Some(v) if v >= MIN_ANNEX => Ok(v),
        _ => Err(Error::TooOld {
            program: "git-annex",
            found: text.lines().next().unwrap_or("").trim().to_owned(),
            minimum: format!("{}.{}", MIN_ANNEX.0, MIN_ANNEX.1),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_parse_and_compare() {
        assert_eq!(parse_git("git version 2.43.0\n"), Some((2, 43, 0)));
        assert_eq!(
            parse_git("git version 2.39.5 (Apple Git-154)"),
            Some((2, 39, 5))
        );
        assert_eq!(parse_git("git version 2.50.0.windows.1"), Some((2, 50, 0)));
        assert!(
            check_git("git version 2.37.9").is_err() && check_git("git version 2.38.0").is_ok()
        );
        assert_eq!(parse_annex("10.20261006-g28ea958\n"), Some((10, 20261006)));
        assert_eq!(
            parse_annex("git-annex version: 10.20250101\nbuild flags: x"),
            Some((10, 20250101))
        );
        assert!(check_annex("10.20240101").is_err() && check_annex("11.20200101").is_ok());
        assert!(check_annex("garbage").is_err());
    }
}
