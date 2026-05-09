//! WebAssembly bindings via [`wasm-bindgen`].
//!
//! Enabled by the `wasm` feature. Build with:
//!
//! ```bash
//! cargo build --target wasm32-unknown-unknown --no-default-features --features wasm,alloc
//! wasm-bindgen target/wasm32-unknown-unknown/debug/ebur128_stream.wasm \
//!   --out-dir pkg --target web
//! ```
//!
//! The generated TypeScript surface looks like:
//!
//! ```text
//! const a = new WasmAnalyzer(48000, 2, 0xFF);
//! a.push_interleaved(new Float32Array([...]));
//! const r = a.finalize();
//! console.log(r.integrated_lufs);
//! ```
//!
//! Modes are passed as a `u8` matching the [`Mode`](crate::Mode)
//! bitflags representation.

use crate::{Analyzer, AnalyzerBuilder, Channel, Mode, Report};
use wasm_bindgen::prelude::*;

/// Convert a numeric channel-count into a default channel layout
/// (`L, R` for 2; `L, R, C, LFE, Ls, Rs` for 6; otherwise `Other` ×N).
fn default_layout(n: u32) -> alloc::vec::Vec<Channel> {
    use alloc::vec;
    match n {
        1 => vec![Channel::Center],
        2 => vec![Channel::Left, Channel::Right],
        6 => vec![
            Channel::Left,
            Channel::Right,
            Channel::Center,
            Channel::Lfe,
            Channel::LeftSurround,
            Channel::RightSurround,
        ],
        n => (0..n).map(|_| Channel::Other).collect(),
    }
}

/// Browser-callable wrapper around [`Analyzer`].
#[wasm_bindgen]
pub struct WasmAnalyzer {
    inner: Analyzer,
}

#[wasm_bindgen]
impl WasmAnalyzer {
    /// Construct a new analyzer.
    ///
    /// `modes` is the bit OR of [`Mode`](crate::Mode) values:
    /// `Integrated = 1, Momentary = 2, ShortTerm = 4, TruePeak = 8,
    /// Lra = 16, All = 31`.
    #[wasm_bindgen(constructor)]
    pub fn new(sample_rate: u32, channels: u32, modes: u8) -> Result<WasmAnalyzer, JsError> {
        let layout = default_layout(channels);
        let modes = Mode::from_bits(modes).ok_or_else(|| JsError::new("invalid mode bitset"))?;
        let inner = AnalyzerBuilder::new()
            .sample_rate(sample_rate)
            .channels(&layout)
            .modes(modes)
            .build()
            .map_err(|e| JsError::new(&alloc::format!("{e}")))?;
        Ok(WasmAnalyzer { inner })
    }

    /// Push interleaved samples (`Float32Array`).
    pub fn push_interleaved(&mut self, samples: &[f32]) -> Result<(), JsError> {
        self.inner
            .push_interleaved::<f32>(samples)
            .map_err(|e| JsError::new(&alloc::format!("{e}")))
    }

    /// Take a current measurement snapshot.
    pub fn snapshot(&mut self) -> WasmSnapshot {
        let s = self.inner.snapshot();
        WasmSnapshot {
            momentary_lufs: s.momentary_lufs(),
            short_term_lufs: s.short_term_lufs(),
            integrated_lufs: s.integrated_lufs(),
            true_peak_dbtp: s.true_peak_dbtp(),
            loudness_range_lu: s.loudness_range_lu(),
            programme_duration_seconds: s.programme_duration_seconds(),
        }
    }

    /// Finalize and return the programme report.
    pub fn finalize(self) -> WasmReport {
        let r: Report = self.inner.finalize();
        WasmReport {
            integrated_lufs: r.integrated_lufs(),
            loudness_range_lu: r.loudness_range_lu(),
            true_peak_dbtp: r.true_peak_dbtp(),
            momentary_max_lufs: r.momentary_max_lufs(),
            short_term_max_lufs: r.short_term_max_lufs(),
            programme_duration_seconds: r.programme_duration_seconds(),
        }
    }

    /// Reset internal state, retaining configuration.
    pub fn reset(&mut self) {
        self.inner.reset();
    }
}

/// Snapshot exposed to JavaScript.
#[wasm_bindgen]
pub struct WasmSnapshot {
    /// Momentary loudness, LUFS. `null` if unset.
    pub momentary_lufs: Option<f64>,
    /// Short-term loudness, LUFS. `null` if unset.
    pub short_term_lufs: Option<f64>,
    /// Integrated loudness, LUFS. `null` if unset.
    pub integrated_lufs: Option<f64>,
    /// True peak, dBTP. `null` if unset.
    pub true_peak_dbtp: Option<f64>,
    /// Loudness range, LU. `null` if unset.
    pub loudness_range_lu: Option<f64>,
    /// Programme duration, seconds.
    pub programme_duration_seconds: f64,
}

/// Final report exposed to JavaScript.
#[wasm_bindgen]
pub struct WasmReport {
    /// Integrated loudness, LUFS.
    pub integrated_lufs: Option<f64>,
    /// Loudness range, LU.
    pub loudness_range_lu: Option<f64>,
    /// True peak, dBTP.
    pub true_peak_dbtp: Option<f64>,
    /// Maximum momentary loudness observed, LUFS.
    pub momentary_max_lufs: Option<f64>,
    /// Maximum short-term loudness observed, LUFS.
    pub short_term_max_lufs: Option<f64>,
    /// Programme duration, seconds.
    pub programme_duration_seconds: f64,
}
