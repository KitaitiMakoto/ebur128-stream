//! Loudness normalisation helpers.
//!
//! Two pieces:
//!
//! 1. [`gain_to_target`] — pure function: given a measured [`Report`]
//!    and a target LUFS, return the linear gain to apply.
//! 2. [`Normalizer`] — convenience pipeline: run a buffer through the
//!    analyzer, apply the calibrated gain, optionally clip-protect
//!    against true-peak overshoot.
//!
//! The normaliser is intentionally a *separate* concern from the
//! analyzer (per the SPEC discipline) but lives in the same crate so
//! callers don't have to wire the math themselves. This module is
//! gated behind the `normalize` feature.
//!
//! # Example
//!
//! ```
//! use ebur128_stream::{normalize::Normalizer, Channel, Mode};
//!
//! // 1 second of -3 dBFS sine (≈ -3 LUFS-ish)
//! let mut buf: Vec<f32> = (0..48_000)
//!     .flat_map(|i| {
//!         let v = 0.7 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48_000.0).sin();
//!         [v, v]
//!     })
//!     .collect();
//!
//! let n = Normalizer::new(48_000, &[Channel::Left, Channel::Right])
//!     .target_lufs(-23.0)
//!     .true_peak_ceiling_dbtp(-1.0);
//! let report = n.normalize_in_place(&mut buf)?;
//! assert!(report.applied_gain_db.is_finite());
//! # Ok::<(), ebur128_stream::Error>(())
//! ```

use crate::analyzer::AnalyzerBuilder;
use crate::channel::Channel;
use crate::error::Error;
use crate::mode::Mode;
use crate::report::Report;

/// Compute the linear amplitude gain that, when applied to the audio
/// the [`Report`] was measured from, would shift its integrated
/// loudness from `report.integrated_lufs()` to `target_lufs`.
///
/// Returns `None` if the report has no integrated value (silent
/// programme).
///
/// # Example
///
/// ```
/// # use ebur128_stream::{AnalyzerBuilder, Channel, Mode, normalize};
/// let mut a = AnalyzerBuilder::new()
///     .sample_rate(48_000)
///     .channels(&[Channel::Center])
///     .modes(Mode::Integrated)
///     .build()?;
/// let signal: Vec<f32> = (0..48_000)
///     .map(|i| 0.1 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48_000.0).sin())
///     .collect();
/// a.push_interleaved::<f32>(&signal)?;
/// let report = a.finalize();
///
/// let gain = normalize::gain_to_target(&report, -23.0).unwrap();
/// // gain is a linear multiplier; applying it would raise integrated
/// // LUFS to ≈ -23.0.
/// assert!(gain.is_finite() && gain > 0.0);
/// # Ok::<(), ebur128_stream::Error>(())
/// ```
#[must_use]
pub fn gain_to_target(report: &Report, target_lufs: f64) -> Option<f64> {
    let i = report.integrated_lufs()?;
    let delta_db = target_lufs - i;
    Some(libm::pow(10.0, delta_db / 20.0))
}

/// Result of a normalisation pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalizeReport {
    /// The measured integrated loudness *before* normalisation.
    pub measured_integrated_lufs: Option<f64>,
    /// The measured true peak *before* normalisation, in dBTP.
    pub measured_true_peak_dbtp: Option<f64>,
    /// The target loudness, in LUFS.
    pub target_lufs: f64,
    /// The true-peak ceiling in dBTP, or `None` if no ceiling was set.
    pub true_peak_ceiling_dbtp: Option<f64>,
    /// The gain that was actually applied, in dB.
    ///
    /// May differ from `target_lufs - measured_integrated_lufs` if
    /// the true-peak ceiling reduced it.
    pub applied_gain_db: f64,
    /// `true` if the gain was attenuated to honour the true-peak
    /// ceiling.
    pub limited_by_true_peak: bool,
}

/// One-shot normaliser: analyse + scale a buffer in place.
#[derive(Debug, Clone)]
pub struct Normalizer<'a> {
    sample_rate: u32,
    channels: &'a [Channel],
    target_lufs: f64,
    true_peak_ceiling_dbtp: Option<f64>,
}

impl<'a> Normalizer<'a> {
    /// Build a fresh normaliser. Default target is −23 LUFS (EBU R128
    /// broadcast reference).
    #[must_use]
    pub fn new(sample_rate: u32, channels: &'a [Channel]) -> Self {
        Self {
            sample_rate,
            channels,
            target_lufs: -23.0,
            true_peak_ceiling_dbtp: None,
        }
    }

    /// Set the target integrated loudness in LUFS.
    #[must_use]
    pub fn target_lufs(mut self, lufs: f64) -> Self {
        self.target_lufs = lufs;
        self
    }

    /// Cap the post-normalisation true peak at this dBTP value. If the
    /// gain required to reach `target_lufs` would push the true peak
    /// above the ceiling, the gain is reduced.
    #[must_use]
    pub fn true_peak_ceiling_dbtp(mut self, dbtp: f64) -> Self {
        self.true_peak_ceiling_dbtp = Some(dbtp);
        self
    }

    /// Run the analyser, compute the gain, and scale `samples` in place.
    /// `samples` is interleaved (`[L0 R0 L1 R1 ...]`).
    ///
    /// Returns the [`NormalizeReport`] describing what happened.
    ///
    /// # Errors
    ///
    /// Anything [`AnalyzerBuilder::build`] or
    /// [`crate::Analyzer::push_interleaved`] can return.
    pub fn normalize_in_place(&self, samples: &mut [f32]) -> Result<NormalizeReport, Error> {
        let mut a = AnalyzerBuilder::new()
            .sample_rate(self.sample_rate)
            .channels(self.channels)
            .modes(Mode::Integrated | Mode::TruePeak)
            .build()?;
        a.push_interleaved::<f32>(samples)?;
        let report = a.finalize();

        let measured_i = report.integrated_lufs();
        let measured_tp = report.true_peak_dbtp();

        let mut gain_db = match measured_i {
            Some(i) => self.target_lufs - i,
            None => 0.0,
        };
        let mut limited_by_true_peak = false;
        if let (Some(ceiling), Some(tp)) = (self.true_peak_ceiling_dbtp, measured_tp) {
            // After applying gain_db, the new TP would be tp + gain_db.
            // Cap so tp + gain_db <= ceiling.
            let max_gain = ceiling - tp;
            if gain_db > max_gain {
                gain_db = max_gain;
                limited_by_true_peak = true;
            }
        }
        let gain_lin = libm::pow(10.0, gain_db / 20.0) as f32;
        for s in samples.iter_mut() {
            *s *= gain_lin;
        }

        Ok(NormalizeReport {
            measured_integrated_lufs: measured_i,
            measured_true_peak_dbtp: measured_tp,
            target_lufs: self.target_lufs,
            true_peak_ceiling_dbtp: self.true_peak_ceiling_dbtp,
            applied_gain_db: gain_db,
            limited_by_true_peak,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AnalyzerBuilder;

    fn synth_stereo(amp: f32, secs: f32, fs: u32) -> Vec<f32> {
        let n = (fs as f32 * secs) as usize;
        let omega = 2.0 * std::f32::consts::PI * 1000.0 / fs as f32;
        let mut v = Vec::with_capacity(n * 2);
        for i in 0..n {
            let s = amp * (omega * i as f32).sin();
            v.push(s);
            v.push(s);
        }
        v
    }

    #[test]
    fn gain_to_target_zero_for_self() {
        // Build a -23 LUFS programme; gain to reach -23 should ≈ 1.0.
        let mut a = AnalyzerBuilder::new()
            .sample_rate(48_000)
            .channels(&[Channel::Left, Channel::Right])
            .modes(Mode::Integrated)
            .build()
            .unwrap();
        let s = synth_stereo(0.05, 5.0, 48_000); // ≈ -29 LUFS
        a.push_interleaved::<f32>(&s).unwrap();
        let r = a.finalize();
        let measured = r.integrated_lufs().unwrap();
        let g = gain_to_target(&r, measured).unwrap();
        assert!((g - 1.0).abs() < 1e-9, "gain to self = {g}");
    }

    #[test]
    fn normalise_brings_loudness_to_target() {
        let mut signal = synth_stereo(0.05, 5.0, 48_000);
        let n = Normalizer::new(48_000, &[Channel::Left, Channel::Right]).target_lufs(-23.0);
        let r = n.normalize_in_place(&mut signal).unwrap();

        // Re-measure the normalised buffer; should be near -23 LUFS.
        let mut a = AnalyzerBuilder::new()
            .sample_rate(48_000)
            .channels(&[Channel::Left, Channel::Right])
            .modes(Mode::Integrated)
            .build()
            .unwrap();
        a.push_interleaved::<f32>(&signal).unwrap();
        let after = a.finalize().integrated_lufs().unwrap();
        assert!(
            (after - (-23.0)).abs() <= 0.1,
            "after-norm I = {after}, expected ≈ -23"
        );
        assert!(r.applied_gain_db > 0.0);
    }

    #[test]
    fn true_peak_ceiling_attenuates_gain() {
        // Aim for a *loud* target with a strict TP ceiling — gain to
        // reach the target would push TP above the ceiling, so the
        // gain is capped.
        let mut signal = synth_stereo(0.05, 5.0, 48_000);
        let n = Normalizer::new(48_000, &[Channel::Left, Channel::Right])
            .target_lufs(-1.0) // very loud
            .true_peak_ceiling_dbtp(-1.0);
        let r = n.normalize_in_place(&mut signal).unwrap();
        assert!(
            r.limited_by_true_peak,
            "expected the gain to be limited by the TP ceiling"
        );
        // After applying capped gain, true peak should be ≤ -1 dBTP + ε.
        let mut a = AnalyzerBuilder::new()
            .sample_rate(48_000)
            .channels(&[Channel::Left, Channel::Right])
            .modes(Mode::TruePeak)
            .build()
            .unwrap();
        a.push_interleaved::<f32>(&signal).unwrap();
        let tp = a.finalize().true_peak_dbtp().unwrap();
        assert!(tp <= -1.0 + 0.5, "tp after cap = {tp}, expected ≤ -1 + ε");
    }
}
