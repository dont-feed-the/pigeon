//! The same resource preferences for CLI and web (15 §15.6). Parse a
//! complete candidate before writing, preserving unrelated configuration.

use crate::resource_config::Resources;
use pigeon_proto::api::Setting;

/// All editable keys, including the preset's independently overridable details.
pub const KEYS: &[&str] = &[
    "preset",
    "upload",
    "download",
    "on_battery",
    "battery_percent",
    "on_metered",
    "full_speed",
    "scrub_window",
];

/// Values are effective where a detail follows its preset; no selected
/// menu option silently contradicts what the controller is doing.
pub fn rows(r: &Resources) -> Result<Vec<Setting>, String> {
    let effective = r.effective()?;
    let values = [
        r.preset.clone().unwrap_or_else(|| "gentle".into()),
        r.upload.clone().unwrap_or_else(|| "auto".into()),
        r.download.clone().unwrap_or_else(|| "auto".into()),
        if effective.battery_below == 0 {
            "continue"
        } else {
            "pause"
        }
        .into(),
        effective.battery_below.to_string(),
        if effective.avoid_metered {
            "pause"
        } else {
            "continue"
        }
        .into(),
        r.full_speed.join(", "),
        r.scrub_window.clone(),
    ];
    Ok(KEYS
        .iter()
        .zip(values)
        .map(|(key, value)| Setting {
            key: format!("resources.{key}"),
            value,
            choices: match *key {
                "preset" => vec!["gentle".into(), "balanced".into(), "fast".into()],
                "on_battery" | "on_metered" => vec!["pause".into(), "continue".into()],
                _ => Vec::new(),
            },
        })
        .collect())
}

/// Validate against the whole existing configuration before a durable write.
/// Choosing a preset keeps explicitly chosen details: they override it.
pub fn set(path: &std::path::Path, key: &str, text: &str) -> Result<(), String> {
    if !KEYS.contains(&key) {
        return Err("unknown resource setting".into());
    }
    let current = crate::config::load(path).map_err(|e| e.to_string())?;
    let mut candidate = current.resources;
    match key {
        "preset" => candidate.preset = Some(text.into()),
        "upload" => candidate.upload = Some(text.into()),
        "download" => candidate.download = Some(text.into()),
        "on_battery" => candidate.on_battery = Some(text.into()),
        "battery_percent" => {
            candidate.battery_percent = Some(
                text.parse()
                    .map_err(|_| "expected a percentage from 0 to 100")?,
            )
        }
        "on_metered" => candidate.on_metered = Some(text.into()),
        "full_speed" => candidate.full_speed = windows(text),
        "scrub_window" => candidate.scrub_window = text.into(),
        _ => return Err("unknown resource setting".into()),
    }
    candidate.effective()?;
    let value: toml_edit::Value = match key {
        "battery_percent" => i64::from(candidate.battery_percent.unwrap_or(0)).into(),
        "full_speed" => toml_edit::Value::Array(windows(text).into_iter().collect()),
        _ => text.into(),
    };
    crate::config::set_resource(path, key, Some(value)).map_err(|e| e.to_string())
}

fn windows(text: &str) -> Vec<String> {
    if text.trim().is_empty() {
        Vec::new()
    } else {
        text.split(',').map(|s| s.trim().to_owned()).collect()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "resource preference fixtures")]
    use super::*;
    #[test]
    fn invalid_settings_leave_the_file_exact_and_presets_keep_manual_details() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let original = "format = 1\nlanguage = \"fr\"\n[resources]\nupload = \"2 MB/s\"\npaused_until = \"resume\"\n";
        std::fs::write(&path, original).unwrap();
        for (key, value) in [
            ("upload", "-1 MB/s"),
            ("full_speed", "03:00-04:00, bad"),
            ("battery_percent", "101"),
            ("preset", "invalid"),
        ] {
            assert!(set(&path, key, value).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        }
        set(&path, "preset", "fast").unwrap();
        set(&path, "full_speed", "03:00-04:00, 22:00-23:00").unwrap();
        let config = crate::config::load(&path).unwrap();
        assert_eq!(
            config.resources.effective().unwrap().upload,
            Some(2_000_000)
        );
        assert_eq!(config.language.as_deref(), Some("fr"));
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("paused_until = \"resume\"")
        );
        assert_eq!(rows(&config.resources).unwrap().len(), KEYS.len());
    }
}
