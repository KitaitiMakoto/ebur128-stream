//! Cross-validation against the `ebur128` crate (the established Rust
//! implementation, originally bindings to `libebur128` C, now pure
//! Rust upstream).
//!
//! For each test programme we run the same audio through both
//! implementations and assert the integrated LUFS, true peak, and LRA
//! values agree within published tolerances.

use ebur128::{EbuR128, Mode as EbuMode};
use ebur128_stream::{AnalyzerBuilder, Channel, Mode};

const FS: u32 = 48_000;

fn synth(amp: f32, seconds: f32, freq: f32) -> Vec<f32> {
    let n = (FS as f32 * seconds) as usize;
    let omega = 2.0 * std::f32::consts::PI * freq / FS as f32;
    (0..n).map(|i| amp * (omega * i as f32).sin()).collect()
}

#[test]
fn integrated_agrees_within_0_5_lu_for_stereo_sine() {
    let mono = synth(0.1, 10.0, 1000.0);
    let stereo: Vec<f32> = mono.iter().flat_map(|s| [*s, *s]).collect();

    // Our crate
    let mut ours = AnalyzerBuilder::new()
        .sample_rate(FS)
        .channels(&[Channel::Left, Channel::Right])
        .modes(Mode::Integrated)
        .build()
        .unwrap();
    ours.push_interleaved::<f32>(&stereo).unwrap();
    let i_ours = ours.finalize().integrated_lufs().unwrap();

    // Reference
    let mut theirs = EbuR128::new(2, FS, EbuMode::I).unwrap();
    theirs.add_frames_f32(&stereo).unwrap();
    let i_theirs = theirs.loudness_global().unwrap();

    let delta = (i_ours - i_theirs).abs();
    assert!(
        delta <= 0.5,
        "integrated mismatch: ours = {i_ours:.3}, ref = {i_theirs:.3}, |Δ| = {delta:.4} LU"
    );
}

#[test]
fn true_peak_agrees_within_0_5_dbtp_for_inter_sample_signal() {
    let signal = synth(1.0, 2.0, 0.4615 * FS as f32);
    let stereo: Vec<f32> = signal.iter().flat_map(|s| [*s, *s]).collect();

    let mut ours = AnalyzerBuilder::new()
        .sample_rate(FS)
        .channels(&[Channel::Left, Channel::Right])
        .modes(Mode::TruePeak)
        .build()
        .unwrap();
    ours.push_interleaved::<f32>(&stereo).unwrap();
    let tp_ours = ours.finalize().true_peak_dbtp().unwrap();

    let mut theirs = EbuR128::new(2, FS, EbuMode::TRUE_PEAK).unwrap();
    theirs.add_frames_f32(&stereo).unwrap();
    let tp_theirs_linear = (0..2)
        .map(|c| theirs.true_peak(c).unwrap())
        .fold(0.0_f64, f64::max);
    let tp_theirs = 20.0 * tp_theirs_linear.log10();

    let delta = (tp_ours - tp_theirs).abs();
    assert!(
        delta <= 0.5,
        "true peak mismatch: ours = {tp_ours:.3}, ref = {tp_theirs:.3}, |Δ| = {delta:.4} dBTP"
    );
}

#[test]
fn lra_agrees_within_2_lu_for_dynamic_programme() {
    // 30 s alternating between -23 and -36 LUFS in 5 s segments,
    // long enough for the 3 s short-term window to settle.
    let mut signal: Vec<f32> = Vec::new();
    for &amp in &[0.1f32, 0.018, 0.1, 0.018, 0.1, 0.018] {
        signal.extend(synth(amp, 5.0, 1000.0));
    }
    let stereo: Vec<f32> = signal.iter().flat_map(|s| [*s, *s]).collect();

    let mut ours = AnalyzerBuilder::new()
        .sample_rate(FS)
        .channels(&[Channel::Left, Channel::Right])
        .modes(Mode::Lra)
        .build()
        .unwrap();
    ours.push_interleaved::<f32>(&stereo).unwrap();
    let lra_ours = ours.finalize().loudness_range_lu().unwrap();

    let mut theirs = EbuR128::new(2, FS, EbuMode::LRA).unwrap();
    theirs.add_frames_f32(&stereo).unwrap();
    let lra_theirs = theirs.loudness_range().unwrap();

    let delta = (lra_ours - lra_theirs).abs();
    assert!(
        delta <= 2.0,
        "LRA mismatch: ours = {lra_ours:.3}, ref = {lra_theirs:.3}, |Δ| = {delta:.4} LU"
    );
}

#[test]
fn integrated_agrees_for_multi_segment_gating_programme() {
    // Programme designed to exercise the gating algorithm:
    // 20 s @ -36 LUFS, 60 s @ -23 LUFS, 20 s @ -36 LUFS.
    let mut signal: Vec<f32> = Vec::new();
    signal.extend(synth(0.018, 20.0, 1000.0));
    signal.extend(synth(0.1, 60.0, 1000.0));
    signal.extend(synth(0.018, 20.0, 1000.0));
    let stereo: Vec<f32> = signal.iter().flat_map(|s| [*s, *s]).collect();

    let mut ours = AnalyzerBuilder::new()
        .sample_rate(FS)
        .channels(&[Channel::Left, Channel::Right])
        .modes(Mode::Integrated)
        .build()
        .unwrap();
    ours.push_interleaved::<f32>(&stereo).unwrap();
    let i_ours = ours.finalize().integrated_lufs().unwrap();

    let mut theirs = EbuR128::new(2, FS, EbuMode::I).unwrap();
    theirs.add_frames_f32(&stereo).unwrap();
    let i_theirs = theirs.loudness_global().unwrap();

    let delta = (i_ours - i_theirs).abs();
    assert!(
        delta <= 0.5,
        "gated I mismatch: ours = {i_ours:.3}, ref = {i_theirs:.3}, |Δ| = {delta:.4} LU"
    );
}
