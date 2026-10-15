//! The local time zone, for names people read (05 §5.3): the offset from
//! UTC that git records in a commit made at that time.

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
    fn the_offset_is_whole_minutes_within_a_day() {
        let m = super::local_offset_minutes(1_791_892_800);
        assert!(m.abs() < 24 * 60, "{m}");
    }
}
