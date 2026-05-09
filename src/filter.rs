//! K-weighting filter (ITU-R BS.1770-4 §3).
//!
//! Two-stage cascaded biquad in Direct Form I:
//!
//! 1. **Pre-filter** — a high-shelf (+\~4 dB at high frequencies) modelling
//!    the acoustic response of an idealised head/torso.
//! 2. **RLB** — a high-pass (~38 Hz) removing low-frequency rumble.
//!
//! Coefficients for 48 kHz are published verbatim in BS.1770-4. For
//! other supported rates we derive them analytically via the bilinear
//! transform from the analog prototype (matching the libebur128 reference
//! implementation).

use core::f64::consts::PI;

/// One stage of a Direct Form I biquad.
///
/// Coefficients are normalised so `a0 = 1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Biquad {
    /// Numerator coefficients: `[b0, b1, b2]`.
    pub b: [f32; 3],
    /// Denominator coefficients: `[a1, a2]` (with `a0 = 1`).
    pub a: [f32; 2],
    /// State: `[x_z1, x_z2, y_z1, y_z2]`.
    state: [f32; 4],
}

impl Biquad {
    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.b[1] * self.state[0] + self.b[2] * self.state[1]
            - self.a[0] * self.state[2]
            - self.a[1] * self.state[3];
        self.state[1] = self.state[0];
        self.state[0] = x;
        self.state[3] = self.state[2];
        self.state[2] = y;
        y
    }

    #[inline]
    fn reset(&mut self) {
        self.state = [0.0; 4];
    }
}

/// Two-stage K-weighting filter for one channel.
#[derive(Debug, Clone, Copy)]
pub(crate) struct KFilter {
    pre: Biquad,
    rlb: Biquad,
}

impl KFilter {
    /// Build a K-weighting filter for `sample_rate`.
    pub(crate) fn new(sample_rate: u32) -> Self {
        let (pre, rlb) = derive_coefficients(sample_rate);
        KFilter { pre, rlb }
    }

    /// Process one input sample and return the K-weighted output.
    #[inline]
    pub(crate) fn process(&mut self, x: f32) -> f32 {
        let y1 = self.pre.process(x);
        self.rlb.process(y1)
    }

    /// Reset all internal delay-line state to zero.
    #[inline]
    pub(crate) fn reset(&mut self) {
        self.pre.reset();
        self.rlb.reset();
    }
}

/// Analog-prototype parameters for the BS.1770-4 K-weighting filter,
/// matching the libebur128 reference implementation.
const PRE_F0: f64 = 1681.974450955533;
const PRE_Q: f64 = 0.7071752369554196;
const PRE_GAIN_DB: f64 = 3.999843853973347;
const RLB_F0: f64 = 38.13547087602444;
const RLB_Q: f64 = 0.5003270373238773;

fn derive_coefficients(sample_rate: u32) -> (Biquad, Biquad) {
    let fs = sample_rate as f64;

    // ---- Stage 1: high-shelf (pre-filter) ----
    let k = libm::tan(PI * PRE_F0 / fs);
    let vh = libm::pow(10.0, PRE_GAIN_DB / 20.0);
    let vb = libm::pow(vh, 0.4996667741545416);
    let kk = k * k;
    let kq = k / PRE_Q;
    let a0_pre = 1.0 + kq + kk;
    let pre_b0 = (vh + vb * kq + kk) / a0_pre;
    let pre_b1 = 2.0 * (kk - vh) / a0_pre;
    let pre_b2 = (vh - vb * kq + kk) / a0_pre;
    let pre_a1 = 2.0 * (kk - 1.0) / a0_pre;
    let pre_a2 = (1.0 - kq + kk) / a0_pre;

    let pre = Biquad {
        b: [pre_b0 as f32, pre_b1 as f32, pre_b2 as f32],
        a: [pre_a1 as f32, pre_a2 as f32],
        state: [0.0; 4],
    };

    // ---- Stage 2: high-pass (RLB) ----
    let k2 = libm::tan(PI * RLB_F0 / fs);
    let kk2 = k2 * k2;
    let kq2 = k2 / RLB_Q;
    let a0_rlb = 1.0 + kq2 + kk2;
    // BS.1770-4 specifies the RLB numerator as [1, -2, 1] (a high-pass
    // with unit DC-block gain), without normalising by a0 — the standard
    // reference values for 48 kHz follow that convention.
    let rlb_b0 = 1.0_f64;
    let rlb_b1 = -2.0_f64;
    let rlb_b2 = 1.0_f64;
    let rlb_a1 = 2.0 * (kk2 - 1.0) / a0_rlb;
    let rlb_a2 = (1.0 - kq2 + kk2) / a0_rlb;

    let rlb = Biquad {
        b: [rlb_b0 as f32, rlb_b1 as f32, rlb_b2 as f32],
        a: [rlb_a1 as f32, rlb_a2 as f32],
        state: [0.0; 4],
    };

    (pre, rlb)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// At 48 kHz the published BS.1770-4 reference coefficients are:
    ///
    /// Pre-filter:
    ///   b = [ 1.53512485958697, -2.69169618940638, 1.19839281085285 ]
    ///   a = [ 1.0, -1.69065929318241, 0.73248077421585 ]
    ///
    /// RLB:
    ///   b = [ 1.0, -2.0, 1.0 ]
    ///   a = [ 1.0, -1.99004745483398, 0.99007225036621 ]
    #[test]
    fn coefficients_match_reference_at_48k() {
        let (pre, rlb) = derive_coefficients(48_000);
        let eps = 1e-4_f32;
        assert!((pre.b[0] - 1.535_124_8_f32).abs() < eps);
        assert!((pre.b[1] - -2.691_696_2_f32).abs() < eps);
        assert!((pre.b[2] - 1.198_392_8_f32).abs() < eps);
        assert!((pre.a[0] - -1.690_659_3_f32).abs() < eps);
        assert!((pre.a[1] - 0.732_480_8_f32).abs() < eps);

        assert!((rlb.b[0] - 1.0_f32).abs() < eps);
        assert!((rlb.b[1] - -2.0_f32).abs() < eps);
        assert!((rlb.b[2] - 1.0_f32).abs() < eps);
        assert!((rlb.a[0] - -1.990_047_5_f32).abs() < eps);
        assert!((rlb.a[1] - 0.990_072_2_f32).abs() < eps);
    }

    #[test]
    fn impulse_response_decays() {
        let mut f = KFilter::new(48_000);
        let _ = f.process(1.0);
        for _ in 0..100_000 {
            let v = f.process(0.0);
            assert!(v.abs() < 10.0, "filter blew up");
        }
    }

    #[test]
    fn reset_zeros_state() {
        let mut f = KFilter::new(48_000);
        for _ in 0..1_000 {
            f.process(0.5);
        }
        f.reset();
        // After reset, the next zero-input output should also be ≈0.
        let v = f.process(0.0);
        assert!(v.abs() < 1e-6);
    }
}
