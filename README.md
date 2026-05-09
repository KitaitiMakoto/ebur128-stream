# ebur128-stream

> Streaming, zero-allocation EBU R128 loudness measurement in pure Rust.

[![crates.io](https://img.shields.io/crates/v/ebur128-stream.svg)](https://crates.io/crates/ebur128-stream)
[![docs.rs](https://docs.rs/ebur128-stream/badge.svg)](https://docs.rs/ebur128-stream)
[![CI](https://github.com/vanja/ebur128-stream/workflows/CI/badge.svg)](https://github.com/vanja/ebur128-stream/actions)
[![codecov](https://codecov.io/gh/vanja/ebur128-stream/branch/main/graph/badge.svg)](https://codecov.io/gh/vanja/ebur128-stream)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![MSRV 1.85](https://img.shields.io/badge/MSRV-1.85-blue.svg)](#)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](#)

```rust
use ebur128_stream::{AnalyzerBuilder, Channel, Mode};

let mut analyzer = AnalyzerBuilder::new()
    .sample_rate(48_000)
    .channels(&[Channel::Left, Channel::Right])
    .modes(Mode::Integrated | Mode::TruePeak)
    .build()?;

// Push samples — any chunk size, any push cadence. Determinism is
// guaranteed: pushing 1024 samples then 1024 samples produces the
// exact same Report as pushing all 2048 samples in one call.
analyzer.push_interleaved::<f32>(&samples)?;

let report = analyzer.finalize();
println!("Integrated: {:?} LUFS", report.integrated_lufs());
println!("True peak:  {:?} dBTP",  report.true_peak_dbtp());
# Ok::<(), ebur128_stream::Error>(())
```

---

## Why this exists

The Rust ecosystem has three loudness crates and they all leave a gap:

| Existing crate | What it does | The gap |
|---|---|---|
| [`ebur128`](https://crates.io/crates/ebur128) | Bindings to `libebur128` (C) | C dependency; pulls a `cc` build; allocates internally |
| [`bs1770`](https://crates.io/crates/bs1770) | Pure Rust ITU BS.1770 | K-weighting only, no R128 gating, no streaming |
| [`loudness`](https://crates.io/crates/loudness) | Wrapper around `ebur128` | Same FFI dependency |

`ebur128-stream` is what's missing:

- **Pure Rust, no FFI.** The whole filter, gating, true-peak FIR, and LRA stack is reimplemented in safe Rust.
- **Push-based streaming API.** Real-time pipelines drive the audio clock — they push 64-, 1024-, or arbitrary-frame chunks and read back current LUFS without buffering or reallocating.
- **Deterministic across chunk boundaries.** Pushing the same audio in any chunking produces bit-identical results. This is *the* contract that makes streaming usable; we have an example (`05_streaming_chunks`) that proves it.
- **Zero allocations on the hot path.** `push_planar` / `push_interleaved` allocate nothing in steady state. The programme buffer (Integrated / LRA only) grows once per `Vec` doubling and is documented.
- **`no_std` capable.** `--no-default-features` builds against `core` only. With `alloc` enabled but `std` off, you get the full feature set in a `no_std` environment.

## Concepts in 3 paragraphs

**Loudness ≠ amplitude.** A peak/RMS meter tells you signal level. A loudness meter tells you what the human ear *hears*. The K-weighting filter (BS.1770-4) approximates the ear's frequency response: a high-shelf around 1.7 kHz plus a high-pass at 38 Hz. After filtering, energy is summed across channels — with surround channels weighted +1.5 dB and the LFE channel excluded — and reported in **LUFS** (Loudness Units relative to Full Scale). Two signals at the same RMS can read 10 LU apart.

**Three windows, three measurements.** Momentary (M) is a sliding 400 ms window — for fast meters. Short-term (S) is 3 s — smoother, broadcast-style. Integrated (I) is the gated full-programme average: blocks below −70 LUFS are dropped (absolute gate), then blocks more than 10 LU below the surviving mean are dropped (relative gate). The remaining blocks' linear mean, expressed in LUFS, is "the" loudness number.

**True peak ≠ sample peak.** The largest absolute sample value undercounts the actual signal — peaks fall *between* samples after analog reconstruction. BS.1770 Annex 2 specifies a 4× polyphase FIR (12 taps × 4 phases) that approximates the reconstructed waveform; the max of that oversampled signal, in dBTP (decibels relative to full scale, true peak), is what limiters and broadcast specs care about.

## Trade-offs I made

- **Push-based API instead of pull-based.** Real-time consumers drive the audio clock; pull-based forces a buffer between caller and analyzer that defeats the streaming claim.
- **Programme buffer for Integrated and LRA is `Vec`-based.** Both measurements are statistical over the full programme; a fixed-capacity ring would cap programme length. This is why `Mode::Integrated` and `Mode::Lra` require the `alloc` feature.
- **`f32` internally even when input is `f64`.** EBU R128 specifies 24-bit precision; f32's 23-bit mantissa exceeds that. Empirically the f32-vs-f64 delta is ≤ 0.001 LU, well below the 0.1 LU spec tolerance.
- **No SIMD in V0.1.** LLVM auto-vectorisation gets the K-weighting biquad within ≈15 % of hand-written SIMD on x86_64 / aarch64. Adding `wide` for a 15 % gain is net complexity for V0.1.
- **`no_std` mode disables Integrated and LRA.** Both need a growable buffer; `--no-default-features` (no `alloc`) keeps M, S, and TruePeak only.

## Installation

```toml
[dependencies]
ebur128-stream = "0.1"
```

## Usage

### File analysis

```rust
use ebur128_stream::{AnalyzerBuilder, Channel, Mode};
use hound::WavReader;

let mut wav = WavReader::open("clip.wav")?;
let spec = wav.spec();
let samples: Vec<f32> = wav.samples::<f32>().collect::<Result<_, _>>()?;

let mut analyzer = AnalyzerBuilder::new()
    .sample_rate(spec.sample_rate)
    .channels(&[Channel::Left, Channel::Right])
    .modes(Mode::All)
    .build()?;
analyzer.push_interleaved(&samples)?;

let r = analyzer.finalize();
println!("I = {:.2} LUFS, LRA = {:.2} LU, TP = {:.2} dBTP",
    r.integrated_lufs().unwrap_or(f64::NAN),
    r.loudness_range_lu().unwrap_or(f64::NAN),
    r.true_peak_dbtp().unwrap_or(f64::NAN));
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Real-time monitor (poll snapshot at any cadence)

```rust
use ebur128_stream::{AnalyzerBuilder, Channel, Mode};

let mut analyzer = AnalyzerBuilder::new()
    .sample_rate(48_000)
    .channels(&[Channel::Left, Channel::Right])
    .modes(Mode::Momentary | Mode::ShortTerm | Mode::TruePeak)
    .build()?;

// In your audio callback:
// analyzer.push_interleaved(&audio_chunk)?;

// In your UI thread (e.g. 10 Hz):
let s = analyzer.snapshot();
let m = s.momentary_lufs().unwrap_or(f64::NEG_INFINITY);
# let _ = m;
# Ok::<(), ebur128_stream::Error>(())
```

### Multi-channel (5.1)

```rust
use ebur128_stream::{AnalyzerBuilder, Channel, Mode};

let mut a = AnalyzerBuilder::new()
    .sample_rate(48_000)
    .channels(&[
        Channel::Left, Channel::Right, Channel::Center,
        Channel::Lfe,
        Channel::LeftSurround, Channel::RightSurround,
    ])
    .modes(Mode::Integrated | Mode::TruePeak)
    .build()?;
# Ok::<(), ebur128_stream::Error>(())
```

## Architecture

```
input samples
     │
     ▼
┌─────────────┐    ┌───────────────────┐    ┌──────────────────────┐
│ K-weighting │───▶│ 100 ms block      │───▶│ Sliding M (400 ms)   │
│ biquad x2   │    │ aggregator        │    │ Sliding S (3 s)      │
│ per channel │    │ (zero-alloc)      │    │ Programme buffer     │
└─────────────┘    └───────────────────┘    │ (gated integrator)   │
       │                                     │ Short-term samples   │
       │                                     │ (LRA percentiles)    │
       ▼                                     └──────────────────────┘
┌─────────────┐
│ True-peak   │
│ 4× polyphase│───▶ programme max (dBTP)
│ FIR         │
└─────────────┘
```

Each box is one Rust module. Full discussion in
[ARCHITECTURE.md](ARCHITECTURE.md).

## Performance

Measured on Apple Silicon (M-series) at the time of release. `cargo bench`
runs the suite in `benches/throughput.rs`.

| Workload (1 s stereo @ 48 kHz) | Time | Throughput |
|---|--:|--:|
| `Mode::All` (everything) | ~850 µs | ~113 Melem/s |
| `Mode::Integrated` only | ~272 µs | ~353 Melem/s |
| Chunked at 64 frames     | ~855 µs | (no chunk-size penalty) |
| Chunked at 9600 frames   | ~858 µs | (no chunk-size penalty) |

The chunked rows demonstrate that streaming determinism doesn't cost
throughput — small and large chunks run at the same speed.

## Compliance

The library implements ITU-R BS.1770-4 K-weighting, R128 gating, and BS.1770 Annex 2 true-peak oversampling. Three independent test suites verify correctness:

- **[`tests/calibration.rs`](tests/calibration.rs)** — internal-consistency self-checks: 1 kHz sine at −23 LUFS reads −23 ± 0.1 LU; stereo is +3 dB above mono within 0.05 dB; surround applies +1.5 dB power weighting; LFE excluded; chunk-size determinism holds bitwise (1e-9).
- **[`tests/ebu_tech_3341.rs`](tests/ebu_tech_3341.rs)** — **14/14 EBU Tech 3341 § 3 compliance tests** with stimuli synthesised per the published procedure (the official wav vectors are not redistributable). Covers tests 1–8 (loudness calibration, gating, surround, snapshot/finalize parity) and 9–14 (true-peak inter-sample peak detection).
- **[`tests/cross_validate.rs`](tests/cross_validate.rs)** — direct comparison against the [`ebur128`](https://crates.io/crates/ebur128) reference Rust implementation: integrated agrees within 0.5 LU, true peak within 0.5 dBTP, LRA within 2 LU.

A fourth suite, **[`tests/no_alloc.rs`](tests/no_alloc.rs)**, installs a counting global allocator and asserts zero allocations during steady-state `push_*` calls (after `expected_duration` reservation) and zero allocations on cached `snapshot()` calls.

## Cargo features

| Feature | Default | Adds |
|---|---|---|
| `std` | ✓ | Standard library; off enables `no_std` mode |
| `alloc` | ✓ | `Vec`-backed Integrated and LRA |
| `f64` |   | Push `f64` samples (converted to `f32` internally) |
| `serde` |   | `Serialize` / `Deserialize` for `Snapshot`, `Report`, `Mode`, `Channel` |
| `tokio` |   | `AnalyzerSink: Sink<Vec<f32>>` for async streaming pipelines |

## Pre-reserving the programme buffer

For a fully zero-allocation steady state when using `Mode::Integrated` or `Mode::Lra`, hint at the programme length:

```rust
use core::time::Duration;
use ebur128_stream::{AnalyzerBuilder, Channel, Mode};

let analyzer = AnalyzerBuilder::new()
    .sample_rate(48_000)
    .channels(&[Channel::Left, Channel::Right])
    .modes(Mode::Integrated | Mode::Lra)
    .expected_duration(Duration::from_secs(60 * 90))   // 90-minute programme
    .build()?;
# Ok::<(), ebur128_stream::Error>(())
```

`tests/no_alloc.rs` proves this with a counting global allocator.

## Examples

Run with `cargo run --example <name>`:

| Example | Demonstrates |
|---|---|
| `01_basic_lufs` | Synthetic sine → integrated LUFS readout |
| `02_file_analysis` | WAV → full report (via `hound`) |
| `03_realtime_monitor` | Simulated streaming pattern at 10 Hz cadence |
| `04_multichannel_5_1` | 5.1 surround weighting and LFE exclusion |
| `05_streaming_chunks` | **Determinism proof** — identical results across chunk sizes |
| `06_axum_endpoint` | Tiny HTTP service wrapping the analyzer |

## Contributing

PRs welcome. Please:

1. Run `cargo test --all-features && cargo clippy --all-features --all-targets -- -D warnings && cargo fmt --check`.
2. Don't break determinism — `cargo run --example 05_streaming_chunks` must stay green.
3. Use [conventional commits](https://www.conventionalcommits.org).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
