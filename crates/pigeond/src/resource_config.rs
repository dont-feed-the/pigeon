//! Local resource settings (ADR 0037, 11 §11.2, 15 §15.6). Every hand-set
//! detail overrides its preset. Invalid values reject the whole reading;
//! the live controller keeps its preceding valid settings.

use pigeon_core::{resources::Preset, scrub::Window};
use serde::Deserialize;

/// `[resources]` as written in config.toml; absent details follow the preset.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Resources {
    /// gentle, balanced or fast; absent means gentle.
    pub preset: Option<String>,
    /// auto or a positive byte rate such as 2 MB/s.
    pub upload: Option<String>,
    /// auto or a positive byte rate such as 2 MB/s.
    pub download: Option<String>,
    /// pause or continue; absent follows the preset.
    pub on_battery: Option<String>,
    /// Battery threshold override, inclusive range 0–100.
    pub battery_percent: Option<u8>,
    /// pause or continue; absent follows the preset.
    pub on_metered: Option<String>,
    /// Local windows disabling the automatic shaper, never explicit ceilings.
    pub full_speed: Vec<String>,
    /// Half-open local scrub window, defaulting to 01:00–06:00.
    #[serde(default = "scrub_window")]
    pub scrub_window: String,
}

fn scrub_window() -> String {
    "01:00-06:00".into()
}

impl Default for Resources {
    fn default() -> Self {
        Self {
            preset: None,
            upload: None,
            download: None,
            on_battery: None,
            battery_percent: None,
            on_metered: None,
            full_speed: Vec::new(),
            scrub_window: scrub_window(),
        }
    }
}

/// Validated effective choices, independent of configuration spelling.
#[derive(Clone, Debug)]
pub struct Effective {
    /// The base preset for scheduling and automatic shaping.
    pub preset: Preset,
    /// Explicit upload ceiling in bytes per second.
    pub upload: Option<u64>,
    /// Explicit download ceiling in bytes per second.
    pub download: Option<u64>,
    /// Pause content below this percentage; zero means continue.
    pub battery_below: u8,
    /// Pause content and maintenance on metered networks.
    pub avoid_metered: bool,
    /// Validated local full-speed windows.
    pub full_speed: Vec<Window>,
}

impl Resources {
    /// Resolve presets without silently accepting unsupported details.
    pub fn effective(&self) -> Result<Effective, String> {
        let preset = match self.preset.as_deref().unwrap_or("gentle") {
            "gentle" => Preset::Gentle,
            "balanced" => Preset::Balanced,
            "fast" => Preset::Fast,
            _ => return Err("resources.preset must be gentle, balanced or fast".into()),
        };
        let pause = |value: Option<&str>, default: bool, key: &str| match value {
            None => Ok(default),
            Some("pause") => Ok(true),
            Some("continue") => Ok(false),
            _ => Err(format!("resources.{key} must be pause or continue")),
        };
        let threshold = self.battery_percent.unwrap_or_else(|| {
            let threshold = preset.battery_threshold();
            if self.on_battery.as_deref() == Some("pause") && threshold == 0 {
                20
            } else {
                threshold
            }
        });
        if threshold > 100 {
            return Err("resources.battery_percent must be between 0 and 100".into());
        }
        let on_battery = pause(self.on_battery.as_deref(), threshold > 0, "on_battery")?;
        let on_metered = pause(
            self.on_metered.as_deref(),
            preset != Preset::Fast,
            "on_metered",
        )?;
        if Window::parse(&self.scrub_window).is_none() {
            return Err("resources.scrub_window must be HH:MM-HH:MM".into());
        }
        let full_speed = self
            .full_speed
            .iter()
            .map(|s| {
                Window::parse(s)
                    .ok_or_else(|| "resources.full_speed must contain HH:MM-HH:MM windows".into())
            })
            .collect::<Result<_, String>>()?;
        Ok(Effective {
            preset,
            upload: bandwidth(self.upload.as_deref().unwrap_or("auto"))?,
            download: bandwidth(self.download.as_deref().unwrap_or("auto"))?,
            battery_below: if on_battery { threshold } else { 0 },
            avoid_metered: on_metered,
            full_speed,
        })
    }
}

/// Strict positive byte rates, without floating-point rounding or overflow.
/// Decimal KB/MB/GB and binary KiB/MiB/GiB are supported, with up to six
/// fractional digits. A fraction smaller than one byte per second is invalid.
pub fn bandwidth(text: &str) -> Result<Option<u64>, String> {
    let bad = || "resources rate must be auto or a positive rate such as 2 MB/s".to_owned();
    let text = text.trim();
    if text == "auto" {
        return Ok(None);
    }
    let text = text.strip_suffix("/s").ok_or_else(bad)?;
    let boundary = text
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .ok_or_else(bad)?;
    let (number, unit) = text.split_at(boundary);
    let scale: u128 = match unit.trim() {
        "B" => 1,
        "KB" => 1_000,
        "MB" => 1_000_000,
        "GB" => 1_000_000_000,
        "KiB" => 1_024,
        "MiB" => 1_048_576,
        "GiB" => 1_073_741_824,
        _ => return Err(bad()),
    };
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || fraction.len() > 6
        || !fraction.bytes().all(|b| b.is_ascii_digit())
        || number.ends_with('.')
    {
        return Err(bad());
    }
    let whole = whole.parse::<u128>().map_err(|_| bad())?;
    let divisor = 10u128.pow(fraction.len() as u32);
    let fraction = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<u128>().map_err(|_| bad())?
    };
    let bytes = whole
        .checked_mul(scale)
        .and_then(|n| n.checked_add(fraction * scale / divisor))
        .ok_or_else(bad)?;
    let bytes = u64::try_from(bytes)
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(bad)?;
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::unwrap_used, reason = "validated configuration fixtures")]
    fn presets_and_manual_overrides_are_independent() {
        let mut config = Resources::default();
        let gentle = config.effective().unwrap();
        assert_eq!(gentle.preset, Preset::Gentle);
        assert_eq!(gentle.battery_below, 20);
        assert!(gentle.avoid_metered);
        config.preset = Some("fast".into());
        let fast = config.effective().unwrap();
        assert_eq!(fast.battery_below, 0);
        assert!(!fast.avoid_metered);
        config.battery_percent = Some(35);
        config.on_metered = Some("pause".into());
        let overrides = config.effective().unwrap();
        assert_eq!(overrides.battery_below, 35);
        assert!(overrides.avoid_metered);
        config.on_battery = Some("continue".into());
        assert_eq!(config.effective().unwrap().battery_below, 0);
        config.battery_percent = Some(101);
        assert!(config.effective().is_err());
    }

    #[test]
    fn rates_are_exact_and_invalid_values_never_turn_into_auto() {
        assert_eq!(bandwidth("auto"), Ok(None));
        assert_eq!(bandwidth("2 MB/s"), Ok(Some(2_000_000)));
        assert_eq!(bandwidth("0.5MiB/s"), Ok(Some(524_288)));
        assert_eq!(bandwidth("0.001 KB/s"), Ok(Some(1)));
        assert_eq!(bandwidth("18446744073709551615 B/s"), Ok(Some(u64::MAX)));
        for bad in [
            "0 B/s",
            "-1 MB/s",
            "NaN MB/s",
            "inf MB/s",
            "1 Mb/s",
            "1. MB/s",
            "1..5 MB/s",
            "18446744073709551616 B/s",
            "0.000001 B/s",
        ] {
            assert!(bandwidth(bad).is_err(), "{bad}");
        }
    }
}
