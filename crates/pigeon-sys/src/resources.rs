//! Native power and network observations (ADR 0037, 15 §15.3). Missing
//! services or unknown properties remain unknown; callers use the specified
//! permissive fallback. No shell command or environment variable supplies
//! an observation.

#[cfg(target_os = "macos")]
#[path = "resources_macos.rs"]
mod apple;

/// Latest native observations, separate from the person's chosen policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Observation {
    /// Percentage only while the computer is running on battery.
    pub battery: Option<u8>,
    /// The operating system's low-power mode, when available.
    pub low_power: Option<bool>,
    /// Whether the active network is expensive or constrained.
    pub metered: Option<bool>,
}

/// Reuses platform connections; dropping a failed reading invents no state.
#[derive(Default)]
pub struct Probe {
    #[cfg(target_os = "linux")]
    bus: Option<zbus::Connection>,
    #[cfg(target_os = "macos")]
    apple: apple::Probe,
}

impl Probe {
    /// Read native APIs. The adapter applies a deadline outside this call,
    /// so an unresponsive system service never blocks a content decision.
    pub async fn sample(&mut self) -> Observation {
        #[cfg(target_os = "linux")]
        {
            self.linux().await
        }
        #[cfg(target_os = "macos")]
        {
            self.apple.sample()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Observation::default()
        }
    }

    #[cfg(target_os = "linux")]
    async fn linux(&mut self) -> Observation {
        if self.bus.as_ref().is_some_and(|bus| bus.is_closed()) {
            self.bus = None;
        }
        if self.bus.is_none() {
            self.bus = zbus::Connection::system().await.ok();
        }
        let Some(bus) = self.bus.as_ref() else {
            return Observation::default();
        };
        let mut observation = Observation::default();
        if let Ok(power) = zbus::Proxy::new(
            bus,
            "org.freedesktop.UPower",
            "/org/freedesktop/UPower",
            "org.freedesktop.UPower",
        )
        .await
            && power.get_property::<bool>("OnBattery").await == Ok(true)
            && let Ok(display) = zbus::Proxy::new(
                bus,
                "org.freedesktop.UPower",
                "/org/freedesktop/UPower/devices/DisplayDevice",
                "org.freedesktop.UPower.Device",
            )
            .await
            && let Ok(percent) = display.get_property::<f64>("Percentage").await
        {
            observation.battery = percentage(percent);
        }
        if let Ok(profile) = zbus::Proxy::new(
            bus,
            "org.freedesktop.UPower.PowerProfiles",
            "/org/freedesktop/UPower/PowerProfiles",
            "org.freedesktop.UPower.PowerProfiles",
        )
        .await
        {
            observation.low_power = profile
                .get_property::<String>("ActiveProfile")
                .await
                .ok()
                .map(|p| p == "power-saver");
        }
        // NetworkManager: YES=1, NO=2, GUESS_YES=3, GUESS_NO=4,
        // UNKNOWN=0. A guess is usable, UNKNOWN remains unknown.
        if let Ok(network) = zbus::Proxy::new(
            bus,
            "org.freedesktop.NetworkManager",
            "/org/freedesktop/NetworkManager",
            "org.freedesktop.NetworkManager",
        )
        .await
        {
            observation.metered = network
                .get_property::<u32>("Metered")
                .await
                .ok()
                .and_then(metered);
        }
        observation
    }
}

#[cfg(any(target_os = "linux", test))]
fn metered(value: u32) -> Option<bool> {
    match value {
        1 | 3 => Some(true),
        2 | 4 => Some(false),
        _ => None,
    }
}

#[cfg(any(target_os = "linux", test))]
fn percentage(value: f64) -> Option<u8> {
    (value.is_finite() && (0.0..=100.0).contains(&value)).then_some(value as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_or_unknown_native_values_do_not_manufacture_conditions() {
        assert_eq!(metered(0), None);
        assert_eq!(metered(9), None);
        assert_eq!(metered(3), Some(true));
        assert_eq!(metered(4), Some(false));
        for value in [f64::NAN, f64::INFINITY, -1.0, 101.0] {
            assert_eq!(percentage(value), None);
        }
        assert_eq!(percentage(19.9), Some(19));
    }
}
