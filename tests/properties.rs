//! Property-based tests over the analyzer's algebraic invariants.
//!
//! Where calibration tests check *one* signal against an expected
//! value, these tests assert that *many* random signals satisfy
//! invariants that must hold for any valid streaming loudness
//! analyzer:
//!
//! 1. **Determinism across chunkings** — pushing the same audio in any
//!    chunking schedule produces the same final report.
//! 2. **No NaN / inf escape** — for any finite-sample input, all
//!    measurements are either finite or `None`. NaN must not surface.
//! 3. **Reset idempotency** — running the analyzer, calling `reset()`,
//!    then running the same audio again is equivalent to a fresh
//!    analyzer.
//! 4. **Programme-duration linearity** — duration after pushing N
//!    samples equals N / sample_rate exactly.
//! 5. **Channel-weight invariance** — replacing a Center channel with
//!    `Other` (both unweighted) produces the same loudness.

use ebur128_stream::{AnalyzerBuilder, Channel, Mode, Report};
use proptest::prelude::*;

const FS: u32 = 48_000;

/// Generate a finite f32 audio buffer of length `n`, samples in
/// `[-1.0, 1.0]`. Filters out NaN / infinity at generation time so the
/// analyzer's `NonFiniteSample` rejection isn't tripped.
fn finite_audio_strategy(n: usize) -> impl Strategy<Value = Vec<f32>> {
    proptest::collection::vec(-1.0f32..=1.0f32, n)
}

/// Build a fresh analyzer with all modes for the given layout.
fn build(layout: &[Channel]) -> ebur128_stream::Analyzer {
    AnalyzerBuilder::new()
        .sample_rate(FS)
        .channels(layout)
        .modes(Mode::All)
        .build()
        .unwrap()
}

fn run_chunked(layout: &[Channel], samples: &[f32], chunk_frames: usize) -> Report {
    let mut a = build(layout);
    let cs = chunk_frames * layout.len();
    if cs == 0 {
        return a.finalize();
    }
    for c in samples.chunks(cs) {
        a.push_interleaved::<f32>(c).unwrap();
    }
    a.finalize()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    /// **Determinism**: pushing in different chunk sizes is equivalent.
    #[test]
    fn determinism_across_chunkings(
        samples in finite_audio_strategy(48_000 * 4) // 4 s mono
    ) {
        // Make stereo (L=R) so both channels exercise the filter.
        let stereo: Vec<f32> = samples.iter().flat_map(|s| [*s, *s]).collect();

        let layout = [Channel::Left, Channel::Right];
        let r_a = run_chunked(&layout, &stereo, 64);
        let r_b = run_chunked(&layout, &stereo, 1_024);
        let r_c = run_chunked(&layout, &stereo, 9_600);

        for (label, getter) in [
            ("integrated", &Report::integrated_lufs as &dyn Fn(&Report) -> Option<f64>),
            ("M_max", &Report::momentary_max_lufs),
            ("S_max", &Report::short_term_max_lufs),
            ("true_peak", &Report::true_peak_dbtp),
        ] {
            let (a, b, c) = (getter(&r_a), getter(&r_b), getter(&r_c));
            match (a, b, c) {
                (None, None, None) => {}
                (Some(a), Some(b), Some(c)) => {
                    prop_assert!(
                        (a - b).abs() < 1e-3 && (a - c).abs() < 1e-3,
                        "{label} differs: {a} {b} {c}"
                    );
                }
                _ => prop_assert!(false, "{label} mismatch: {a:?} {b:?} {c:?}"),
            }
        }
    }

    /// **Finiteness**: no `Some(NaN)` or `Some(±inf)` ever escapes the
    /// analyzer for finite-sample input.
    #[test]
    fn no_nan_or_inf_escape(samples in finite_audio_strategy(48_000)) {
        let stereo: Vec<f32> = samples.iter().flat_map(|s| [*s, *s]).collect();
        let layout = [Channel::Left, Channel::Right];
        let r = run_chunked(&layout, &stereo, 1_024);
        for v in [
            r.integrated_lufs(),
            r.loudness_range_lu(),
            r.true_peak_dbtp(),
            r.momentary_max_lufs(),
            r.short_term_max_lufs(),
        ].into_iter().flatten() {
            prop_assert!(v.is_finite(), "non-finite output: {v}");
        }
    }

    /// **Reset idempotency**: pushing audio, resetting, then pushing
    /// the same audio again equals a fresh analyzer.
    #[test]
    fn reset_idempotency(samples in finite_audio_strategy(48_000 * 2)) {
        let stereo: Vec<f32> = samples.iter().flat_map(|s| [*s, *s]).collect();
        let layout = [Channel::Left, Channel::Right];

        let mut a = build(&layout);
        a.push_interleaved::<f32>(&stereo).unwrap();
        a.reset();
        a.push_interleaved::<f32>(&stereo).unwrap();
        let after_reset = a.finalize();

        let mut b = build(&layout);
        b.push_interleaved::<f32>(&stereo).unwrap();
        let fresh = b.finalize();

        for (label, getter) in [
            ("integrated", &Report::integrated_lufs as &dyn Fn(&Report) -> Option<f64>),
            ("M_max", &Report::momentary_max_lufs),
            ("true_peak", &Report::true_peak_dbtp),
        ] {
            match (getter(&after_reset), getter(&fresh)) {
                (None, None) => {}
                (Some(a), Some(b)) => {
                    prop_assert!((a - b).abs() < 1e-9, "{label}: reset={a} fresh={b}");
                }
                _ => prop_assert!(false, "{label} disagreement"),
            }
        }
    }

    /// **Programme-duration linearity**: duration after pushing N
    /// frames equals N / sample_rate, exactly.
    #[test]
    fn programme_duration_linear(n in 1u32..200_000) {
        let samples = vec![0.0f32; n as usize * 2];
        let layout = [Channel::Left, Channel::Right];
        let mut a = build(&layout);
        a.push_interleaved::<f32>(&samples).unwrap();
        let dur = a.snapshot().programme_duration_seconds();
        let expected = n as f64 / FS as f64;
        prop_assert!((dur - expected).abs() < 1e-9, "{dur} != {expected}");
    }

    /// **Channel-weight invariance**: replacing `Center` with `Other`
    /// (both have weight = 1.0) leaves the integrated value unchanged.
    #[test]
    fn other_channel_equals_center(samples in finite_audio_strategy(48_000 * 4)) {
        let mut center = AnalyzerBuilder::new()
            .sample_rate(FS).channels(&[Channel::Center])
            .modes(Mode::Integrated).build().unwrap();
        center.push_interleaved::<f32>(&samples).unwrap();
        let l_c = center.finalize().integrated_lufs();

        let mut other = AnalyzerBuilder::new()
            .sample_rate(FS).channels(&[Channel::Other])
            .modes(Mode::Integrated).build().unwrap();
        other.push_interleaved::<f32>(&samples).unwrap();
        let l_o = other.finalize().integrated_lufs();

        match (l_c, l_o) {
            (None, None) => {}
            (Some(c), Some(o)) => {
                prop_assert!((c - o).abs() < 1e-9, "Center={c} Other={o}");
            }
            _ => prop_assert!(false, "{l_c:?} vs {l_o:?}"),
        }
    }

    /// **NonFiniteSample rejection**: a buffer containing NaN must
    /// produce `Error::NonFiniteSample`, not propagate the NaN.
    #[test]
    fn nan_input_rejected(prefix_len in 0u32..1_000) {
        let mut samples: Vec<f32> = (0..prefix_len).map(|i| 0.01 * (i as f32).sin()).collect();
        samples.push(f32::NAN);
        samples.push(0.0);
        let mut a = AnalyzerBuilder::new()
            .sample_rate(FS).channels(&[Channel::Left, Channel::Right])
            .modes(Mode::Momentary).build().unwrap();
        // Pad to whole frames.
        if samples.len() % 2 != 0 { samples.push(0.0); }
        let err = a.push_interleaved::<f32>(&samples).unwrap_err();
        prop_assert!(matches!(err, ebur128_stream::Error::NonFiniteSample));
    }
}
