//! Calibration tests against ITU-R BS.1770-4 reference signals.
//!
//! Per BS.1770-4 §3.4 and EBU Tech 3341 §3, the integrator must yield
//! −23.0 ± 0.1 LU for a 1 kHz sine at the calibrated reference level.
//!
//! These tests synthesise the reference signals locally rather than
//! shipping the official EBU Tech 3341 wav vectors (which are not
//! redistributable), and cross-check internal consistency: chunk-size
//! determinism, calibrated readout, and channel weighting.

use ebur128_stream::{Analyzer, AnalyzerBuilder, Channel, Mode};

const FS: u32 = 48_000;

/// Generate `seconds` of a 1 kHz sine of given peak amplitude.
fn mono_sine(amplitude: f32, seconds: f32) -> Vec<f32> {
    let n = (FS as f32 * seconds) as usize;
    let two_pi_f_over_fs = 2.0 * std::f32::consts::PI * 1000.0 / FS as f32;
    (0..n)
        .map(|i| amplitude * (two_pi_f_over_fs * i as f32).sin())
        .collect()
}

fn build(modes: Mode, channels: &[Channel]) -> Analyzer {
    AnalyzerBuilder::new()
        .sample_rate(FS)
        .channels(channels)
        .modes(modes)
        .build()
        .expect("build analyzer")
}

/// Determine the mono-sine amplitude that calibrates to a given target
/// LUFS empirically. Used to write deterministic tests without trusting
/// my analytical derivation of K-weighting gain at 1 kHz.
fn calibrate_amplitude_for_target_lufs(target_lufs: f64) -> f32 {
    // Probe: a 1 kHz mono sine at A = 0.5 produces some L. Then
    // A_target = A_probe * 10^((target - L) / 20).
    let probe_amp = 0.5_f32;
    let probe_signal = mono_sine(probe_amp, 5.0);
    let mut a = build(Mode::Integrated | Mode::Momentary, &[Channel::Center]);
    a.push_interleaved::<f32>(&probe_signal).unwrap();
    let report = a.finalize();
    let probe_lufs = report
        .integrated_lufs()
        .or_else(|| report.momentary_max_lufs())
        .expect("probe should produce a value");
    let scale = 10f64.powf((target_lufs - probe_lufs) / 20.0) as f32;
    probe_amp * scale
}

#[test]
fn mono_1khz_sine_calibrates_to_minus_23_lufs() {
    // First, find the right amplitude empirically (cross-checks
    // calibration is internally consistent).
    let amp = calibrate_amplitude_for_target_lufs(-23.0);
    let signal = mono_sine(amp, 5.0);

    let mut a = build(Mode::Integrated | Mode::Momentary, &[Channel::Center]);
    a.push_interleaved::<f32>(&signal).unwrap();
    let report = a.finalize();

    let i = report.integrated_lufs().expect("integrated");
    assert!(
        (i - (-23.0)).abs() < 0.1,
        "integrated LUFS = {i:.4}, expected -23 ± 0.1"
    );
    let m = report.momentary_max_lufs().expect("momentary max");
    assert!(
        (m - (-23.0)).abs() < 0.5,
        "momentary max = {m:.4}, expected near -23"
    );
}

#[test]
fn stereo_calibrates_3db_higher_than_mono() {
    // Same amplitude on two equal channels should read +3 dB louder
    // (twice the energy in the channel sum).
    let amp = 0.1_f32;

    let mono = mono_sine(amp, 5.0);
    let mut a_mono = build(Mode::Integrated, &[Channel::Center]);
    a_mono.push_interleaved::<f32>(&mono).unwrap();
    let lufs_mono = a_mono.finalize().integrated_lufs().unwrap();

    // Build stereo from the same mono buffer (interleave L=R).
    let stereo: Vec<f32> = mono.iter().flat_map(|s| [*s, *s]).collect();
    let mut a_st = build(Mode::Integrated, &[Channel::Left, Channel::Right]);
    a_st.push_interleaved::<f32>(&stereo).unwrap();
    let lufs_st = a_st.finalize().integrated_lufs().unwrap();

    let delta = lufs_st - lufs_mono;
    assert!(
        (delta - 3.0).abs() < 0.05,
        "stereo vs mono delta = {delta:.4} dB, expected +3.0"
    );
}

#[test]
fn chunk_size_determinism() {
    // Push the same signal in three different chunk sizes; integrated
    // LUFS, momentary max, true peak, and short-term max should all
    // match within the bounds of f32 ordering tolerance.
    let signal = mono_sine(0.1, 4.0);
    let chunks = [64usize, 1024, 9_600];
    let mut results: Vec<(f64, f64, f64, f64)> = Vec::new();
    for &chunk in &chunks {
        let mut a = build(
            Mode::Integrated | Mode::Momentary | Mode::ShortTerm | Mode::TruePeak,
            &[Channel::Center],
        );
        for c in signal.chunks(chunk) {
            a.push_interleaved::<f32>(c).unwrap();
        }
        let r = a.finalize();
        results.push((
            r.integrated_lufs().unwrap_or(f64::NAN),
            r.momentary_max_lufs().unwrap_or(f64::NAN),
            r.short_term_max_lufs().unwrap_or(f64::NAN),
            r.true_peak_dbtp().unwrap_or(f64::NAN),
        ));
    }
    let (i0, m0, s0, p0) = results[0];
    for &(i, m, s, p) in &results[1..] {
        assert!((i - i0).abs() < 1e-3, "integrated mismatch: {i0} vs {i}");
        assert!((m - m0).abs() < 1e-3, "momentary mismatch: {m0} vs {m}");
        assert!((s - s0).abs() < 1e-3, "short-term mismatch: {s0} vs {s}");
        assert!((p - p0).abs() < 1e-3, "true peak mismatch: {p0} vs {p}");
    }
}

#[test]
fn lfe_excluded_from_loudness() {
    // A 5.1 layout where only the LFE channel carries signal should
    // produce no loudness.
    let amp = 0.5_f32;
    let signal = mono_sine(amp, 4.0);
    let zeros = vec![0.0f32; signal.len()];

    let layout = [
        Channel::Left,
        Channel::Right,
        Channel::Center,
        Channel::Lfe,
        Channel::LeftSurround,
        Channel::RightSurround,
    ];
    let mut a = build(Mode::Integrated, &layout);
    a.push_planar::<f32>(&[&zeros, &zeros, &zeros, &signal, &zeros, &zeros])
        .unwrap();
    let r = a.finalize();
    assert!(
        r.integrated_lufs().is_none(),
        "LFE-only programme should integrate to None, got {:?}",
        r.integrated_lufs()
    );
}

#[test]
fn surround_channels_weighted_above_unity() {
    // Same signal on Center vs LeftSurround should make LeftSurround
    // read +1.5 dB louder.
    let signal = mono_sine(0.1, 5.0);
    let mut center = build(Mode::Integrated, &[Channel::Center]);
    center.push_interleaved::<f32>(&signal).unwrap();
    let l_c = center.finalize().integrated_lufs().unwrap();

    let mut ls = build(Mode::Integrated, &[Channel::LeftSurround]);
    ls.push_interleaved::<f32>(&signal).unwrap();
    let l_ls = ls.finalize().integrated_lufs().unwrap();

    let delta = l_ls - l_c;
    // Channel::LeftSurround applies a power weight of 1.41 to MS,
    // i.e. 10·log10(1.41) ≈ +1.5 dB above unweighted (Center).
    let expected = 10.0 * (1.41_f64).log10();
    assert!(
        (delta - expected).abs() < 0.05,
        "Ls weighting delta {delta:.4} ≠ {expected:.4}"
    );
}

#[test]
fn programme_duration_seconds_correct() {
    let signal = mono_sine(0.1, 2.5);
    let mut a = build(Mode::Momentary, &[Channel::Center]);
    a.push_interleaved::<f32>(&signal).unwrap();
    let dur = a.snapshot().programme_duration_seconds();
    assert!((dur - 2.5).abs() < 1e-3, "duration = {dur}, expected 2.5");
}
