//! Gated integrated loudness, per ITU-R BS.1770-4 §5.6.
//!
//! The integrator consumes the stream of 400 ms gating-block weighted
//! mean-square values produced by the analyzer (one per 100 ms step) and
//! returns the doubly-gated integrated LUFS value.
//!
//! **Algorithm (§5.6):**
//! 1. *Absolute gate.* Discard blocks whose loudness is below −70 LUFS.
//! 2. Compute the relative threshold as the (linear) mean of the
//!    surviving blocks' MS values, expressed in LUFS, minus 10 LU.
//! 3. *Relative gate.* Discard blocks below the relative threshold.
//! 4. The integrated value is the (linear) mean of the doubly-gated
//!    blocks, expressed in LUFS.
//!
//! Both means are taken in the linear MS domain — averaging dB values
//! directly would be incorrect.

use crate::analyzer::{ABSOLUTE_GATE_LUFS, LUFS_OFFSET, RELATIVE_GATE_OFFSET_LU};

/// Convert a gate threshold expressed in LUFS into the equivalent linear
/// mean-square value, given the BS.1770-4 LUFS_OFFSET (-0.691 dB).
#[inline]
fn lufs_to_ms_threshold(lufs: f64) -> f64 {
    libm::pow(10.0, (lufs - LUFS_OFFSET) / 10.0)
}

/// Compute integrated loudness from a slice of per-gating-block weighted
/// mean-square values. Returns `None` if no blocks survive the gates.
pub(crate) fn compute_integrated(blocks: &[f32]) -> Option<f64> {
    if blocks.is_empty() {
        return None;
    }
    let abs_threshold_ms = lufs_to_ms_threshold(ABSOLUTE_GATE_LUFS);

    // ---- Absolute gate ----
    let mut sum: f64 = 0.0;
    let mut count: u64 = 0;
    for &ms in blocks {
        let msf = ms as f64;
        if msf > abs_threshold_ms {
            sum += msf;
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }

    // ---- Relative gate ----
    let mean_after_abs = sum / count as f64;
    let relative_threshold_lufs =
        LUFS_OFFSET + 10.0 * libm::log10(mean_after_abs) + RELATIVE_GATE_OFFSET_LU;
    let rel_threshold_ms = lufs_to_ms_threshold(relative_threshold_lufs);

    let mut sum2: f64 = 0.0;
    let mut count2: u64 = 0;
    for &ms in blocks {
        let msf = ms as f64;
        if msf > abs_threshold_ms && msf > rel_threshold_ms {
            sum2 += msf;
            count2 += 1;
        }
    }
    if count2 == 0 {
        return None;
    }

    let mean = sum2 / count2 as f64;
    Some(LUFS_OFFSET + 10.0 * libm::log10(mean))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_yields_none() {
        assert!(compute_integrated(&[]).is_none());
    }

    #[test]
    fn all_silent_blocks_yield_none() {
        // A block with weighted-MS of 0 corresponds to negative infinity
        // LUFS — well below the −70 absolute gate.
        let blocks = vec![0.0f32; 100];
        assert!(compute_integrated(&blocks).is_none());
    }

    #[test]
    fn constant_loudness_returns_calibration() {
        // A programme of identical blocks at known MS should integrate
        // to that exact LUFS.
        let target_lufs = -23.0_f64;
        let target_ms = lufs_to_ms_threshold(target_lufs) as f32;
        let blocks = vec![target_ms; 200];
        let lufs = compute_integrated(&blocks).unwrap();
        assert!((lufs - target_lufs).abs() < 1e-3, "got {lufs}");
    }

    #[test]
    fn quiet_blocks_excluded_by_absolute_gate() {
        // A programme that's mostly silent with one loud block should
        // integrate to close to the loud block's loudness.
        let loud_lufs = -20.0_f64;
        let loud_ms = lufs_to_ms_threshold(loud_lufs) as f32;
        let mut blocks = vec![0.0f32; 999];
        blocks.push(loud_ms);
        let lufs = compute_integrated(&blocks).unwrap();
        assert!((lufs - loud_lufs).abs() < 1.0, "got {lufs}");
    }
}
