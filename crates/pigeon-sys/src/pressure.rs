//! The system's I/O pressure (ADR 0037): on Linux the `some` line of
//! `/proc/pressure/io`, the share of time some task waited for I/O. macOS
//! has no such file; pigeon relies on the latency of its own reads there.

use crate::{Error, Result};

/// The averages of the `some` line, in percent of wall time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pressure {
    /// Over the last 10 seconds.
    pub avg10: f64,
    /// Over the last 60 seconds.
    pub avg60: f64,
    /// Total time waited, in microseconds, since boot.
    pub total_us: u64,
}

/// Parses the text of a PSI file such as `/proc/pressure/io`.
pub fn parse(text: &str) -> Result<Pressure> {
    let bad = || Error::Malformed {
        what: "/proc/pressure/io",
        detail: text.to_owned(),
    };
    let line = text
        .lines()
        .find(|l| l.starts_with("some "))
        .ok_or_else(bad)?;
    let field = |name: &str| {
        line.split_whitespace()
            .find_map(|w| w.strip_prefix(name)?.strip_prefix('='))
    };
    let num = |name: &str| field(name).and_then(|v| v.parse::<f64>().ok());
    Ok(Pressure {
        avg10: num("avg10").ok_or_else(bad)?,
        avg60: num("avg60").ok_or_else(bad)?,
        total_us: field("total")
            .and_then(|v| v.parse().ok())
            .ok_or_else(bad)?,
    })
}

/// The I/O pressure now, or `None` where the system does not report it (macOS,
/// or a Linux kernel without PSI).
pub fn io() -> Result<Option<Pressure>> {
    match std::fs::read_to_string("/proc/pressure/io") {
        Ok(text) => parse(&text).map(Some),
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::Unsupported
            ) =>
        {
            Ok(None)
        }
        Err(e) if e.raw_os_error() == Some(95) => Ok(None),
        Err(e) => Err(Error::path("read", "/proc/pressure/io", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_some_line() {
        let text = "some avg10=1.50 avg60=0.25 avg300=0.00 total=123456\n\
                    full avg10=0.00 avg60=0.00 avg300=0.00 total=0\n";
        let p = parse(text).unwrap();
        assert_eq!((p.avg10, p.avg60, p.total_us), (1.5, 0.25, 123456));
        assert!(parse("full avg10=0 avg60=0 total=0").is_err());
        assert!(parse("some avg10=x avg60=0 total=0").is_err());
    }

    #[test]
    fn reads_the_system_or_says_it_cannot() {
        let p = io().unwrap();
        if cfg!(target_vendor = "apple") {
            assert_eq!(p, None);
        }
    }
}
