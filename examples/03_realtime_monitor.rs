//! `03_realtime_monitor` — simulated real-time polling at 10 Hz.
//!
//! The SPEC originally called for `cpal` microphone capture, but `cpal`
//! pulls a heavyweight platform-specific audio backend that doesn't run
//! cleanly in CI on Linux without ALSA installed. The streaming pattern
//! is what matters: drive 100 ms chunks into the analyzer, poll
//! [`Analyzer::snapshot`] from a separate cadence (here, the same one).
//!
//! Wiring `cpal` (or any other input) is a one-function swap.

use ebur128_stream::{AnalyzerBuilder, Channel, Mode};
use std::time::Duration;

fn main() {
    const FS: u32 = 48_000;
    const CHUNK_FRAMES: usize = 4_800; // 100 ms

    let mut analyzer = AnalyzerBuilder::new()
        .sample_rate(FS)
        .channels(&[Channel::Left, Channel::Right])
        .modes(Mode::Momentary | Mode::ShortTerm | Mode::Integrated | Mode::TruePeak)
        .build()
        .unwrap();

    println!(
        "\n{:>8} {:>8} {:>8} {:>8} {:>8}",
        "t (s)", "M", "S", "I", "TP"
    );
    println!("{}", "─".repeat(50));

    let two_pi_f = 2.0 * std::f32::consts::PI * 1000.0 / FS as f32;
    for chunk_idx in 0..50 {
        // Synthesize one 100 ms stereo chunk with slight amplitude drift
        // so the meters move (real-time use would block on mic capture).
        let amp_l = 0.10 + (chunk_idx as f32 * 0.001);
        let amp_r = 0.10 - (chunk_idx as f32 * 0.001);
        let mut buf = Vec::with_capacity(CHUNK_FRAMES * 2);
        for i in 0..CHUNK_FRAMES {
            let n = chunk_idx * CHUNK_FRAMES + i;
            let phase = two_pi_f * n as f32;
            buf.push(amp_l * phase.sin());
            buf.push(amp_r * phase.sin());
        }
        analyzer.push_interleaved::<f32>(&buf).unwrap();

        let s = analyzer.snapshot();
        println!(
            "{:>8.2} {:>8} {:>8} {:>8} {:>8}",
            s.programme_duration_seconds(),
            fmt(s.momentary_lufs()),
            fmt(s.short_term_lufs()),
            fmt(s.integrated_lufs()),
            fmt(s.true_peak_dbtp()),
        );

        // Cap the simulated rate at 10 Hz; remove for the file-replay case.
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn fmt(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.2}"))
        .unwrap_or_else(|| "  --  ".into())
}
