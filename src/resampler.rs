//! Sample-rate conversion to one of the analyzer's supported rates.
//!
//! The analyzer rejects sample rates that aren't in the supported set
//! (`{22 050, 32 000, 44 100, 48 000, 88 200, 96 000, 192 000}`) — but
//! real-world audio comes at all kinds of rates (8 kHz telephony,
//! 11 025 Hz legacy game audio, 11 .025 → 96 → 48 chained pipelines).
//!
//! This module provides a thin [`Resampler`] wrapper around the
//! [`rubato`](https://crates.io/crates/rubato) crate so callers can
//! adapt arbitrary input rates without re-deriving the polyphase math.
//! Linear interpolation (cheap, lossy) and high-quality sinc paths are
//! both available.
//!
//! Gated behind the `resampler` feature.
//!
//! # Example
//!
//! ```ignore
//! use ebur128_stream::resampler::{Resampler, Quality};
//! use ebur128_stream::{AnalyzerBuilder, Channel, Mode};
//!
//! // Inbound stream at 11 025 Hz; analyzer wants 48 000 Hz.
//! let mut resampler = Resampler::new(11_025, 48_000, 2, Quality::HighQuality)?;
//! let mut analyzer = AnalyzerBuilder::new()
//!     .sample_rate(48_000)
//!     .channels(&[Channel::Left, Channel::Right])
//!     .modes(Mode::Integrated)
//!     .build()?;
//!
//! // Push input → resample → analyze.
//! let input: Vec<f32> = vec![0.1; 11_025 * 2]; // 1 s stereo
//! let output = resampler.process_interleaved(&input)?;
//! analyzer.push_interleaved::<f32>(&output)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![allow(clippy::needless_range_loop)]

use alloc::vec::Vec;

/// Resampling quality preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    /// Linear interpolation. Cheap; introduces aliasing. Acceptable
    /// for development / low-latency previews; not for measurement
    /// publishing.
    Linear,
    /// Sinc interpolation with a moderate-length filter. The sweet
    /// spot for streaming pipelines.
    HighQuality,
}

/// Errors produced by the resampler.
#[derive(Debug)]
#[non_exhaustive]
pub enum ResampleError {
    /// `rubato` reported a conversion error.
    Rubato(rubato::ResampleError),
    /// Construction of the resampler failed.
    Construction(rubato::ResamplerConstructionError),
    /// The input buffer was not a whole multiple of the channel count.
    UnalignedBuffer {
        /// Number of samples in the offending buffer.
        samples: usize,
        /// Configured channel count.
        channels: usize,
    },
}

impl core::fmt::Display for ResampleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ResampleError::Rubato(e) => write!(f, "rubato: {e}"),
            ResampleError::Construction(e) => write!(f, "rubato construction: {e}"),
            ResampleError::UnalignedBuffer { samples, channels } => write!(
                f,
                "buffer length {samples} is not a multiple of channel count {channels}"
            ),
        }
    }
}

impl std::error::Error for ResampleError {}

impl From<rubato::ResampleError> for ResampleError {
    fn from(e: rubato::ResampleError) -> Self {
        ResampleError::Rubato(e)
    }
}
impl From<rubato::ResamplerConstructionError> for ResampleError {
    fn from(e: rubato::ResamplerConstructionError) -> Self {
        ResampleError::Construction(e)
    }
}

/// Internal dispatch: rubato's `Resampler` trait is not dyn-compatible
/// (it has generic methods), so we hold the concrete impls in an enum.
enum Inner {
    Linear(rubato::FastFixedIn<f32>),
    Sinc(rubato::SincFixedIn<f32>),
}

impl Inner {
    fn input_frames_next(&self) -> usize {
        use rubato::Resampler;
        match self {
            Inner::Linear(r) => r.input_frames_next(),
            Inner::Sinc(r) => r.input_frames_next(),
        }
    }
    fn output_frames_next(&self) -> usize {
        use rubato::Resampler;
        match self {
            Inner::Linear(r) => r.output_frames_next(),
            Inner::Sinc(r) => r.output_frames_next(),
        }
    }
    fn process_into_buffer(
        &mut self,
        input: &[&[f32]],
        output: &mut [&mut [f32]],
    ) -> Result<(usize, usize), rubato::ResampleError> {
        use rubato::Resampler;
        match self {
            Inner::Linear(r) => r.process_into_buffer(input, output, None),
            Inner::Sinc(r) => r.process_into_buffer(input, output, None),
        }
    }
}

/// Streaming resampler that converts interleaved audio between two
/// rates.
pub struct Resampler {
    inner: Inner,
    in_rate: u32,
    out_rate: u32,
    channels: usize,
    /// Buffer carry: input samples not yet consumed by the inner
    /// resampler (rubato wants exact chunk sizes).
    pending: Vec<Vec<f32>>,
}

impl core::fmt::Debug for Resampler {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Resampler")
            .field("in_rate", &self.in_rate)
            .field("out_rate", &self.out_rate)
            .field("channels", &self.channels)
            .finish_non_exhaustive()
    }
}

impl Resampler {
    /// Build a resampler converting `in_rate` → `out_rate` for the
    /// given channel count.
    ///
    /// # Errors
    ///
    /// Returns [`ResampleError::Construction`] if rubato can't be
    /// constructed (extreme rate ratios, zero channels, etc.).
    pub fn new(
        in_rate: u32,
        out_rate: u32,
        channels: usize,
        quality: Quality,
    ) -> Result<Self, ResampleError> {
        let ratio = out_rate as f64 / in_rate as f64;
        let chunk_size = 1024;
        let inner = match quality {
            Quality::Linear => {
                use rubato::{FastFixedIn, PolynomialDegree};
                Inner::Linear(FastFixedIn::new(
                    ratio,
                    1.0,
                    PolynomialDegree::Linear,
                    chunk_size,
                    channels,
                )?)
            }
            Quality::HighQuality => {
                use rubato::{
                    SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
                };
                let params = SincInterpolationParameters {
                    sinc_len: 128,
                    f_cutoff: 0.95,
                    interpolation: SincInterpolationType::Quadratic,
                    oversampling_factor: 256,
                    window: WindowFunction::Blackman2,
                };
                Inner::Sinc(SincFixedIn::new(ratio, 1.0, params, chunk_size, channels)?)
            }
        };
        Ok(Self {
            inner,
            in_rate,
            out_rate,
            channels,
            pending: (0..channels).map(|_| Vec::new()).collect(),
        })
    }

    /// Resample one chunk of interleaved audio.
    ///
    /// Returns the resampled output as interleaved f32. Latency: the
    /// resampler buffers up to `chunk_size` input frames; trailing
    /// samples that don't fill a chunk remain in `self` and emerge on
    /// the next call.
    pub fn process_interleaved(&mut self, input: &[f32]) -> Result<Vec<f32>, ResampleError> {
        if self.channels == 0 {
            return Ok(Vec::new());
        }
        if input.len() % self.channels != 0 {
            return Err(ResampleError::UnalignedBuffer {
                samples: input.len(),
                channels: self.channels,
            });
        }
        // De-interleave into per-channel pending buffers.
        let frames = input.len() / self.channels;
        for f in 0..frames {
            for c in 0..self.channels {
                self.pending[c].push(input[f * self.channels + c]);
            }
        }
        let mut output_frames: Vec<Vec<f32>> = (0..self.channels).map(|_| Vec::new()).collect();

        // Drain whole chunks.
        loop {
            let needed = self.inner.input_frames_next();
            if self.pending[0].len() < needed {
                break;
            }
            // Build planar slices for rubato.
            let chunks: Vec<&[f32]> = (0..self.channels)
                .map(|c| &self.pending[c][..needed])
                .collect();
            let mut out: Vec<Vec<f32>> = (0..self.channels)
                .map(|_| Vec::with_capacity(self.inner.output_frames_next()))
                .collect();
            for buf in &mut out {
                buf.resize(self.inner.output_frames_next(), 0.0);
            }
            let mut out_slices: Vec<&mut [f32]> =
                out.iter_mut().map(|v| v.as_mut_slice()).collect();
            let (consumed_in, produced_out) =
                self.inner.process_into_buffer(&chunks, &mut out_slices)?;
            // Drop consumed input.
            for c in 0..self.channels {
                self.pending[c].drain(..consumed_in);
                output_frames[c].extend_from_slice(&out[c][..produced_out]);
            }
        }

        // Re-interleave.
        let frames_out = output_frames[0].len();
        let mut out = Vec::with_capacity(frames_out * self.channels);
        for f in 0..frames_out {
            for c in 0..self.channels {
                out.push(output_frames[c][f]);
            }
        }
        Ok(out)
    }

    /// Configured input rate, in Hz.
    #[must_use]
    pub fn input_rate(&self) -> u32 {
        self.in_rate
    }
    /// Configured output rate, in Hz.
    #[must_use]
    pub fn output_rate(&self) -> u32 {
        self.out_rate
    }
    /// Configured channel count.
    #[must_use]
    pub fn channels(&self) -> usize {
        self.channels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsample_8k_to_48k_preserves_signal_length_approximately() {
        let mut r = Resampler::new(8_000, 48_000, 2, Quality::Linear).unwrap();
        // 1 second of stereo at 8 kHz = 16 000 samples.
        let input = vec![0.1f32; 16_000];
        let out = r.process_interleaved(&input).unwrap();
        // Resampler buffers some input internally; output frames
        // should be ≈ 1 s × 48 kHz × 2 channels = 96 000 samples,
        // minus latency.
        assert!(
            out.len() > 80_000 && out.len() < 100_000,
            "got {} output samples, expected near 96000",
            out.len()
        );
    }

    #[test]
    fn highquality_path_works_at_44_1_to_48() {
        let mut r = Resampler::new(44_100, 48_000, 2, Quality::HighQuality).unwrap();
        // 0.5 s of -6 dBFS sine.
        let n = 22_050usize;
        let omega = 2.0 * std::f32::consts::PI * 1000.0 / 44_100.0;
        let mut input = Vec::with_capacity(n * 2);
        for i in 0..n {
            let v = 0.5 * (omega * i as f32).sin();
            input.push(v);
            input.push(v);
        }
        let out = r.process_interleaved(&input).unwrap();
        assert!(!out.is_empty());
    }

    #[test]
    fn unaligned_buffer_rejected() {
        let mut r = Resampler::new(48_000, 48_000, 2, Quality::Linear).unwrap();
        let err = r.process_interleaved(&[0.0, 0.0, 0.0]).unwrap_err();
        assert!(matches!(err, ResampleError::UnalignedBuffer { .. }));
    }
}
