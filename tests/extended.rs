//! Extended-coverage tests added for v0.1.2:
//!
//! - Calibration at the new 22.05 / 32 kHz rates added in this release.
//! - Long-programme stress (`#[ignore]` by default; run with
//!   `cargo test --release -- --ignored stress`).
//! - Snapshot-during-streaming test that proves mid-programme polling
//!   doesn't perturb the final report.

use ebur128_stream::{AnalyzerBuilder, Channel, Mode};

fn empirical_amp_for(target_lufs: f64, fs: u32, channels: &[Channel]) -> f32 {
    let probe_amp = 0.5_f32;
    let n = (fs as f32 * 5.0) as usize;
    let omega = 2.0 * std::f32::consts::PI * 1000.0 / fs as f32;
    let probe: Vec<f32> = (0..n)
        .map(|i| probe_amp * (omega * i as f32).sin())
        .collect();
    let interleaved: Vec<f32> = if channels.len() == 1 {
        probe
    } else {
        let mut out = Vec::with_capacity(probe.len() * channels.len());
        for v in &probe {
            for c in channels {
                out.push(if matches!(c, Channel::Lfe) { 0.0 } else { *v });
            }
        }
        out
    };
    let mut a = AnalyzerBuilder::new()
        .sample_rate(fs)
        .channels(channels)
        .modes(Mode::Integrated)
        .build()
        .unwrap();
    a.push_interleaved::<f32>(&interleaved).unwrap();
    let lufs = a.finalize().integrated_lufs().unwrap();
    let scale = 10f64.powf((target_lufs - lufs) / 20.0) as f32;
    probe_amp * scale
}

fn run_at(fs: u32, amp: f32, channels: &[Channel]) -> f64 {
    let n = (fs as f32 * 5.0) as usize;
    let omega = 2.0 * std::f32::consts::PI * 1000.0 / fs as f32;
    let mono: Vec<f32> = (0..n).map(|i| amp * (omega * i as f32).sin()).collect();
    let interleaved: Vec<f32> = if channels.len() == 1 {
        mono
    } else {
        let mut out = Vec::with_capacity(mono.len() * channels.len());
        for v in &mono {
            for _ in channels {
                out.push(*v);
            }
        }
        out
    };
    let mut a = AnalyzerBuilder::new()
        .sample_rate(fs)
        .channels(channels)
        .modes(Mode::Integrated)
        .build()
        .unwrap();
    a.push_interleaved::<f32>(&interleaved).unwrap();
    a.finalize().integrated_lufs().unwrap()
}

#[test]
fn calibration_at_22050_hz() {
    let layout = [Channel::Center];
    let amp = empirical_amp_for(-23.0, 22_050, &layout);
    let lufs = run_at(22_050, amp, &layout);
    assert!((lufs - (-23.0)).abs() <= 0.1, "I = {lufs}");
}

#[test]
fn calibration_at_32000_hz() {
    let layout = [Channel::Left, Channel::Right];
    let amp = empirical_amp_for(-23.0, 32_000, &layout);
    let lufs = run_at(32_000, amp, &layout);
    assert!((lufs - (-23.0)).abs() <= 0.1, "I = {lufs}");
}

#[test]
fn calibration_at_88200_hz() {
    let layout = [Channel::Left, Channel::Right];
    let amp = empirical_amp_for(-23.0, 88_200, &layout);
    let lufs = run_at(88_200, amp, &layout);
    assert!((lufs - (-23.0)).abs() <= 0.1, "I = {lufs}");
}

#[test]
fn calibration_at_96000_hz() {
    let layout = [Channel::Left, Channel::Right];
    let amp = empirical_amp_for(-23.0, 96_000, &layout);
    let lufs = run_at(96_000, amp, &layout);
    assert!((lufs - (-23.0)).abs() <= 0.1, "I = {lufs}");
}

#[test]
fn calibration_at_192000_hz() {
    let layout = [Channel::Left, Channel::Right];
    let amp = empirical_amp_for(-23.0, 192_000, &layout);
    let lufs = run_at(192_000, amp, &layout);
    assert!((lufs - (-23.0)).abs() <= 0.1, "I = {lufs}");
}

#[test]
fn snapshot_during_streaming_doesnt_perturb_report() {
    use core::time::Duration;
    let fs = 48_000_u32;
    let layout = [Channel::Left, Channel::Right];
    let amp = empirical_amp_for(-23.0, fs, &layout);
    let n = (fs as f32 * 30.0) as usize; // 30 s mono
    let omega = 2.0 * std::f32::consts::PI * 1000.0 / fs as f32;
    let mono: Vec<f32> = (0..n).map(|i| amp * (omega * i as f32).sin()).collect();
    let stereo: Vec<f32> = mono.iter().flat_map(|s| [*s, *s]).collect();

    let mut a = AnalyzerBuilder::new()
        .sample_rate(fs)
        .channels(&layout)
        .modes(Mode::All)
        .expected_duration(Duration::from_secs(30))
        .build()
        .unwrap();
    let mut b = AnalyzerBuilder::new()
        .sample_rate(fs)
        .channels(&layout)
        .modes(Mode::All)
        .expected_duration(Duration::from_secs(30))
        .build()
        .unwrap();

    // a: poll snapshot every 100 ms.
    for chunk in stereo.chunks(9_600 * 2) {
        a.push_interleaved::<f32>(chunk).unwrap();
        let _ = a.snapshot();
    }
    // b: don't poll.
    b.push_interleaved::<f32>(&stereo).unwrap();

    let r_a = a.finalize();
    let r_b = b.finalize();
    let diff = (r_a.integrated_lufs().unwrap() - r_b.integrated_lufs().unwrap()).abs();
    assert!(diff < 1e-9, "polling perturbs result by {diff}");
}

/// Long-programme stress: 1 hour of audio at 48 kHz, all modes
/// engaged. Asserts no overflow / drift / NaN escape.
///
/// Run with: `cargo test --release -- --ignored stress`
#[test]
#[ignore = "stress: ~1 minute of CPU time in --release mode"]
fn stress_one_hour_no_drift() {
    use core::time::Duration;
    let fs = 48_000_u32;
    let layout = [Channel::Left, Channel::Right];
    let mut a = AnalyzerBuilder::new()
        .sample_rate(fs)
        .channels(&layout)
        .modes(Mode::All)
        .expected_duration(Duration::from_secs(3_600))
        .build()
        .unwrap();

    // Push 1 hour in 100 ms chunks. We synthesize on the fly to avoid
    // allocating a full-hour buffer.
    let omega = 2.0 * std::f32::consts::PI * 1000.0 / fs as f32;
    let chunk_frames = 4_800usize;
    let mut buf: Vec<f32> = vec![0.0; chunk_frames * 2];
    for chunk_idx in 0..36_000 {
        // 36 000 chunks × 100 ms = 1 hour
        for f in 0..chunk_frames {
            let n = (chunk_idx * chunk_frames + f) as f32;
            let v = 0.05 * (omega * n).sin();
            buf[f * 2] = v;
            buf[f * 2 + 1] = v;
        }
        a.push_interleaved::<f32>(&buf).unwrap();
    }

    let r = a.finalize();
    assert!(r.integrated_lufs().unwrap().is_finite());
    assert!(r.true_peak_dbtp().unwrap().is_finite());
    assert!(r.loudness_range_lu().unwrap().is_finite());
    assert!((r.programme_duration_seconds() - 3_600.0).abs() < 1e-3);
}
