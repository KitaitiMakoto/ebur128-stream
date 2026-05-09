//! True-peak measurement via 4× polyphase oversampling.
//!
//! Per ITU-R BS.1770-4 Annex 2, true peak is measured by upsampling the
//! input by 4× through a 48-tap polyphase FIR (12 taps per phase) and
//! tracking the absolute maximum of the oversampled signal.
//!
//! The polyphase coefficients are published in Annex 2 of BS.1770 and
//! match the reference values used by `libebur128`.

// Coefficients are reproduced verbatim from the standard; trailing zeros
// in some literals exceed f32 precision (clippy::excessive_precision)
// but we keep the published form for traceability.
#![allow(clippy::excessive_precision, clippy::needless_range_loop)]

use alloc::vec::Vec;

/// Length of each polyphase filter, in input samples.
const TAPS: usize = 12;

/// 4× polyphase coefficients from BS.1770-4 Annex 2 (12 taps per phase,
/// 48 coefficients total).
const COEFFS: [[f32; TAPS]; 4] = [
    // phase 0
    [
        0.001_708_984_4,
        0.010_986_328_0,
        -0.019_653_320_0,
        0.033_203_125_0,
        -0.059_448_242_0,
        0.137_329_100_0,
        0.972_167_970_0,
        -0.102_294_920_0,
        0.047_607_422_0,
        -0.026_611_328_0,
        0.014_892_578_0,
        -0.008_300_781_3,
    ],
    // phase 1
    [
        -0.029_174_805_0,
        0.029_296_875_0,
        -0.051_757_812_0,
        0.089_111_328_0,
        -0.166_503_910_0,
        0.465_087_890_0,
        0.779_785_160_0,
        -0.200_317_380_0,
        0.101_562_500_0,
        -0.058_227_540_0,
        0.033_081_055_0,
        -0.018_920_898_0,
    ],
    // phase 2 (= phase 1 reversed by symmetry)
    [
        -0.018_920_898_0,
        0.033_081_055_0,
        -0.058_227_540_0,
        0.101_562_500_0,
        -0.200_317_380_0,
        0.779_785_160_0,
        0.465_087_890_0,
        -0.166_503_910_0,
        0.089_111_328_0,
        -0.051_757_812_0,
        0.029_296_875_0,
        -0.029_174_805_0,
    ],
    // phase 3 (= phase 0 reversed by symmetry)
    [
        -0.008_300_781_3,
        0.014_892_578_0,
        -0.026_611_328_0,
        0.047_607_422_0,
        -0.102_294_920_0,
        0.972_167_970_0,
        0.137_329_100_0,
        -0.059_448_242_0,
        0.033_203_125_0,
        -0.019_653_320_0,
        0.010_986_328_0,
        0.001_708_984_4,
    ],
];

/// Per-channel polyphase state. The delay line stores the most recent
/// `TAPS` raw input samples in a circular buffer.
#[derive(Debug)]
struct ChannelState {
    delay: [f32; TAPS],
    write: usize,
    peak_abs: f32,
}

impl ChannelState {
    fn new() -> Self {
        Self {
            delay: [0.0; TAPS],
            write: 0,
            peak_abs: 0.0,
        }
    }

    fn reset(&mut self) {
        self.delay = [0.0; TAPS];
        self.write = 0;
        self.peak_abs = 0.0;
    }

    #[inline]
    fn feed(&mut self, x: f32) {
        // Insert into circular delay line.
        self.delay[self.write] = x;
        self.write = (self.write + 1) % TAPS;

        // Track the absolute peak of the raw sample first — at high
        // sample rates the inter-sample peak adds little, but the
        // sample peak itself is always an admissible lower bound.
        let abs_x = x.abs();
        if abs_x > self.peak_abs {
            self.peak_abs = abs_x;
        }

        #[cfg(feature = "simd")]
        {
            self.feed_phases_simd();
        }
        #[cfg(not(feature = "simd"))]
        {
            self.feed_phases_scalar();
        }
    }

    #[cfg(not(feature = "simd"))]
    #[inline]
    fn feed_phases_scalar(&mut self) {
        // Run all four polyphase taps; track the max |y|.
        for phase in 0..4 {
            let mut acc = 0.0f32;
            // Iterate the delay line in reverse-chronological order
            // (most recent sample first).
            for i in 0..TAPS {
                // index = (write - 1 - i) mod TAPS
                let idx = (self.write + TAPS - 1 - i) % TAPS;
                acc += COEFFS[phase][i] * self.delay[idx];
            }
            let abs = acc.abs();
            if abs > self.peak_abs {
                self.peak_abs = abs;
            }
        }
    }

    #[cfg(feature = "simd")]
    #[inline]
    fn feed_phases_simd(&mut self) {
        use wide::f32x4;
        // Build the 12-element delay vector in chronological-reverse
        // order so it lines up with the published phase coefficients.
        let mut delay_ord = [0.0f32; TAPS];
        for i in 0..TAPS {
            let idx = (self.write + TAPS - 1 - i) % TAPS;
            delay_ord[i] = self.delay[idx];
        }
        // 12 taps = 3 × f32x4 lanes.
        let d0 = f32x4::from([delay_ord[0], delay_ord[1], delay_ord[2], delay_ord[3]]);
        let d1 = f32x4::from([delay_ord[4], delay_ord[5], delay_ord[6], delay_ord[7]]);
        let d2 = f32x4::from([delay_ord[8], delay_ord[9], delay_ord[10], delay_ord[11]]);

        for phase in 0..4 {
            let c = &COEFFS[phase];
            let c0 = f32x4::from([c[0], c[1], c[2], c[3]]);
            let c1 = f32x4::from([c[4], c[5], c[6], c[7]]);
            let c2 = f32x4::from([c[8], c[9], c[10], c[11]]);
            let acc = (c0 * d0) + (c1 * d1) + (c2 * d2);
            // Horizontal sum.
            let lanes = acc.to_array();
            let sum = lanes[0] + lanes[1] + lanes[2] + lanes[3];
            let abs = sum.abs();
            if abs > self.peak_abs {
                self.peak_abs = abs;
            }
        }
    }
}

/// True-peak tracker across all configured channels.
#[derive(Debug)]
pub(crate) struct TruePeakState {
    channels: Vec<ChannelState>,
}

impl TruePeakState {
    pub(crate) fn new(n_channels: usize, _sample_rate: u32) -> Self {
        Self {
            channels: (0..n_channels).map(|_| ChannelState::new()).collect(),
        }
    }

    pub(crate) fn reset(&mut self) {
        for c in &mut self.channels {
            c.reset();
        }
    }

    #[inline]
    pub(crate) fn feed_frame(&mut self, frame: &[f32]) {
        for (ch, s) in frame.iter().enumerate() {
            if let Some(state) = self.channels.get_mut(ch) {
                state.feed(*s);
            }
        }
    }

    /// Programme-wide maximum true peak in dBTP (decibels relative to
    /// full scale, true peak). Returns `None` if no audio has been
    /// pushed.
    pub(crate) fn peak_dbtp(&self) -> Option<f64> {
        let mut max_abs = 0.0f32;
        for c in &self.channels {
            if c.peak_abs > max_abs {
                max_abs = c.peak_abs;
            }
        }
        if max_abs <= 0.0 {
            None
        } else {
            Some(20.0 * libm::log10(max_abs as f64))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coefficient_phases_have_unit_dc_response() {
        // Sum of coefficients across all 4 phases should be ~ 4 (since
        // each phase has DC gain ≈ 1 and we have 4 phases).
        let total: f32 = COEFFS.iter().flatten().sum();
        assert!(
            (total - 4.0).abs() < 0.5,
            "DC gain across phases should sum to ~4; got {total}"
        );
    }

    #[test]
    fn full_scale_dc_gives_about_zero_dbtp() {
        let mut tp = TruePeakState::new(1, 48_000);
        // 1000 samples at full-scale DC
        for _ in 0..1_000 {
            tp.feed_frame(&[1.0]);
        }
        let db = tp.peak_dbtp().unwrap();
        // Should be very near 0 dBTP.
        assert!(db.abs() < 1.0, "expected ≈0 dBTP, got {db}");
    }

    #[test]
    fn silent_input_returns_none() {
        let tp = TruePeakState::new(2, 48_000);
        assert!(tp.peak_dbtp().is_none());
    }
}
