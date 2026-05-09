//! Final measurements after [`Analyzer::finalize`].
//!
//! [`Report`] is what the analyzer produces when streaming is finished.
//! Compared to [`Snapshot`](crate::Snapshot), it includes programme-final
//! values like the maximum momentary / short-term LUFS observed and the
//! finalized loudness range.

/// Programme-final loudness measurements.
///
/// Returned from [`Analyzer::finalize`]. Each accessor returns
/// `Option<f64>`:
/// - `Some(value)` when the corresponding [`Mode`](crate::Mode) was
///   requested *and* enough data was ingested.
/// - `None` otherwise.
///
/// [`Analyzer::finalize`]: crate::Analyzer::finalize
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Report {
    pub(crate) integrated_lufs: Option<f64>,
    pub(crate) loudness_range_lu: Option<f64>,
    pub(crate) true_peak_dbtp: Option<f64>,
    pub(crate) momentary_max_lufs: Option<f64>,
    pub(crate) short_term_max_lufs: Option<f64>,
    pub(crate) programme_duration_seconds: f64,
}

impl Report {
    /// Integrated programme loudness in LUFS, gated per BS.1770-4 §5.6.
    #[inline]
    #[must_use]
    pub fn integrated_lufs(&self) -> Option<f64> {
        self.integrated_lufs
    }

    /// Loudness range in LU per EBU Tech 3342.
    #[inline]
    #[must_use]
    pub fn loudness_range_lu(&self) -> Option<f64> {
        self.loudness_range_lu
    }

    /// Maximum true peak in dBTP (decibels relative to full scale, true
    /// peak — i.e. measured after 4× oversampling per BS.1770 Annex 2).
    #[inline]
    #[must_use]
    pub fn true_peak_dbtp(&self) -> Option<f64> {
        self.true_peak_dbtp
    }

    /// Maximum momentary loudness observed during the programme, in LUFS.
    #[inline]
    #[must_use]
    pub fn momentary_max_lufs(&self) -> Option<f64> {
        self.momentary_max_lufs
    }

    /// Maximum short-term loudness observed during the programme, in LUFS.
    #[inline]
    #[must_use]
    pub fn short_term_max_lufs(&self) -> Option<f64> {
        self.short_term_max_lufs
    }

    /// Total programme duration ingested, in seconds.
    #[inline]
    #[must_use]
    pub fn programme_duration_seconds(&self) -> f64 {
        self.programme_duration_seconds
    }
}
