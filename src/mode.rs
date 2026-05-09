//! The [`Mode`] bitflags — which measurements the analyzer should compute.
//!
//! Modes are composable: `Mode::Integrated | Mode::TruePeak` builds an
//! analyzer that maintains both gating state and an oversampling FIR.
//! Each mode adds work on the hot path; only request what the caller will
//! read.

use bitflags::bitflags;

bitflags! {
    /// The set of measurements an analyzer should compute.
    ///
    /// Combine with `|`:
    ///
    /// ```
    /// use ebur128_stream::Mode;
    /// let modes = Mode::Integrated | Mode::TruePeak;
    /// assert!(modes.contains(Mode::Integrated));
    /// ```
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct Mode: u8 {
        /// Integrated loudness over the full programme, gated per
        /// BS.1770-4 §5.6. Reported via [`Report::integrated_lufs`].
        ///
        /// [`Report::integrated_lufs`]: crate::Report::integrated_lufs
        const Integrated = 0b0000_0001;
        /// Momentary loudness over a sliding 400 ms window. Reported via
        /// [`Snapshot::momentary_lufs`] and
        /// [`Report::momentary_max_lufs`].
        ///
        /// [`Snapshot::momentary_lufs`]: crate::Snapshot::momentary_lufs
        /// [`Report::momentary_max_lufs`]: crate::Report::momentary_max_lufs
        const Momentary = 0b0000_0010;
        /// Short-term loudness over a sliding 3 s window. Reported via
        /// [`Snapshot::short_term_lufs`] and
        /// [`Report::short_term_max_lufs`].
        ///
        /// [`Snapshot::short_term_lufs`]: crate::Snapshot::short_term_lufs
        /// [`Report::short_term_max_lufs`]: crate::Report::short_term_max_lufs
        const ShortTerm = 0b0000_0100;
        /// True-peak via 4× oversampling FIR per BS.1770 Annex 2.
        /// Reported via [`Report::true_peak_dbtp`].
        ///
        /// [`Report::true_peak_dbtp`]: crate::Report::true_peak_dbtp
        const TruePeak = 0b0000_1000;
        /// Loudness range per EBU Tech 3342. Reported via
        /// [`Report::loudness_range_lu`].
        ///
        /// [`Report::loudness_range_lu`]: crate::Report::loudness_range_lu
        const Lra = 0b0001_0000;
        /// Convenience: all measurement modes.
        const All = 0b0001_1111;
    }
}

impl Default for Mode {
    fn default() -> Self {
        Mode::All
    }
}
