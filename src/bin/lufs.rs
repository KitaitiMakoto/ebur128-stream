//! `lufs` — command-line loudness analyzer.
//!
//! Usage:
//!   lufs <file.wav>             # analyse a WAV file
//!   lufs -                      # analyse a WAV from stdin
//!   lufs --json <file.wav>      # output JSON instead of human-readable
//!   lufs --target -23 <file>    # also print the gain to reach -23 LUFS
//!   lufs --svg <file>           # write an SVG meter to stdout
//!   lufs --version

use ebur128_stream::{AnalyzerBuilder, Channel, Mode, Report};
use std::io::Read;
use std::process::ExitCode;

#[derive(Default)]
struct Args {
    path: Option<String>,
    json: bool,
    svg: bool,
    target_lufs: Option<f64>,
    show_help: bool,
    show_version: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args::default();
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => a.show_help = true,
            "-V" | "--version" => a.show_version = true,
            "--json" => a.json = true,
            "--svg" => a.svg = true,
            "--target" | "-t" => {
                let v = iter.next().ok_or("--target needs a value")?;
                a.target_lufs = Some(v.parse().map_err(|e| format!("--target: {e}"))?);
            }
            other => {
                if a.path.is_some() {
                    return Err(format!("unexpected argument: {other}"));
                }
                a.path = Some(other.to_string());
            }
        }
    }
    Ok(a)
}

fn print_help() {
    println!(
        "lufs — EBU R128 / BS.1770-4 loudness analyzer

USAGE:
  lufs <file.wav>           Analyse a WAV file.
  lufs -                    Read WAV from stdin.

FLAGS:
  -h, --help                Show this help.
  -V, --version             Show version.
      --json                Emit JSON instead of human-readable text.
      --svg                 Emit an SVG meter to stdout.
  -t, --target <LUFS>       Also print the gain change required to
                            normalise the programme to <LUFS>.

OUTPUT (default):
  Integrated, Loudness Range, True Peak, M-max, S-max, duration."
    );
}

fn read_input(path: &str) -> Result<Vec<u8>, String> {
    if path == "-" {
        let mut buf = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buf)
            .map_err(|e| e.to_string())?;
        Ok(buf)
    } else {
        std::fs::read(path).map_err(|e| format!("{path}: {e}"))
    }
}

fn analyze(bytes: &[u8]) -> Result<(Report, u32, u16), String> {
    let mut reader = hound::WavReader::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("not a WAV: {e}"))?;
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
    let layout: Vec<Channel> = match spec.channels {
        1 => vec![Channel::Center],
        2 => vec![Channel::Left, Channel::Right],
        n => (0..n).map(|_| Channel::Other).collect(),
    };
    let mut analyzer = AnalyzerBuilder::new()
        .sample_rate(spec.sample_rate)
        .channels(&layout)
        .modes(Mode::All)
        .build()
        .map_err(|e| e.to_string())?;
    analyzer
        .push_interleaved::<f32>(&samples)
        .map_err(|e| e.to_string())?;
    Ok((analyzer.finalize(), spec.sample_rate, spec.channels))
}

fn fmt_human(r: &Report, fs: u32, channels: u16, target: Option<f64>) -> String {
    let mut s = String::new();
    s.push_str(&format!("file:        {fs} Hz, {channels} channel(s)\n"));
    s.push_str(&format!(
        "duration:    {:>8.2} s\n",
        r.programme_duration_seconds()
    ));
    s.push_str(&format!(
        "integrated:  {:>8} LUFS\n",
        opt_fmt(r.integrated_lufs())
    ));
    s.push_str(&format!(
        "LRA:         {:>8} LU\n",
        opt_fmt(r.loudness_range_lu())
    ));
    s.push_str(&format!(
        "true peak:   {:>8} dBTP\n",
        opt_fmt(r.true_peak_dbtp())
    ));
    s.push_str(&format!(
        "M max:       {:>8} LUFS\n",
        opt_fmt(r.momentary_max_lufs())
    ));
    s.push_str(&format!(
        "S max:       {:>8} LUFS\n",
        opt_fmt(r.short_term_max_lufs())
    ));
    if let (Some(t), Some(i)) = (target, r.integrated_lufs()) {
        let gain_db = t - i;
        s.push_str(&format!(
            "→ to reach {t:.1} LUFS: apply {gain_db:+.2} dB ({} dB headroom)\n",
            (-r.true_peak_dbtp().unwrap_or(0.0) - gain_db).round() as i32
        ));
    }
    s
}

fn opt_fmt(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("{x:.2}"),
        None => "  --  ".into(),
    }
}

fn fmt_json(r: &Report) -> String {
    fn opt(v: Option<f64>) -> String {
        v.map(|x| format!("{x:.6}"))
            .unwrap_or_else(|| "null".into())
    }
    format!(
        "{{\"integrated_lufs\":{},\"loudness_range_lu\":{},\"true_peak_dbtp\":{},\"momentary_max_lufs\":{},\"short_term_max_lufs\":{},\"programme_duration_seconds\":{:.6}}}\n",
        opt(r.integrated_lufs()),
        opt(r.loudness_range_lu()),
        opt(r.true_peak_dbtp()),
        opt(r.momentary_max_lufs()),
        opt(r.short_term_max_lufs()),
        r.programme_duration_seconds()
    )
}

#[cfg(feature = "svg")]
fn fmt_svg(r: &Report) -> String {
    ebur128_stream::svg::render_dynamic_vumeter(r)
}

#[cfg(not(feature = "svg"))]
fn fmt_svg(_: &Report) -> String {
    "<!-- the `svg` feature was not enabled at build time -->".to_string()
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    if args.show_help {
        print_help();
        return Ok(());
    }
    if args.show_version {
        println!("lufs {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let path = args
        .path
        .as_deref()
        .ok_or("no input file (try `lufs --help` or pipe a WAV: `cat clip.wav | lufs -`)")?;
    let bytes = read_input(path)?;
    let (report, fs, channels) = analyze(&bytes)?;
    if args.json {
        print!("{}", fmt_json(&report));
    } else if args.svg {
        print!("{}", fmt_svg(&report));
    } else {
        print!("{}", fmt_human(&report, fs, channels, args.target_lufs));
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("lufs: {e}");
            ExitCode::FAILURE
        }
    }
}
