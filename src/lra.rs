//! Loudness Range (LRA) per EBU Tech 3342.
//!
//! Algorithm:
//! 1. Compute short-term loudness (3 s window) every 100 ms.
//! 2. **Absolute gate:** discard short-term values below −70 LUFS.
//! 3. **Relative gate:** discard values more than 20 LU below the mean
//!    (in the linear MS domain) of the absolute-gated set.
//! 4. Sort the doubly-gated values, return the 95 % − 10 % percentile
//!    range, in LU.
//!
//! Percentiles use linear interpolation (Hyndman-Fan Type 7).

use alloc::vec::Vec;

use crate::analyzer::{ABSOLUTE_GATE_LUFS, LRA_RELATIVE_GATE_OFFSET_LU, LUFS_OFFSET};

#[inline]
fn lufs_to_ms_threshold(lufs: f64) -> f64 {
    libm::pow(10.0, (lufs - LUFS_OFFSET) / 10.0)
}

/// Compute LRA from a sequence of 3-second-window MS values (one per
/// 100 ms). Returns `None` if fewer than 2 values survive both gates.
pub(crate) fn compute_lra(short_term_ms: &[f32]) -> Option<f64> {
    if short_term_ms.len() < 2 {
        return None;
    }
    let abs_threshold_ms = lufs_to_ms_threshold(ABSOLUTE_GATE_LUFS);

    // Absolute gate.
    let mut sum: f64 = 0.0;
    let mut kept_ms: Vec<f64> = Vec::new();
    for &ms in short_term_ms {
        let msf = ms as f64;
        if msf > abs_threshold_ms {
            sum += msf;
            kept_ms.push(msf);
        }
    }
    if kept_ms.len() < 2 {
        return None;
    }

    // Relative gate at -20 LU below mean of absolute-gated set.
    let mean = sum / kept_ms.len() as f64;
    let rel_threshold_lufs = LUFS_OFFSET + 10.0 * libm::log10(mean) + LRA_RELATIVE_GATE_OFFSET_LU;
    let rel_threshold_ms = lufs_to_ms_threshold(rel_threshold_lufs);

    let mut kept_lufs: Vec<f64> = kept_ms
        .iter()
        .copied()
        .filter(|&ms| ms > rel_threshold_ms)
        .map(|ms| LUFS_OFFSET + 10.0 * libm::log10(ms))
        .collect();
    if kept_lufs.len() < 2 {
        return None;
    }

    // Sort ascending.
    kept_lufs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));

    let p10 = percentile_linear(&kept_lufs, 0.10);
    let p95 = percentile_linear(&kept_lufs, 0.95);
    Some(p95 - p10)
}

/// Linear-interpolated percentile (Hyndman-Fan Type 7).
fn percentile_linear(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return sorted[0];
    }
    let pos = p * (n - 1) as f64;
    let lo = libm::floor(pos) as usize;
    let hi = (lo + 1).min(n - 1);
    let frac = pos - lo as f64;
    sorted[lo] + frac * (sorted[hi] - sorted[lo])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_returns_none() {
        assert!(compute_lra(&[]).is_none());
    }

    #[test]
    fn single_value_returns_none() {
        let ms = lufs_to_ms_threshold(-23.0) as f32;
        assert!(compute_lra(&[ms]).is_none());
    }

    #[test]
    fn constant_programme_has_zero_lra() {
        let ms = lufs_to_ms_threshold(-23.0) as f32;
        let v = vec![ms; 200];
        let lra = compute_lra(&v).unwrap();
        assert!(lra.abs() < 1e-3, "got {lra}");
    }

    #[test]
    fn dynamic_programme_has_positive_lra() {
        // 100 quiet (-30) + 100 loud (-15) blocks → LRA ≈ 15 LU
        let quiet = lufs_to_ms_threshold(-30.0) as f32;
        let loud = lufs_to_ms_threshold(-15.0) as f32;
        let mut v = vec![quiet; 100];
        v.extend(vec![loud; 100]);
        let lra = compute_lra(&v).unwrap();
        assert!(lra > 10.0 && lra < 18.0, "expected ~15 LU, got {lra}");
    }
}
