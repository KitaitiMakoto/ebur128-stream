//! `01_basic_lufs` — Synthetic 1 kHz sine → integrated LUFS readout.
//!
//! Run with: `cargo run --example 01_basic_lufs`

use ebur128_stream::{AnalyzerBuilder, Channel, Mode};

fn main() {
    const FS: u32 = 48_000;
    const SECONDS: f32 = 5.0;
    const AMP: f32 = 0.108; // ≈ -23 LUFS for a mono 1 kHz sine

    let two_pi_f_over_fs = 2.0 * std::f32::consts::PI * 1000.0 / FS as f32;
    let signal: Vec<f32> = (0..(FS as f32 * SECONDS) as usize)
        .map(|i| AMP * (two_pi_f_over_fs * i as f32).sin())
        .collect();

    let mut analyzer = AnalyzerBuilder::new()
        .sample_rate(FS)
        .channels(&[Channel::Center])
        .modes(Mode::Integrated | Mode::Momentary | Mode::ShortTerm | Mode::TruePeak)
        .build()
        .unwrap();

    analyzer.push_interleaved::<f32>(&signal).unwrap();
    let report = analyzer.finalize();

    println!("ebur128-stream — 5 s of 1 kHz sine @ 48 kHz, amp = {AMP}");
    println!("──────────────────────────────────────────────");
    println!(
        "  Integrated  : {:>8.2} LUFS",
        report.integrated_lufs().unwrap_or(f64::NAN)
    );
    println!(
        "  Momentary M : {:>8.2} LUFS",
        report.momentary_max_lufs().unwrap_or(f64::NAN)
    );
    println!(
        "  Short-term S: {:>8.2} LUFS",
        report.short_term_max_lufs().unwrap_or(f64::NAN)
    );
    println!(
        "  True peak   : {:>8.2} dBTP",
        report.true_peak_dbtp().unwrap_or(f64::NAN)
    );
    println!(
        "  Duration    : {:>8.2} s",
        report.programme_duration_seconds()
    );
}
