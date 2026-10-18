//! Bounded capture input batches (15 §15.5): preserve every path in order,
//! with at most 1024 paths and 64 KiB of NUL-separated input per batch.
//! An individually oversized path stands alone, so the partition progresses.
//! Greedy contiguous batches avoid unnecessary native index rewrites.

use std::ops::Range;

/// Maximum number of path identities retained by capture progress tracking.
pub const MAX_FILES: usize = 1024;
/// Maximum ordinary input bytes, including each path's NUL terminator.
pub const MAX_INPUT_BYTES: usize = 64 * 1024;

/// Partition the input without copying paths, dropping entries or reordering.
/// Laws: exact ordered coverage, bounded nonempty batches, greedy packing,
/// and progress even for a single input larger than the ordinary byte bound.
pub fn ranges(paths: &[String]) -> impl Iterator<Item = Range<usize>> + '_ {
    let mut at = 0;
    std::iter::from_fn(move || {
        if at == paths.len() {
            return None;
        }
        let start = at;
        let mut bytes = 0usize;
        while at < paths.len() && at - start < MAX_FILES {
            let next = bytes.saturating_add(paths[at].len()).saturating_add(1);
            if at > start && next > MAX_INPUT_BYTES {
                break;
            }
            bytes = next;
            at += 1;
        }
        Some(start..at)
    })
}
