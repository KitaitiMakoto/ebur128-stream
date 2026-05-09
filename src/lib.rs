//! # ebur128-stream
//!
//! Streaming, zero-allocation EBU R128 loudness measurement in pure Rust.
//!
//! `ebur128-stream` implements the
//! [ITU-R BS.1770-4](https://www.itu.int/rec/R-REC-BS.1770) loudness
//! algorithm and the surrounding [EBU R128] practices (momentary,
//! short-term, integrated, true peak, loudness range) with a **push-based
//! streaming API** that accepts arbitrary chunk sizes deterministically
//! and performs no allocations on the hot path.
//!
//! [EBU R128]: https://tech.ebu.ch/docs/r/r128.pdf
//!
//! ## Quick start
//!
//! ```
//! use ebur128_stream::{AnalyzerBuilder, Channel, Mode};
//!
//! let mut analyzer = AnalyzerBuilder::new()
//!     .sample_rate(48_000)
//!     .channels(&[Channel::Left, Channel::Right])
//!     .modes(Mode::Integrated | Mode::TruePeak)
//!     .build()?;
//!
//! // Push interleaved stereo samples (any chunk size; the analyzer is
//! // chunk-size-deterministic).
//! let stereo = vec![0.0f32; 9_600]; // 100 ms of silence at 48 kHz
//! analyzer.push_interleaved(&stereo)?;
//!
//! let report = analyzer.finalize();
//! // For silent input, integrated loudness is None (nothing cleared the
//! // -70 LUFS absolute gate).
//! assert!(report.integrated_lufs().is_none());
//! # Ok::<(), ebur128_stream::Error>(())
//! ```
//!
//! ## Concepts
//!
//! ### What is loudness?
//!
//! Loudness is the *perceived* energy of an audio signal — what the ear
//! hears, not what a meter sees. The classic peak / RMS meter measures
//! signal *amplitude*; loudness meters apply a frequency-weighted
//! filter (the **K-weighting** filter of BS.1770) that approximates the
//! ear's sensitivity, then sum energy across channels with appropriate
//! per-channel weights, and report a single calibrated value in
//! **LUFS** (Loudness Units relative to Full Scale).
//!
//! Two signals at the same RMS can differ by 10 LU in measured
//! loudness if one has more high-frequency content than the other.
//! This is why broadcasters use LUFS rather than peak / RMS for
//! loudness compliance.
//!
//! ### Momentary, short-term, integrated
//!
//! BS.1770 defines three measurement windows:
//!
//! ```text
//!   M (momentary)    : sliding 400 ms window  → quick reaction
//!   S (short-term)   : sliding 3 s window     → smoother envelope
//!   I (integrated)   : full programme, gated  → "the" loudness number
//! ```
//!
//! Integrated loudness uses **gating** to ignore quiet sections that
//! shouldn't drag the programme average down: an absolute gate at
//! -70 LUFS, then a relative gate -10 LU below the absolute-gated mean.
//!
//! ### True peak vs. sample peak
//!
//! Sample peak — the largest absolute value in the PCM buffer — undercounts
//! the actual signal level whenever a peak falls *between* samples. True
//! peak (per BS.1770 Annex 2) approximates the analog reconstructed signal
//! by 4× upsampling through a 12-tap polyphase FIR and reports the
//! oversampled max in **dBTP** (decibels relative to full scale, true
//! peak).
//!
//! ## Choosing a mode
//!
//! | Use case | Modes to request |
//! |---|---|
//! | Real-time monitor | [`Mode::Momentary`] + [`Mode::ShortTerm`] |
//! | Programme delivery QC | [`Mode::Integrated`] + [`Mode::TruePeak`] |
//! | Mastering / archival | [`Mode::All`] |
//! | True-peak limiter feed | [`Mode::TruePeak`] only |
//!
//! ## Performance characteristics
//!
//! - **Allocations on `push_*`:** zero in steady state. The programme
//!   buffer ([`Mode::Integrated`], [`Mode::Lra`]) may grow when its
//!   `Vec` exceeds capacity; pre-call [`Vec::with_capacity`] semantics
//!   are out of scope for V0.1 but tracked for V0.2.
//! - **Computational cost:** O(samples) per push, dominated by the
//!   K-weighting biquad and (when enabled) the 4× polyphase FIR.
//! - **Snapshot cost:** O(1) for [`Snapshot::momentary_lufs`] /
//!   [`Snapshot::short_term_lufs`] (cached); O(programme blocks) for
//!   [`Snapshot::integrated_lufs`] (cached and invalidated on next
//!   push).
//!
//! ## Compliance
//!
//! Calibration is verified against synthetic stimuli matching the
//! ITU-R BS.1770-4 reference signals. Independent verification against
//! the official [EBU Tech 3341] test vectors is in progress; see
//! `tests/calibration.rs` for the current cross-checks.
//!
//! [EBU Tech 3341]: https://tech.ebu.ch/docs/tech/tech3341.pdf
//!
//! ## See also
//!
//! - [`bs1770`](https://crates.io/crates/bs1770) — filter only,
//!   no gating, no streaming API.
//! - [`ebur128`](https://crates.io/crates/ebur128) — Rust bindings to
//!   the C `libebur128`.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "alloc")]
extern crate alloc;

mod analyzer;
mod blocks;
mod channel;
mod error;
mod filter;
#[cfg(feature = "alloc")]
mod gating;
#[cfg(feature = "alloc")]
mod lra;
mod mode;
#[cfg(feature = "normalize")]
pub mod normalize;
#[cfg(feature = "alloc")]
mod peak;
mod report;
#[cfg(feature = "resampler")]
pub mod resampler;
mod sample;
#[cfg(feature = "tokio")]
mod sink;
mod snapshot;
#[cfg(feature = "svg")]
pub mod svg;
#[cfg(feature = "wasm")]
mod wasm;
#[cfg(feature = "wasm")]
pub use wasm::{WasmAnalyzer, WasmReport, WasmSnapshot};

pub use analyzer::{Analyzer, AnalyzerBuilder};
pub use channel::Channel;
pub use error::Error;
pub use mode::Mode;
pub use report::Report;
pub use sample::Sample;
#[cfg(feature = "tokio")]
pub use sink::AnalyzerSink;
pub use snapshot::Snapshot;
