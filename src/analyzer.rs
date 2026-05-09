//! [`Analyzer`] and [`AnalyzerBuilder`] — the entry points to the crate.
//!
//! Construct an analyzer by chaining builder methods and calling
//! [`AnalyzerBuilder::build`]. Push samples with
//! [`Analyzer::push_planar`] or [`Analyzer::push_interleaved`]. Read
//! intermediate values via [`Analyzer::snapshot`] or finalize with
//! [`Analyzer::finalize`].

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::blocks::{BlockAccumulator, MOMENTARY_BLOCKS, SHORT_TERM_BLOCKS};
use crate::channel::Channel;
use crate::error::Error;
use crate::filter::KFilter;
use crate::mode::Mode;
#[cfg(feature = "alloc")]
use crate::peak::TruePeakState;
use crate::report::Report;
use crate::sample::Sample;
use crate::snapshot::Snapshot;

/// Maximum number of channels supported in a single layout.
///
/// Covers every standard audio format up to and including
/// 22.2 immersive surround. Bumped from 8 in v0.2.
pub(crate) const MAX_CHANNELS: usize = 24;

/// Acceptable sample rates (Hz). Other rates are rejected at `build()`.
///
/// Includes telephony / streaming-codec rates (22.05, 32 kHz) in
/// addition to the broadcast / production rates from BS.1770-4.
const SUPPORTED_RATES: &[u32] = &[22_050, 32_000, 44_100, 48_000, 88_200, 96_000, 192_000];

/// The K-weighting calibration offset, in dB, per ITU-R BS.1770-4 §4.1.
pub(crate) const LUFS_OFFSET: f64 = -0.691;

/// Absolute gate threshold for integrated loudness, per BS.1770-4 §5.6.
#[cfg(feature = "alloc")]
pub(crate) const ABSOLUTE_GATE_LUFS: f64 = -70.0;

/// Relative gate offset for integrated loudness, per BS.1770-4 §5.6.
#[cfg(feature = "alloc")]
pub(crate) const RELATIVE_GATE_OFFSET_LU: f64 = -10.0;

/// Relative gate offset for LRA, per EBU Tech 3342.
#[cfg(feature = "alloc")]
pub(crate) const LRA_RELATIVE_GATE_OFFSET_LU: f64 = -20.0;

/// Builder for [`Analyzer`].
///
/// # Example
///
/// ```
/// use ebur128_stream::{AnalyzerBuilder, Channel, Mode};
/// let analyzer = AnalyzerBuilder::new()
///     .sample_rate(48_000)
///     .channels(&[Channel::Left, Channel::Right])
///     .modes(Mode::Integrated | Mode::Momentary)
///     .build()
///     .unwrap();
/// # let _ = analyzer;
/// ```
#[derive(Debug, Clone)]
pub struct AnalyzerBuilder {
    sample_rate: u32,
    channels: [Channel; MAX_CHANNELS],
    n_channels: u8,
    /// `true` if the supplied layout exceeded `MAX_CHANNELS` and the
    /// builder should fail at build-time.
    overflow: bool,
    modes: Mode,
    /// Discriminates "user never called .modes()" from
    /// "user passed Mode::empty()".
    modes_set: bool,
    channels_set: bool,
    /// Optional programme-length hint, in seconds. Used to pre-reserve
    /// the integrated/LRA programme buffers so steady-state pushes
    /// don't trigger Vec growth.
    expected_duration_secs: Option<f64>,
}

impl Default for AnalyzerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalyzerBuilder {
    /// Start a fresh builder with no configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sample_rate: 48_000,
            channels: [Channel::Other; MAX_CHANNELS],
            n_channels: 0,
            overflow: false,
            modes: Mode::All,
            modes_set: false,
            channels_set: false,
            expected_duration_secs: None,
        }
    }

    /// Hint at the expected programme length. The builder uses this to
    /// pre-reserve internal buffers so steady-state `push_*` calls
    /// trigger no allocations even with [`Mode::Integrated`] /
    /// [`Mode::Lra`] enabled.
    ///
    /// Over-estimating wastes a few KB; under-estimating just causes
    /// the buffer to grow as normal. This is a hint, not a hard cap.
    ///
    /// # Example
    ///
    /// ```
    /// # use ebur128_stream::{AnalyzerBuilder, Channel, Mode};
    /// # use core::time::Duration;
    /// let analyzer = AnalyzerBuilder::new()
    ///     .sample_rate(48_000)
    ///     .channels(&[Channel::Left, Channel::Right])
    ///     .modes(Mode::Integrated | Mode::Lra)
    ///     .expected_duration(Duration::from_secs(60 * 90))   // 90 minutes
    ///     .build()?;
    /// # let _ = analyzer;
    /// # Ok::<(), ebur128_stream::Error>(())
    /// ```
    #[must_use]
    pub fn expected_duration(mut self, duration: core::time::Duration) -> Self {
        self.expected_duration_secs = Some(duration.as_secs_f64());
        self
    }

    /// Set the input sample rate, in hertz. Must be one of
    /// 22 050, 32 000, 44 100, 48 000, 88 200, 96 000, 192 000.
    #[must_use]
    pub fn sample_rate(mut self, hz: u32) -> Self {
        self.sample_rate = hz;
        self
    }

    /// Set the channel layout. The order must match the order of channel
    /// slices passed to [`Analyzer::push_planar`] or the interleave
    /// order passed to [`Analyzer::push_interleaved`].
    #[must_use]
    pub fn channels(mut self, layout: &[Channel]) -> Self {
        self.channels_set = true;
        if layout.len() > MAX_CHANNELS {
            self.overflow = true;
            return self;
        }
        self.n_channels = layout.len() as u8;
        for (i, &c) in layout.iter().enumerate() {
            self.channels[i] = c;
        }
        self
    }

    /// Set the analysis modes. At least one mode must be selected.
    #[must_use]
    pub fn modes(mut self, modes: Mode) -> Self {
        self.modes = modes;
        self.modes_set = true;
        self
    }

    /// Validate configuration and produce an [`Analyzer`].
    pub fn build(self) -> Result<Analyzer, Error> {
        if !SUPPORTED_RATES.contains(&self.sample_rate) {
            return Err(Error::InvalidSampleRate {
                hz: self.sample_rate,
            });
        }
        if self.overflow {
            return Err(Error::InvalidChannelLayout {
                got: usize::MAX,
                max: MAX_CHANNELS,
            });
        }
        if !self.channels_set || self.n_channels == 0 {
            return Err(Error::InvalidChannelLayout {
                got: 0,
                max: MAX_CHANNELS,
            });
        }
        if self.modes_set && self.modes.is_empty() {
            return Err(Error::NoModesSelected);
        }
        let modes = if self.modes_set {
            self.modes
        } else {
            Mode::All
        };

        #[cfg(not(feature = "alloc"))]
        if modes.contains(Mode::Integrated) {
            return Err(Error::IntegratedRequiresAlloc);
        }
        #[cfg(not(feature = "alloc"))]
        if modes.contains(Mode::Lra) {
            return Err(Error::LraRequiresAlloc);
        }

        Ok(Analyzer::new(
            self.sample_rate,
            self.channels,
            self.n_channels as usize,
            modes,
            self.expected_duration_secs,
        ))
    }
}

/// Streaming EBU R128 loudness analyzer.
///
/// See the [crate-level documentation](crate) for an overview of the
/// API. Build with [`AnalyzerBuilder`].
#[derive(Debug)]
pub struct Analyzer {
    sample_rate: u32,
    channels: [Channel; MAX_CHANNELS],
    n_channels: usize,
    modes: Mode,
    samples_per_block: u32,
    /// Per-channel K-weighting filter state.
    filters: [KFilter; MAX_CHANNELS],
    /// Per-channel running sum of squared K-weighted samples within the
    /// current 100 ms block.
    block_acc: BlockAccumulator,
    /// Sliding ring buffer of the last `MOMENTARY_BLOCKS` weighted-MS sums.
    momentary_ring: [f32; MOMENTARY_BLOCKS],
    momentary_filled: usize,
    momentary_idx: usize,
    /// Sliding ring buffer of the last `SHORT_TERM_BLOCKS` weighted-MS sums.
    short_term_ring: [f32; SHORT_TERM_BLOCKS],
    short_term_filled: usize,
    short_term_idx: usize,
    /// Programme buffer of per-gating-block weighted-MS values, used by
    /// integrated loudness.
    #[cfg(feature = "alloc")]
    programme_blocks: Vec<f32>,
    /// Per-100ms short-term-window MS samples, used by LRA percentile
    /// computation.
    #[cfg(feature = "alloc")]
    short_term_samples: Vec<f32>,
    /// Per-channel true-peak oversampling state.
    #[cfg(feature = "alloc")]
    true_peak: Option<TruePeakState>,
    /// Programme-wide max momentary LUFS observed so far.
    momentary_max: Option<f64>,
    /// Programme-wide max short-term LUFS observed so far.
    short_term_max: Option<f64>,
    /// Total samples ingested per channel — used for programme duration.
    samples_ingested: u64,
    /// Cached snapshot — invalidated on each `push_*` call.
    cached_snapshot: Option<Snapshot>,
}

impl Analyzer {
    fn new(
        sample_rate: u32,
        channels: [Channel; MAX_CHANNELS],
        n_channels: usize,
        modes: Mode,
        expected_duration_secs: Option<f64>,
    ) -> Self {
        #[cfg(not(feature = "alloc"))]
        let _ = expected_duration_secs;
        let samples_per_block = sample_rate / 10;
        // Build per-channel filters. KFilter has the configured
        // sample-rate-specific biquad coefficients baked in.
        // KFilter is Copy so we can populate the fixed-size array via
        // a single template, regardless of MAX_CHANNELS.
        let template = KFilter::new(sample_rate);
        let mut filters: [KFilter; MAX_CHANNELS] = [template; MAX_CHANNELS];
        for f in filters.iter_mut().take(n_channels) {
            f.reset();
        }
        let block_acc = BlockAccumulator::new(n_channels, samples_per_block);

        // Pre-reserve programme buffers from the expected_duration hint.
        // 10 blocks/s for both buffers; cap to a sane upper bound to
        // protect against malicious huge hints.
        #[cfg(feature = "alloc")]
        let reserve_blocks: usize = expected_duration_secs
            .map(|s| (s.max(0.0) * 10.0).min(48.0 * 3_600.0 * 10.0) as usize)
            .unwrap_or(0);

        Self {
            sample_rate,
            channels,
            n_channels,
            modes,
            samples_per_block,
            filters,
            block_acc,
            momentary_ring: [0.0; MOMENTARY_BLOCKS],
            momentary_filled: 0,
            momentary_idx: 0,
            short_term_ring: [0.0; SHORT_TERM_BLOCKS],
            short_term_filled: 0,
            short_term_idx: 0,
            #[cfg(feature = "alloc")]
            programme_blocks: if modes.contains(Mode::Integrated) {
                Vec::with_capacity(reserve_blocks)
            } else {
                Vec::new()
            },
            #[cfg(feature = "alloc")]
            short_term_samples: if modes.contains(Mode::Lra) {
                Vec::with_capacity(reserve_blocks)
            } else {
                Vec::new()
            },
            #[cfg(feature = "alloc")]
            true_peak: if modes.contains(Mode::TruePeak) {
                Some(TruePeakState::new(n_channels, sample_rate))
            } else {
                None
            },
            momentary_max: None,
            short_term_max: None,
            samples_ingested: 0,
            cached_snapshot: None,
        }
    }

    /// Configured sample rate, in hertz.
    #[inline]
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Configured channel layout.
    #[inline]
    #[must_use]
    pub fn channels(&self) -> &[Channel] {
        &self.channels[..self.n_channels]
    }

    /// Configured modes.
    #[inline]
    #[must_use]
    pub fn modes(&self) -> Mode {
        self.modes
    }

    /// Number of samples in one 100 ms block at the configured rate.
    #[inline]
    #[must_use]
    pub fn samples_per_block(&self) -> u32 {
        self.samples_per_block
    }

    /// Push samples laid out as one slice per channel, in the same order
    /// as the configured channel layout.
    ///
    /// All slices must have the same length.
    ///
    /// # Errors
    ///
    /// - [`Error::ChannelMismatch`] if `channels.len()` differs from the
    ///   configured layout length.
    /// - [`Error::PlanarLengthMismatch`] if slices have different lengths.
    /// - [`Error::NonFiniteSample`] if any sample is `NaN` or infinity.
    ///
    /// # Example
    ///
    /// ```
    /// use ebur128_stream::{AnalyzerBuilder, Channel, Mode};
    ///
    /// let mut analyzer = AnalyzerBuilder::new()
    ///     .sample_rate(48_000)
    ///     .channels(&[Channel::Left, Channel::Right])
    ///     .modes(Mode::Momentary)
    ///     .build()?;
    ///
    /// let left  = vec![0.0_f32; 9_600]; // 100 ms
    /// let right = vec![0.0_f32; 9_600];
    /// analyzer.push_planar::<f32>(&[&left, &right])?;
    /// # Ok::<(), ebur128_stream::Error>(())
    /// ```
    pub fn push_planar<S: Sample>(&mut self, channels: &[&[S]]) -> Result<(), Error> {
        if channels.len() != self.n_channels {
            return Err(Error::ChannelMismatch {
                expected: self.n_channels,
                got: channels.len(),
            });
        }
        if channels.is_empty() {
            return Ok(());
        }
        let frames = channels[0].len();
        for ch in &channels[1..] {
            if ch.len() != frames {
                return Err(Error::PlanarLengthMismatch {
                    first: frames,
                    got: ch.len(),
                });
            }
        }
        self.cached_snapshot = None;
        let n_ch = self.n_channels;
        for i in 0..frames {
            let mut frame: [f32; MAX_CHANNELS] = [0.0; MAX_CHANNELS];
            for (ch_idx, slice) in channels.iter().enumerate() {
                let v = slice[i].to_f32();
                if !v.is_finite() {
                    return Err(Error::NonFiniteSample);
                }
                frame[ch_idx] = v;
            }
            self.process_frame(&frame[..n_ch]);
        }
        self.samples_ingested = self.samples_ingested.saturating_add(frames as u64);
        Ok(())
    }

    /// Push samples laid out as interleaved frames
    /// (`[L0, R0, L1, R1, ...]`).
    ///
    /// `samples.len()` must be a whole multiple of the channel count.
    ///
    /// # Errors
    ///
    /// - [`Error::InterleavedLengthNotMultiple`] if the buffer length
    ///   isn't divisible by the channel count.
    /// - [`Error::NonFiniteSample`] if any sample is `NaN` or infinity.
    ///
    /// # Example
    ///
    /// ```
    /// use ebur128_stream::{AnalyzerBuilder, Channel, Mode};
    ///
    /// let mut analyzer = AnalyzerBuilder::new()
    ///     .sample_rate(48_000)
    ///     .channels(&[Channel::Left, Channel::Right])
    ///     .modes(Mode::Momentary)
    ///     .build()?;
    ///
    /// // 100 ms of stereo silence (9 600 frames × 2 channels).
    /// let stereo = vec![0.0_f32; 9_600 * 2];
    /// analyzer.push_interleaved::<f32>(&stereo)?;
    /// # Ok::<(), ebur128_stream::Error>(())
    /// ```
    pub fn push_interleaved<S: Sample>(&mut self, samples: &[S]) -> Result<(), Error> {
        let n_ch = self.n_channels;
        if n_ch == 0 {
            return Ok(());
        }
        if samples.len() % n_ch != 0 {
            return Err(Error::InterleavedLengthNotMultiple {
                samples: samples.len(),
                channels: n_ch,
            });
        }
        self.cached_snapshot = None;
        let frames = samples.len() / n_ch;
        for f in 0..frames {
            let mut frame: [f32; MAX_CHANNELS] = [0.0; MAX_CHANNELS];
            for ch in 0..n_ch {
                let v = samples[f * n_ch + ch].to_f32();
                if !v.is_finite() {
                    return Err(Error::NonFiniteSample);
                }
                frame[ch] = v;
            }
            self.process_frame(&frame[..n_ch]);
        }
        self.samples_ingested = self.samples_ingested.saturating_add(frames as u64);
        Ok(())
    }

    /// Process a single multi-channel frame.
    #[inline]
    fn process_frame(&mut self, frame: &[f32]) {
        // 1) feed true-peak FIR with raw (unfiltered) samples
        #[cfg(feature = "alloc")]
        if let Some(tp) = self.true_peak.as_mut() {
            tp.feed_frame(frame);
        }
        // 2) K-weight each channel
        let mut weighted: [f32; MAX_CHANNELS] = [0.0; MAX_CHANNELS];
        for (ch, &s) in frame.iter().enumerate() {
            weighted[ch] = self.filters[ch].process(s);
        }
        // 3) accumulate squared values into the 100 ms block
        let block_complete = self.block_acc.push_frame(&weighted[..frame.len()]);
        if block_complete {
            let block_ms = self.block_acc.take_block();
            self.on_block_emitted(&block_ms);
        }
    }

    /// Called whenever the 100 ms block aggregator fires.
    fn on_block_emitted(&mut self, per_channel_ms: &[f32]) {
        // Channel-weighted sum: Σ_ch (w_ch * MS_ch).
        let mut weighted_sum: f32 = 0.0;
        for (i, &ms) in per_channel_ms.iter().enumerate() {
            weighted_sum += self.channels[i].weight() * ms;
        }

        // ---- Momentary (4 blocks = 400 ms) ----
        self.momentary_ring[self.momentary_idx] = weighted_sum;
        self.momentary_idx = (self.momentary_idx + 1) % MOMENTARY_BLOCKS;
        if self.momentary_filled < MOMENTARY_BLOCKS {
            self.momentary_filled += 1;
        }
        if self.momentary_filled == MOMENTARY_BLOCKS {
            let mean = self.momentary_ring.iter().sum::<f32>() / MOMENTARY_BLOCKS as f32;
            if let Some(lufs) = ms_to_lufs(mean) {
                self.momentary_max = Some(self.momentary_max.map_or(lufs, |m| m.max(lufs)));
            }
            // Programme buffer captures the same 400 ms gating block.
            #[cfg(feature = "alloc")]
            if self.modes.contains(Mode::Integrated) {
                self.programme_blocks.push(mean);
            }
        }

        // ---- Short-term (30 blocks = 3 s) ----
        self.short_term_ring[self.short_term_idx] = weighted_sum;
        self.short_term_idx = (self.short_term_idx + 1) % SHORT_TERM_BLOCKS;
        if self.short_term_filled < SHORT_TERM_BLOCKS {
            self.short_term_filled += 1;
        }
        if self.short_term_filled == SHORT_TERM_BLOCKS {
            let mean = self.short_term_ring.iter().sum::<f32>() / SHORT_TERM_BLOCKS as f32;
            if let Some(lufs) = ms_to_lufs(mean) {
                self.short_term_max = Some(self.short_term_max.map_or(lufs, |m| m.max(lufs)));
            }
            #[cfg(feature = "alloc")]
            if self.modes.contains(Mode::Lra) {
                self.short_term_samples.push(mean);
            }
        }
    }

    /// Take a snapshot of the current measurements.
    ///
    /// Cheap to call repeatedly; momentary and short-term values are
    /// `O(1)`. Integrated and LRA values are `O(programme blocks)` but
    /// cached between pushes.
    ///
    /// # Example
    ///
    /// ```
    /// use ebur128_stream::{AnalyzerBuilder, Channel, Mode};
    ///
    /// let mut analyzer = AnalyzerBuilder::new()
    ///     .sample_rate(48_000)
    ///     .channels(&[Channel::Center])
    ///     .modes(Mode::Momentary)
    ///     .build()?;
    ///
    /// analyzer.push_interleaved::<f32>(&vec![0.1; 48_000])?;  // 1 s
    /// let s = analyzer.snapshot();
    /// // Momentary fills after 400 ms; with 1 s pushed it's available.
    /// assert!(s.momentary_lufs().is_some());
    /// # Ok::<(), ebur128_stream::Error>(())
    /// ```
    pub fn snapshot(&mut self) -> Snapshot {
        if let Some(cached) = self.cached_snapshot {
            return cached;
        }

        let momentary_lufs =
            if self.modes.contains(Mode::Momentary) && self.momentary_filled == MOMENTARY_BLOCKS {
                let mean = self.momentary_ring.iter().sum::<f32>() / MOMENTARY_BLOCKS as f32;
                ms_to_lufs(mean)
            } else {
                None
            };

        let short_term_lufs = if self.modes.contains(Mode::ShortTerm)
            && self.short_term_filled == SHORT_TERM_BLOCKS
        {
            let mean = self.short_term_ring.iter().sum::<f32>() / SHORT_TERM_BLOCKS as f32;
            ms_to_lufs(mean)
        } else {
            None
        };

        #[cfg(feature = "alloc")]
        let integrated_lufs = if self.modes.contains(Mode::Integrated) {
            crate::gating::compute_integrated(&self.programme_blocks)
        } else {
            None
        };
        #[cfg(not(feature = "alloc"))]
        let integrated_lufs: Option<f64> = None;

        #[cfg(feature = "alloc")]
        let true_peak_dbtp = self.true_peak.as_ref().and_then(|tp| tp.peak_dbtp());
        #[cfg(not(feature = "alloc"))]
        let true_peak_dbtp: Option<f64> = None;

        #[cfg(feature = "alloc")]
        let loudness_range_lu = if self.modes.contains(Mode::Lra) {
            crate::lra::compute_lra(&self.short_term_samples)
        } else {
            None
        };
        #[cfg(not(feature = "alloc"))]
        let loudness_range_lu: Option<f64> = None;

        let programme_duration_seconds = self.samples_ingested as f64 / self.sample_rate as f64;

        let snap = Snapshot {
            momentary_lufs,
            short_term_lufs,
            integrated_lufs,
            true_peak_dbtp,
            loudness_range_lu,
            programme_duration_seconds,
        };
        self.cached_snapshot = Some(snap);
        snap
    }

    /// Consume the analyzer and return the final [`Report`].
    ///
    /// # Example
    ///
    /// ```
    /// use ebur128_stream::{AnalyzerBuilder, Channel, Mode};
    ///
    /// let mut a = AnalyzerBuilder::new()
    ///     .sample_rate(48_000)
    ///     .channels(&[Channel::Center])
    ///     .modes(Mode::Integrated)
    ///     .build()?;
    /// a.push_interleaved::<f32>(&vec![0.0; 48_000])?;
    /// let report = a.finalize();
    /// // Silent input clears no gating block — integrated is None.
    /// assert!(report.integrated_lufs().is_none());
    /// # Ok::<(), ebur128_stream::Error>(())
    /// ```
    pub fn finalize(mut self) -> Report {
        let snap = self.snapshot();
        Report {
            integrated_lufs: snap.integrated_lufs,
            loudness_range_lu: snap.loudness_range_lu,
            true_peak_dbtp: snap.true_peak_dbtp,
            momentary_max_lufs: self.momentary_max,
            short_term_max_lufs: self.short_term_max,
            programme_duration_seconds: snap.programme_duration_seconds,
        }
    }

    /// Reset all internal state, retaining configuration.
    ///
    /// Use this when reusing one analyzer across multiple programmes
    /// (e.g. between songs in a playback application).
    ///
    /// # Example
    ///
    /// ```
    /// use ebur128_stream::{AnalyzerBuilder, Channel, Mode};
    ///
    /// let mut a = AnalyzerBuilder::new()
    ///     .sample_rate(48_000)
    ///     .channels(&[Channel::Center])
    ///     .modes(Mode::Momentary)
    ///     .build()?;
    /// a.push_interleaved::<f32>(&vec![0.1; 48_000])?;
    /// a.reset();
    /// assert_eq!(a.snapshot().programme_duration_seconds(), 0.0);
    /// # Ok::<(), ebur128_stream::Error>(())
    /// ```
    pub fn reset(&mut self) {
        for f in self.filters.iter_mut().take(self.n_channels) {
            f.reset();
        }
        self.block_acc.reset();
        self.momentary_ring = [0.0; MOMENTARY_BLOCKS];
        self.momentary_filled = 0;
        self.momentary_idx = 0;
        self.short_term_ring = [0.0; SHORT_TERM_BLOCKS];
        self.short_term_filled = 0;
        self.short_term_idx = 0;
        #[cfg(feature = "alloc")]
        {
            self.programme_blocks.clear();
            self.short_term_samples.clear();
            if let Some(tp) = self.true_peak.as_mut() {
                tp.reset();
            }
        }
        self.momentary_max = None;
        self.short_term_max = None;
        self.samples_ingested = 0;
        self.cached_snapshot = None;
    }
}

/// Convert a mean-square value to LUFS via the BS.1770-4 calibration.
/// Returns `None` for zero / negative MS (silence).
#[inline]
pub(crate) fn ms_to_lufs(ms: f32) -> Option<f64> {
    if ms <= 0.0 {
        None
    } else {
        Some(LUFS_OFFSET + 10.0 * libm::log10(ms as f64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "alloc")]
    use alloc::vec;

    #[test]
    fn smoke_builder_rejects_bad_sample_rate() {
        let err = AnalyzerBuilder::new()
            .sample_rate(44_101)
            .channels(&[Channel::Left, Channel::Right])
            .modes(Mode::Integrated | Mode::Momentary)
            .build()
            .unwrap_err();
        assert!(matches!(err, Error::InvalidSampleRate { hz: 44_101 }));
    }

    #[test]
    fn smoke_builder_accepts_supported_rate() {
        let a = AnalyzerBuilder::new()
            .sample_rate(48_000)
            .channels(&[Channel::Left, Channel::Right])
            .modes(Mode::Momentary)
            .build()
            .unwrap();
        assert_eq!(a.sample_rate(), 48_000);
        assert_eq!(a.channels().len(), 2);
        assert_eq!(a.samples_per_block(), 4_800);
    }

    #[test]
    fn smoke_builder_rejects_empty_channels() {
        let err = AnalyzerBuilder::new()
            .sample_rate(48_000)
            .channels(&[])
            .modes(Mode::Momentary)
            .build()
            .unwrap_err();
        assert!(matches!(err, Error::InvalidChannelLayout { got: 0, .. }));
    }

    #[test]
    fn smoke_builder_rejects_no_modes() {
        let err = AnalyzerBuilder::new()
            .sample_rate(48_000)
            .channels(&[Channel::Left])
            .modes(Mode::empty())
            .build()
            .unwrap_err();
        assert!(matches!(err, Error::NoModesSelected));
    }

    #[test]
    fn smoke_push_interleaved_validates_length() {
        let mut a = AnalyzerBuilder::new()
            .sample_rate(48_000)
            .channels(&[Channel::Left, Channel::Right])
            .modes(Mode::Momentary)
            .build()
            .unwrap();
        let err = a.push_interleaved::<f32>(&[0.0, 0.0, 0.0]).unwrap_err();
        assert!(matches!(
            err,
            Error::InterleavedLengthNotMultiple {
                samples: 3,
                channels: 2
            }
        ));
    }

    #[test]
    fn smoke_push_planar_validates_channel_count() {
        let mut a = AnalyzerBuilder::new()
            .sample_rate(48_000)
            .channels(&[Channel::Left, Channel::Right])
            .modes(Mode::Momentary)
            .build()
            .unwrap();
        let mono: &[f32] = &[0.0; 100];
        let err = a.push_planar::<f32>(&[mono]).unwrap_err();
        assert!(matches!(
            err,
            Error::ChannelMismatch {
                expected: 2,
                got: 1
            }
        ));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn smoke_reset_clears_state() {
        let mut a = AnalyzerBuilder::new()
            .sample_rate(48_000)
            .channels(&[Channel::Left])
            .modes(Mode::Momentary)
            .build()
            .unwrap();
        let buf: Vec<f32> = vec![0.5; 4_800 * 5];
        a.push_interleaved::<f32>(&buf).unwrap();
        a.reset();
        let snap = a.snapshot();
        assert_eq!(snap.programme_duration_seconds(), 0.0);
        assert!(snap.momentary_lufs().is_none());
    }

    #[test]
    fn smoke_too_many_channels_rejected() {
        // Above MAX_CHANNELS (24).
        let many = [Channel::Other; 32];
        let err = AnalyzerBuilder::new()
            .sample_rate(48_000)
            .channels(&many)
            .modes(Mode::Momentary)
            .build()
            .unwrap_err();
        assert!(matches!(err, Error::InvalidChannelLayout { .. }));
    }

    #[test]
    fn supports_22_2_immersive_24_channels() {
        // 22.2: 24-channel layout (22 main + LFE1 + LFE2).
        let layout = [Channel::Other; 24];
        let mut a = AnalyzerBuilder::new()
            .sample_rate(48_000)
            .channels(&layout)
            .modes(Mode::Momentary)
            .build()
            .unwrap();
        let buf = vec![0.05f32; 4_800 * 24]; // 100 ms × 24 channels
        a.push_interleaved::<f32>(&buf).unwrap();
        assert_eq!(a.channels().len(), 24);
    }
}
