//! Per-call view of the analyzer's current measurements.
//!
//! [`Snapshot`] is cheap to read (`O(1)` for momentary / short-term —
//! values are cached on each new block; `O(programme blocks)` for
//! integrated — see [`Snapshot::integrated_lufs`]).

/// A point-in-time view of the analyzer's measurements.
///
/// Returned from [`Analyzer::snapshot`].
///
/// All accessors return `Option<f64>`:
/// - `Some(value)` when the requested mode was selected at build time
///   *and* enough audio has been ingested for that measurement.
/// - `None` when the mode wasn't selected, or there isn't enough data
///   yet (for example, less than 400 ms of audio has been pushed for
///   momentary loudness).
///
/// [`Analyzer::snapshot`]: crate::Analyzer::snapshot
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Snapshot {
    pub(crate) momentary_lufs: Option<f64>,
    pub(crate) short_term_lufs: Option<f64>,
    pub(crate) integrated_lufs: Option<f64>,
    pub(crate) true_peak_dbtp: Option<f64>,
    pub(crate) loudness_range_lu: Option<f64>,
    pub(crate) programme_duration_seconds: f64,
}

impl Snapshot {
    /// Momentary loudness (sliding 400 ms window) in LUFS.
    #[inline]
    #[must_use]
    pub fn momentary_lufs(&self) -> Option<f64> {
        self.momentary_lufs
    }

    /// Short-term loudness (sliding 3 s window) in LUFS.
    #[inline]
    #[must_use]
    pub fn short_term_lufs(&self) -> Option<f64> {
        self.short_term_lufs
    }

    /// Integrated (gated) loudness over the audio pushed so far, in LUFS.
    ///
    /// Computing this involves a pass over the programme buffer; the
    /// analyzer caches the result and invalidates the cache when new
    /// audio is pushed, so polling at \~10 Hz from a real-time monitor
    /// is safe.
    #[inline]
    #[must_use]
    pub fn integrated_lufs(&self) -> Option<f64> {
        self.integrated_lufs
    }

    /// Maximum true-peak observed so far, in dBTP.
    #[inline]
    #[must_use]
    pub fn true_peak_dbtp(&self) -> Option<f64> {
        self.true_peak_dbtp
    }

    /// Loudness range (95 % − 10 % of doubly-gated short-term values), in LU.
    #[inline]
    #[must_use]
    pub fn loudness_range_lu(&self) -> Option<f64> {
        self.loudness_range_lu
    }

    /// Total duration of audio ingested, in seconds.
    #[inline]
    #[must_use]
    pub fn programme_duration_seconds(&self) -> f64 {
        self.programme_duration_seconds
    }
}
