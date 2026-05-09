//! `02_file_analysis` — Read a WAV via `hound`, run full EBU R128 analysis.
//!
//! Usage: `cargo run --example 02_file_analysis -- path/to/file.wav`
//!
//! When invoked with no path, the example synthesises 5 s of a 1 kHz
//! mono sine for self-contained demonstration.

use ebur128_stream::{AnalyzerBuilder, Channel, Mode};

fn main() {
    let arg = std::env::args().nth(1);
    let (rate, channels, samples_f32) = match arg {
        Some(p) => read_wav(&p).unwrap_or_else(|e| {
            eprintln!("error reading {p}: {e}");
            std::process::exit(1);
        }),
        None => {
            eprintln!("(no path supplied — using synthetic 1 kHz mono sine)");
            synthesize_demo()
        }
    };

    let layout: Vec<Channel> = match channels {
        1 => vec![Channel::Center],
        2 => vec![Channel::Left, Channel::Right],
        n => (0..n).map(|_| Channel::Other).collect(),
    };

    let mut analyzer = AnalyzerBuilder::new()
        .sample_rate(rate)
        .channels(&layout)
        .modes(Mode::All)
        .build()
        .unwrap_or_else(|e| {
            eprintln!("could not build analyzer: {e}");
            std::process::exit(1);
        });

    analyzer.push_interleaved::<f32>(&samples_f32).unwrap();
    let report = analyzer.finalize();

    println!("File analysis — {rate} Hz, {channels} channel(s)");
    println!("─────────────────────────────────────");
    print_field("Integrated", report.integrated_lufs(), "LUFS");
    print_field("LRA       ", report.loudness_range_lu(), "LU  ");
    print_field("True peak ", report.true_peak_dbtp(), "dBTP");
    print_field("M max     ", report.momentary_max_lufs(), "LUFS");
    print_field("S max     ", report.short_term_max_lufs(), "LUFS");
    println!(
        "Duration    : {:>8.2} s",
        report.programme_duration_seconds()
    );
}

fn print_field(label: &str, v: Option<f64>, unit: &str) {
    match v {
        Some(x) => println!("{label}  : {x:>8.2} {unit}"),
        None => println!("{label}  : {:>8} {unit}", "  --  "),
    }
}

fn read_wav(path: &str) -> Result<(u32, u16, Vec<f32>), String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?,
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max))
                .collect::<Result<_, _>>()
                .map_err(|e| e.to_string())?
        }
    };
    Ok((spec.sample_rate, spec.channels, samples))
}

fn synthesize_demo() -> (u32, u16, Vec<f32>) {
    const FS: u32 = 48_000;
    let two_pi_f = 2.0 * std::f32::consts::PI * 1000.0 / FS as f32;
    let signal: Vec<f32> = (0..(FS as usize) * 5)
        .map(|i| 0.108 * (two_pi_f * i as f32).sin())
        .collect();
    (FS, 1, signal)
}
