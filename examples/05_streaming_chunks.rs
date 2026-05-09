//! `05_streaming_chunks` — proof of streaming determinism.
//!
//! The same audio is fed through analyzers configured with different
//! chunk sizes. All measurements (integrated, momentary max, short-term
//! max, true peak) are reported and asserted to match within `1e-9`.
//!
//! This example backs the "deterministic regardless of chunk size"
//! claim made by the README.

use ebur128_stream::{AnalyzerBuilder, Channel, Mode, Report};

fn main() {
    const FS: u32 = 48_000;
    const SECONDS: f32 = 10.0;
    const AMP: f32 = 0.10;

    let n = (FS as f32 * SECONDS) as usize;
    let two_pi_f = 2.0 * std::f32::consts::PI * 1000.0 / FS as f32;
    // Stereo (L=R) interleaved sine.
    let mut signal = Vec::with_capacity(n * 2);
    for i in 0..n {
        let v = AMP * (two_pi_f * i as f32).sin();
        signal.push(v);
        signal.push(v);
    }

    let chunk_frames = [64usize, 1_024, 9_600, 65_535];

    println!(
        "{:>10}  {:>11}  {:>11}  {:>11}  {:>11}  {:>11}",
        "chunk", "Integrated", "M-max", "S-max", "TruePeak", "LRA"
    );
    println!("{}", "─".repeat(72));

    let mut reports: Vec<Report> = Vec::new();
    for &cf in &chunk_frames {
        let mut a = AnalyzerBuilder::new()
            .sample_rate(FS)
            .channels(&[Channel::Left, Channel::Right])
            .modes(Mode::All)
            .build()
            .unwrap();
        let cs = cf * 2; // frames → interleaved samples
        for c in signal.chunks(cs) {
            a.push_interleaved::<f32>(c).unwrap();
        }
        let r = a.finalize();
        println!(
            "{cf:>10}  {:>11}  {:>11}  {:>11}  {:>11}  {:>11}",
            fmt(r.integrated_lufs(), "LUFS"),
            fmt(r.momentary_max_lufs(), "LUFS"),
            fmt(r.short_term_max_lufs(), "LUFS"),
            fmt(r.true_peak_dbtp(), "dBTP"),
            fmt(r.loudness_range_lu(), "LU"),
        );
        reports.push(r);
    }

    let r0 = reports[0];
    let mut all_match = true;
    for (i, r) in reports.iter().enumerate().skip(1) {
        if !approx(r0.integrated_lufs(), r.integrated_lufs())
            || !approx(r0.momentary_max_lufs(), r.momentary_max_lufs())
            || !approx(r0.short_term_max_lufs(), r.short_term_max_lufs())
            || !approx(r0.true_peak_dbtp(), r.true_peak_dbtp())
            || !approx(r0.loudness_range_lu(), r.loudness_range_lu())
        {
            eprintln!(
                "✗ chunk {} differs from chunk {}",
                chunk_frames[i], chunk_frames[0]
            );
            all_match = false;
        }
    }
    if all_match {
        println!("\n✓ All chunk sizes produce identical results within 1e-9.");
    } else {
        eprintln!("\n✗ Determinism violated.");
        std::process::exit(1);
    }
}

fn approx(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => (x - y).abs() < 1e-9,
        _ => false,
    }
}

fn fmt(v: Option<f64>, unit: &str) -> String {
    v.map(|x| format!("{x:>6.3} {unit}"))
        .unwrap_or_else(|| format!("{:>6} {unit}", "--"))
}
