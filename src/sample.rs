//! The [`Sample`] trait — sealed over `f32` (and `f64` under the `f64`
//! feature).
//!
//! Internally the analyzer always works in `f32` (per `SPEC.md` §5: the
//! K-weighting filter's accuracy delta between `f32` and `f64` is below 0.001
//! LU, well under the 0.1 LU EBU R128 spec tolerance). The `f64` feature
//! lets callers push `f64` buffers without manually casting; values are
//! converted to `f32` on ingest.

mod sealed {
    pub trait Sealed {}
    impl Sealed for f32 {}
    #[cfg(feature = "f64")]
    impl Sealed for f64 {}
}

/// A sample type accepted by [`Analyzer::push_planar`] and
/// [`Analyzer::push_interleaved`].
///
/// Sealed: only `f32` (and `f64` under the `f64` feature) implement this
/// trait. Adding more sample types is a deliberate API change.
///
/// [`Analyzer::push_planar`]: crate::Analyzer::push_planar
/// [`Analyzer::push_interleaved`]: crate::Analyzer::push_interleaved
pub trait Sample: sealed::Sealed + Copy {
    /// Convert the sample to the analyzer's internal `f32` representation.
    fn to_f32(self) -> f32;
}

impl Sample for f32 {
    #[inline]
    fn to_f32(self) -> f32 {
        self
    }
}

#[cfg(feature = "f64")]
impl Sample for f64 {
    #[inline]
    fn to_f32(self) -> f32 {
        self as f32
    }
}
