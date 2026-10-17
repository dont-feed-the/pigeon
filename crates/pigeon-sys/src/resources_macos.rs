//! Native macOS observations (ADR 0037, 15 §15.3): IOKit battery state,
//! NSProcessInfo low-power mode and an asynchronous Network.framework
//! path monitor. Its serial queue owns callback execution, never Tokio.

use std::ffi::{c_char, c_void};
use std::ptr;
use std::sync::{Arc, Mutex};

use block2::{Block, RcBlock};
use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFType, CFTypeRef, TCFType};
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::number::CFNumber;
use core_foundation::string::{CFString, CFStringRef};

use super::Observation;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOPSCopyPowerSourcesInfo() -> CFTypeRef;
    fn IOPSCopyPowerSourcesList(info: CFTypeRef) -> CFArrayRef;
    fn IOPSGetProvidingPowerSourceType(info: CFTypeRef) -> CFStringRef;
    fn IOPSGetPowerSourceDescription(info: CFTypeRef, source: CFTypeRef) -> CFDictionaryRef;
}

#[link(name = "Network", kind = "framework")]
unsafe extern "C" {
    fn nw_path_monitor_create() -> *mut c_void;
    fn nw_path_monitor_set_update_handler(
        monitor: *mut c_void,
        handler: &Block<dyn Fn(*mut c_void)>,
    );
    fn nw_path_monitor_set_queue(monitor: *mut c_void, queue: *mut c_void);
    fn nw_path_monitor_start(monitor: *mut c_void);
    fn nw_path_monitor_cancel(monitor: *mut c_void);
    fn nw_path_is_expensive(path: *mut c_void) -> bool;
    fn nw_path_is_constrained(path: *mut c_void) -> bool;
    fn nw_release(object: *mut c_void);
}

unsafe extern "C" {
    fn dispatch_queue_create(label: *const c_char, attr: *const c_void) -> *mut c_void;
    fn dispatch_release(object: *mut c_void);
}

/// Monitor ownership stays in the probe; callbacks retain their own Arc.
pub(super) struct Probe {
    monitor: *mut c_void,
    queue: *mut c_void,
    metered: Arc<Mutex<Option<bool>>>,
}

// SAFETY: Network.framework monitors and dispatch queues are thread safe.
// Only the serial queue runs the callback, and its shared value is locked.
// Moving the owning handle never moves or aliases the native object itself.
unsafe impl Send for Probe {}

impl Default for Probe {
    fn default() -> Self {
        let metered = Arc::new(Mutex::new(None));
        let observed = metered.clone();
        let update = RcBlock::new(move |path: *mut c_void| {
            if path.is_null() {
                return;
            }
            // SAFETY: Network.framework supplies a live nw_path_t for this
            // callback. Both properties only read it before the callback ends.
            let expensive = unsafe { nw_path_is_expensive(path) || nw_path_is_constrained(path) };
            *observed.lock().unwrap_or_else(|p| p.into_inner()) = Some(expensive);
        });
        // SAFETY: create functions return owned nullable handles. The label
        // is NUL terminated; a null attr requests a serial dispatch queue.
        let monitor = unsafe { nw_path_monitor_create() };
        // SAFETY: valid static C string and the documented default attr.
        let queue = unsafe { dispatch_queue_create(c"org.pigeon.resources".as_ptr(), ptr::null()) };
        if !monitor.is_null() && !queue.is_null() {
            // SAFETY: both handles are live and owned here. Setting the
            // handler copies its block (retaining the Arc); the monitor
            // retains the queue. Start happens only after both are set.
            unsafe {
                nw_path_monitor_set_update_handler(monitor, &update);
                nw_path_monitor_set_queue(monitor, queue);
                nw_path_monitor_start(monitor);
            }
        }
        Self {
            monitor,
            queue,
            metered,
        }
    }
}

impl Probe {
    pub(super) fn sample(&self) -> Observation {
        Observation {
            battery: battery(),
            low_power: Some(objc2_foundation::NSProcessInfo::processInfo().isLowPowerModeEnabled()),
            metered: *self.metered.lock().unwrap_or_else(|p| p.into_inner()),
        }
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        // SAFETY: nullable owned handles are released exactly once. Cancel
        // stops future updates; in-flight callbacks own their Arc and the
        // monitor retains its queue until their execution has finished.
        unsafe {
            if !self.monitor.is_null() {
                nw_path_monitor_cancel(self.monitor);
                nw_release(self.monitor);
            }
            if !self.queue.is_null() {
                dispatch_release(self.queue);
            }
        }
    }
}

fn battery() -> Option<u8> {
    // SAFETY: IOKit returns an owned snapshot, or null on failure.
    let raw = unsafe { IOPSCopyPowerSourcesInfo() };
    if raw.is_null() {
        return None;
    }
    // SAFETY: non-null owned IOKit CFTypeRef follows the create rule.
    let info = unsafe { CFType::wrap_under_create_rule(raw) };
    // SAFETY: info remains live; IOKit lends this string from the snapshot.
    let raw_source = unsafe { IOPSGetProvidingPowerSourceType(info.as_CFTypeRef()) };
    if raw_source.is_null() {
        return None;
    }
    // SAFETY: non-null borrowed CFStringRef, retained under the get rule.
    let source = unsafe { CFString::wrap_under_get_rule(raw_source) };
    if source != "Battery Power" {
        return None;
    }
    // SAFETY: live snapshot; the returned array is owned or null.
    let raw_sources = unsafe { IOPSCopyPowerSourcesList(info.as_CFTypeRef()) };
    if raw_sources.is_null() {
        return None;
    }
    // SAFETY: the documented array holds CFType power source identifiers.
    let sources: CFArray<CFType> = unsafe { CFArray::wrap_under_create_rule(raw_sources) };
    let (mut current, mut maximum) = (0i64, 0i64);
    for item in &sources {
        // SAFETY: both snapshot and source identifier are live. The result
        // is a borrowed dictionary whose entries are CFString -> CFType.
        let raw =
            unsafe { IOPSGetPowerSourceDescription(info.as_CFTypeRef(), item.as_CFTypeRef()) };
        if raw.is_null() {
            continue;
        }
        // SAFETY: non-null borrowed dictionary, retained by the get rule.
        let description: CFDictionary<CFString, CFType> =
            unsafe { CFDictionary::wrap_under_get_rule(raw) };
        let number = |key| {
            description
                .find(CFString::new(key))?
                .downcast::<CFNumber>()?
                .to_i64()
        };
        let (Some(capacity), Some(full)) = (number("Current Capacity"), number("Max Capacity"))
        else {
            continue;
        };
        if capacity < 0 || full <= 0 || capacity > full {
            continue;
        }
        current = current.checked_add(capacity)?;
        maximum = maximum.checked_add(full)?;
    }
    if maximum == 0 {
        None
    } else {
        u8::try_from(i128::from(current) * 100 / i128::from(maximum)).ok()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[allow(
        clippy::unwrap_used,
        reason = "native platform observation and monitor lifecycle"
    )]
    fn monitor_moves_between_threads_and_samples_native_apis() {
        let probe = super::Probe::default();
        let observed = std::thread::spawn(move || probe.sample()).join().unwrap();
        assert!(observed.battery.is_none_or(|p| p <= 100));
    }
}
