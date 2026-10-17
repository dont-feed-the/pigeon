//! Native clocks: local offsets for names (05 §5.3) and elapsed time
//! including system sleep for capture settling and hot-path waits (15 §15.5).

/// Monotonic milliseconds on a clock that includes system sleep. Its epoch
/// is unspecified; compare readings only within this running system.
pub fn continuous_millis() -> std::io::Result<u64> {
    #[cfg(target_os = "linux")]
    let clock = libc::CLOCK_BOOTTIME;
    // Darwin's CLOCK_MONOTONIC_RAW is mach_continuous_time, including sleep;
    // CLOCK_UPTIME_RAW is the clock that excludes it (clock_gettime(3)).
    #[cfg(target_vendor = "apple")]
    let clock = libc::CLOCK_MONOTONIC_RAW;
    let mut reading = std::mem::MaybeUninit::<libc::timespec>::uninit();
    // SAFETY: reading is writable for one timespec; the platform clock id
    // is supported on pigeon's minimum Linux and macOS versions.
    if unsafe { libc::clock_gettime(clock, reading.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: the successful call initialized the complete timespec.
    let reading = unsafe { reading.assume_init() };
    let seconds = u64::try_from(reading.tv_sec).map_err(std::io::Error::other)?;
    let nanos = u64::try_from(reading.tv_nsec).map_err(std::io::Error::other)?;
    Ok(seconds
        .saturating_mul(1000)
        .saturating_add(nanos / 1_000_000))
}

/// Minutes east of UTC of the local time zone at `utc`, in seconds since
/// the epoch; 0 when the C library cannot tell.
pub fn local_offset_minutes(utc: i64) -> i32 {
    let t: libc::time_t = utc;
    // SAFETY: `tm` is a plain C struct, for which all zeroes is a value.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: both pointers are valid for the call, which writes only `tm`.
    let r = unsafe { libc::localtime_r(&t, &mut tm) };
    if r.is_null() {
        return 0;
    }
    i32::try_from(tm.tm_gmtoff / 60).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    #[test]
    fn continuous_clock_advances_without_using_wall_time() -> std::io::Result<()> {
        let before = super::continuous_millis()?;
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert!(super::continuous_millis()? >= before + 10);
        Ok(())
    }

    #[test]
    fn the_offset_is_whole_minutes_within_a_day() {
        let m = super::local_offset_minutes(1_791_892_800);
        assert!(m.abs() < 24 * 60, "{m}");
    }
}
