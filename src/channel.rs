//! Channel layout and per-channel weighting.
//!
//! Per ITU-R BS.1770-4 §4.1, the loudness sum applies these per-channel
//! weights:
//!
//! | Channel | Weight (linear) | dB |
//! |---|---|---|
//! | Left, Right, Center | 1.0 | 0 |
//! | LeftSurround, RightSurround | 1.41 | +1.5 |
//! | Lfe | 0.0 | (excluded) |
//!
//! [`Channel::Other`] is treated as unweighted (1.0); document the choice
//! at the call site if it matters for your layout.

/// One channel of a multi-channel programme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum Channel {
    /// Front left.
    Left,
    /// Front right.
    Right,
    /// Front center.
    Center,
    /// Left surround.
    LeftSurround,
    /// Right surround.
    RightSurround,
    /// Low-frequency effects channel; excluded from loudness sum per
    /// BS.1770-4.
    Lfe,
    /// Any other channel; treated as unweighted (1.0).
    Other,
}

impl Channel {
    /// The channel's contribution weight in the K-weighted sum, per
    /// BS.1770-4 §4.1. Returns `0.0` for [`Channel::Lfe`].
    ///
    /// # Example
    ///
    /// ```
    /// use ebur128_stream::Channel;
    /// assert_eq!(Channel::Center.weight(), 1.0);
    /// assert_eq!(Channel::LeftSurround.weight(), 1.41);
    /// assert_eq!(Channel::Lfe.weight(), 0.0);  // excluded
    /// ```
    #[inline]
    #[must_use]
    pub const fn weight(self) -> f32 {
        match self {
            Channel::Left | Channel::Right | Channel::Center | Channel::Other => 1.0,
            Channel::LeftSurround | Channel::RightSurround => 1.41,
            Channel::Lfe => 0.0,
        }
    }
}
