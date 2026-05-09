//! `06_axum_endpoint` — minimal HTTP service shape that wraps the analyzer.
//!
//! The SPEC originally suggested `axum` + `tokio`. To keep the example
//! tree free of heavyweight async dev-dependencies, this version uses
//! the standard library's `TcpListener` and a hand-rolled JSON
//! serializer. The integration *shape* — accept audio bytes, return a
//! [`Report`](ebur128_stream::Report) as JSON — is identical, and the
//! adapter to `axum` is a one-function swap.
//!
//! Run with: `cargo run --example 06_axum_endpoint`
//! POST a WAV file:
//!     curl --data-binary @sample.wav http://127.0.0.1:7878/analyze

use ebur128_stream::{AnalyzerBuilder, Channel, Mode, Report};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").expect("bind 127.0.0.1:7878");
    eprintln!("ebur128-stream demo service listening on 127.0.0.1:7878");
    eprintln!("POST a WAV file to /analyze; e.g.:");
    eprintln!("  curl --data-binary @sample.wav http://127.0.0.1:7878/analyze");

    for s in listener.incoming().take(8).flatten() {
        handle(s);
    }
}

fn handle(mut stream: TcpStream) {
    let mut buf = [0u8; 8 * 1024];
    let n = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return,
    };
    let req = String::from_utf8_lossy(&buf[..n]);
    let body_start = req.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
    let mut body = Vec::from(&buf[body_start..n]);
    while let Ok(extra) = stream.read(&mut buf) {
        if extra == 0 {
            break;
        }
        body.extend_from_slice(&buf[..extra]);
    }

    let response = match analyze_wav(&body) {
        Ok(r) => format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{}",
            report_to_json(&r)
        ),
        Err(e) => format!("HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\n\r\n{e}"),
    };
    let _ = stream.write_all(response.as_bytes());
}

fn analyze_wav(bytes: &[u8]) -> Result<Report, String> {
    use std::io::Cursor;
    let mut reader = hound::WavReader::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
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
    Ok(analyzer.finalize())
}

fn report_to_json(r: &Report) -> String {
    fn opt(v: Option<f64>) -> String {
        v.map(|x| format!("{x:.3}"))
            .unwrap_or_else(|| "null".into())
    }
    format!(
        "{{\
            \"integrated_lufs\":{},\
            \"loudness_range_lu\":{},\
            \"true_peak_dbtp\":{},\
            \"momentary_max_lufs\":{},\
            \"short_term_max_lufs\":{},\
            \"programme_duration_seconds\":{:.3}\
        }}",
        opt(r.integrated_lufs()),
        opt(r.loudness_range_lu()),
        opt(r.true_peak_dbtp()),
        opt(r.momentary_max_lufs()),
        opt(r.short_term_max_lufs()),
        r.programme_duration_seconds()
    )
}
