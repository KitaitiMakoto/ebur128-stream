# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

— nothing pending; the SPEC anti-scope was deliberately overridden in
v0.2 to add CLI / normalization / resampling / SVG VU meter /
24-channel layouts / SIMD / WASM / Python.

## [0.2.0] — 2026-05-09

### Breaking

- **MAX_CHANNELS bumped from 8 → 24** to accommodate 22.2 immersive
  surround. The `Analyzer` API is the same; layouts > 8 channels now
  build successfully where they previously returned
  `Error::InvalidChannelLayout`.

### Added — anti-scope expansion (deliberate, per request)

The original SPEC §15 listed CLI, normalization, resampling, and
visualization as out of scope forever. v0.2 ships them as **opt-in
Cargo features** so the core crate stays lean while the feature-rich
surface is available to callers who want it.

| Feature | What it adds |
|---|---|
| `cli` | `lufs` binary — read WAV, print Report (human / JSON / SVG) with optional gain-to-target output |
| `normalize` | `Normalizer` type + `gain_to_target()` helper, with optional true-peak ceiling |
| `resampler` | `Resampler` type wrapping `rubato` (Linear + HighQuality sinc) for arbitrary input rates |
| `svg` | `render_dynamic_vumeter()` + `render_timeseries_vumeter()` — animated SVG VU meters matching the FestivalPlayout style (vertical segmented bars, peak-hold ballistics, SMIL animation, no JS required) |
| `simd` | `wide`-based SIMD true-peak FIR — measured ~19 % faster on `Mode::All` (~850 µs → ~683 µs at 1 s stereo @ 48 kHz) |
| `wasm` | `wasm-bindgen` wrapper exposing `WasmAnalyzer` to browsers — compiles to a self-contained `.wasm` artifact |

Plus:

- **24-channel support** including a 22.2 immersive surround test.
- **Python bindings skeleton** (`bindings/python/`, pyo3 0.22, abi3-py39) — Cargo.toml + lib.rs + README ready; binary build requires maturin + Python dev headers locally.

### Note

The library kept the things the SPEC was right about: zero unsafe,
zero allocations on the hot path with `expected_duration` reservation,
EBU 3341 14/14, cross-validated against the `ebur128` reference.

[0.2.0]: https://github.com/vanja/ebur128-stream/releases/tag/v0.2.0

## [0.1.2] — 2026-05-09

### Added

- **Property-based tests** with `proptest` (`tests/properties.rs`): determinism across random chunkings, no-NaN/inf escape, reset idempotency, programme-duration linearity, channel-weight invariance, NonFiniteSample rejection.
- **`#![forbid(unsafe_code)]`** at the crate root — the library contains zero `unsafe` blocks.
- **22.05 kHz and 32 kHz sample rates** added to the supported set (telephony / streaming-codec rates). Total now 7 rates: 22.05 / 32 / 44.1 / 48 / 88.2 / 96 / 192 kHz.
- **1-hour stress test** (`tests/extended.rs::stress_one_hour_no_drift`, run with `cargo test --release -- --ignored stress`) verifying no drift, overflow, or NaN over an extreme programme.
- Per-rate calibration tests for every supported sample rate.
- Snapshot-during-streaming non-perturbation test: polling `Snapshot` mid-programme does not change the final `Report`.
- Per-method doctests on every behaviour-bearing public method (push_planar, push_interleaved, snapshot, finalize, reset, expected_duration, weight).
- CI: `cargo deny check` job, `cargo-llvm-cov` coverage job uploading to Codecov.
- "unsafe forbidden" + Codecov badges in README.

### Changed

- **MSRV bumped from 1.74 to 1.85** (deliberately, per SPEC §11) so the library can use the safe `Waker::noop()` (Rust 1.85+) and remain `#![forbid(unsafe_code)]`. Users on older toolchains should pin to v0.1.1.

## [0.1.1] — 2026-05-09

### Added

- **EBU Tech 3341 compliance suite** — 14 tests in `tests/ebu_tech_3341.rs`, all passing. Covers tests 1–8 (loudness calibration, gating, surround, snapshot/finalize parity) and 9–14 (true-peak inter-sample peak detection).
- **Cross-validation against the `ebur128` reference crate** in `tests/cross_validate.rs`: integrated within 0.5 LU, true peak within 0.5 dBTP, LRA within 2 LU.
- **Zero-allocation steady-state proof** in `tests/no_alloc.rs` via a counting global allocator. Confirms `push_*` allocates zero bytes after the programme buffer has been reserved, and `snapshot()` is zero-alloc when cached.
- `AnalyzerBuilder::expected_duration(Duration)` — pre-reserves the integrated and LRA programme buffers so steady-state pushes never trigger Vec growth.
- `AnalyzerSink: futures_sink::Sink<Vec<f32>>` under the new `tokio` feature. Compatible with any futures-based executor; the crate depends only on `futures-sink`, not `tokio` itself.
- Architecture SVG asset at `docs/architecture.svg`.
- VHS tape file at `docs/demo.tape` for reproducible README demo GIF rendering.

## [0.1.0] — 2026-05-09

### Added

- Pure-Rust streaming EBU R128 loudness analyzer
- ITU-R BS.1770-4 K-weighting filter (cascaded high-shelf + RLB high-pass biquads, Direct Form I)
- 100 ms block aggregator with deterministic chunk-boundary handling
- Momentary (400 ms) and Short-Term (3 s) sliding windows
- Gated integrated loudness per BS.1770-4 §5.6 (absolute −70 LUFS, relative −10 LU)
- Loudness Range (LRA) per EBU Tech 3342 (95th − 10th percentile after double gating)
- True-peak measurement via 4× polyphase FIR per BS.1770 Annex 2 (12 taps × 4 phases)
- Push-based API: `push_planar` (slice-of-channels) and `push_interleaved`
- Sample formats: `f32` always, `f64` under the `f64` feature (converted to f32 internally)
- Sample rates: 44.1, 48, 88.2, 96, 192 kHz validated at builder time
- Multi-channel layouts up to 8 channels with BS.1770 channel weighting (LFE excluded, Ls/Rs +1.5 dB)
- Optional `serde` feature for `Snapshot`, `Report`, `Mode`, `Channel`
- `no_std` capable via `--no-default-features` (Momentary / Short-term / TruePeak only — Integrated and LRA require `alloc`)
- Six examples (`01_basic_lufs` … `06_axum_endpoint`)
- Criterion benchmark suite (`benches/throughput.rs`)
- Calibration self-tests cross-checking sine-at-23 LUFS readback, stereo-vs-mono +3 dB, surround weighting, LFE exclusion, chunk-size determinism

### Notes

- MSRV: 1.74
- License: MIT OR Apache-2.0
- Independent verification against EBU Tech 3341 test vectors is in progress; calibration self-tests in `tests/calibration.rs` provide internal-consistency proofs in the meantime

[Unreleased]: https://github.com/vanja/ebur128-stream/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/vanja/ebur128-stream/releases/tag/v0.1.0
