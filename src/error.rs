//! Error type for the crate.
//!
//! A single global enum carries every failure mode (per `SPEC.md` §15). New
//! variants may be added without a major version bump; the enum is
//! `#[non_exhaustive]`.

use core::fmt;

/// Errors returned from analyzer construction or sample ingestion.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Error {
    /// The requested sample rate is not one of the supported rates
    /// (22 050, 32 000, 44 100, 48 000, 88 200, 96 000, 192 000 Hz).
    InvalidSampleRate {
        /// The unsupported rate that was supplied.
        hz: u32,
    },
    /// The number of channels in a `push` call did not match the layout
    /// the analyzer was built with.
    ChannelMismatch {
        /// Channels expected (from the configured layout).
        expected: usize,
        /// Channels actually supplied.
        got: usize,
    },
    /// The configured channel layout was empty or exceeded the supported
    /// maximum.
    InvalidChannelLayout {
        /// The number of channels supplied.
        got: usize,
        /// The maximum supported (currently 8).
        max: usize,
    },
    /// `AnalyzerBuilder::build` was called without selecting any [`Mode`].
    ///
    /// At least one mode must be requested.
    ///
    /// [`Mode`]: crate::Mode
    NoModesSelected,
    /// `push_interleaved` was called with a buffer whose length is not a
    /// whole multiple of the channel count.
    InterleavedLengthNotMultiple {
        /// Number of samples actually supplied.
        samples: usize,
        /// Configured channel count.
        channels: usize,
    },
    /// `push_planar` was called with channel slices of differing length.
    PlanarLengthMismatch {
        /// Length of the first slice supplied.
        first: usize,
        /// Length of the offending later slice.
        got: usize,
    },
    /// Loudness Range (`Mode::Lra`) requires the `alloc` feature, which is
    /// not enabled in this build.
    LraRequiresAlloc,
    /// Integrated loudness (`Mode::Integrated`) requires the `alloc`
    /// feature, which is not enabled in this build.
    IntegratedRequiresAlloc,
    /// A sample value was non-finite (NaN or infinity). The analyzer
    /// rejects these rather than silently propagating; sanitize at the
    /// caller.
    NonFiniteSample,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidSampleRate { hz } => write!(
                f,
                "invalid sample rate {hz} Hz; supported: 22050, 32000, 44100, 48000, 88200, 96000, 192000"
            ),
            Error::ChannelMismatch { expected, got } => {
                write!(f, "channel count mismatch: expected {expected}, got {got}")
            }
            Error::InvalidChannelLayout { got, max } => {
                write!(f, "invalid channel layout: {got} channels (max {max})")
            }
            Error::NoModesSelected => write!(f, "no analysis modes selected"),
            Error::InterleavedLengthNotMultiple { samples, channels } => write!(
                f,
                "interleaved buffer length {samples} is not a multiple of channel count {channels}"
            ),
            Error::PlanarLengthMismatch { first, got } => write!(
                f,
                "planar channel slices have differing lengths: first {first}, got {got}"
            ),
            Error::LraRequiresAlloc => {
                write!(f, "Mode::Lra requires the `alloc` feature")
            }
            Error::IntegratedRequiresAlloc => {
                write!(f, "Mode::Integrated requires the `alloc` feature")
            }
            Error::NonFiniteSample => write!(f, "sample buffer contained NaN or infinity"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
