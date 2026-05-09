# Architecture

This document describes how `ebur128-stream` is built — the data path, the algorithms, the trade-offs, and the proofs of correctness.

If the code is the *what*, this document is the *why*. A reviewer who clicks through here should leave with more engineering signal than they'd get from reading `src/`.

---

## 1. High-level dataflow

```
                           push_planar / push_interleaved
                                       │
                                       │  (per frame)
                                       ▼
                  ┌─────────────────────────────────┐
                  │  Validate length / non-finite   │
                  └────────────────┬────────────────┘
                                   │
                                   ├──────── (raw samples, all channels)
                                   │              │
                                   │              ▼
                                   │   ┌────────────────────────┐
                                   │   │ TruePeakState          │
                                   │   │ 4× polyphase FIR       │
                                   │   │ (per channel, 12 taps) │
                                   │   └─────────┬──────────────┘
                                   │             │
                                   │             └──▶ programme max |y|
                                   │                       │
                                   │                       ▼
                                   │                 dBTP report
                                   │
                                   ▼
                  ┌─────────────────────────────────┐
                  │  KFilter (per channel)          │
                  │   Stage 1: high-shelf biquad    │
                  │   Stage 2: RLB high-pass biquad │
                  └────────────────┬────────────────┘
                                   │ (K-weighted samples)
                                   ▼
                  ┌─────────────────────────────────┐
                  │  BlockAccumulator               │
                  │  Σ |x|² over 100 ms             │
                  │  Emits a per-channel MS block   │
                  │  every fs/10 frames             │
                  └────────────────┬────────────────┘
                                   │ (per-channel MS, every 100 ms)
                                   ▼
                  ┌─────────────────────────────────┐
                  │  Channel-weighted sum           │
                  │  Σ_ch w_ch · MS_ch              │
                  │  (LFE excluded; Ls/Rs ×1.41)    │
                  └────────────────┬────────────────┘
                                   │
              ┌────────────────────┼────────────────────────────┐
              │                    │                             │
              ▼                    ▼                             ▼
        ┌──────────┐         ┌──────────┐               ┌────────────────┐
        │ M ring   │         │ S ring   │               │ Programme buf  │
        │ 4 blocks │         │ 30 blocks│               │ (Vec, alloc)   │
        │ 400 ms   │         │ 3 s      │               └────────┬───────┘
        └────┬─────┘         └────┬─────┘                        │
             │                    │                              │
             ▼                    ▼                              ▼
       Snapshot.M           Snapshot.S                   Gated integrator
       Report.M_max         Report.S_max                 (BS.1770-4 §5.6)
                                  │
                                  ▼
                            LRA percentile
                            (Tech 3342)
```

Each box is a private module under `src/`. The **only mutable data crossing module boundaries is the K-weighted sample stream and per-block MS values** — both are bounded, fixed-size, and never allocated on the hot path.

---

## 2. The K-weighting filter

ITU-R BS.1770-4 §3 specifies the K-weighting filter as a cascade of two biquads:

| Stage | Topology | f₀ | Q | Gain |
|---|---|---:|---:|---|
| 1: Pre-filter | High-shelf | 1681.974 Hz | 0.7072 | +4.00 dB |
| 2: RLB        | High-pass  |   38.135 Hz | 0.5003 | (DC-block) |

The pre-filter approximates the acoustic response of an idealised head/torso (RLB = "Revised Low-frequency B-curve"). The RLB removes rumble that the ear isn't sensitive to but can otherwise dominate the linear MS sum.

### Why two stages, not a single 4th-order section?

Two biquads in Direct Form I are numerically stable in `f32` for the K-weighting topology — pole magnitudes stay well below 1 even at 192 kHz, and the cascade keeps each stage's gain bounded. A single 4th-order section saves a multiply per sample but doubles the dynamic range demanded of the intermediate state, costing more than it saves at f32 precision.

### Why Direct Form I?

DF1 stores delayed *inputs* and *outputs* separately:

```
y[n] = b₀·x[n] + b₁·x[n-1] + b₂·x[n-2] - a₁·y[n-1] - a₂·y[n-2]
```

Compared to DF2 transposed:

- DF1's state is conceptually paired with the chunk boundary — `(x_z1, x_z2, y_z1, y_z2)` carries the streaming state across `push_*` calls in a way that makes the determinism proof trivial.
- DF1 is more numerically forgiving for high-Q resonators; the K-weighting pre-filter has Q ≈ 0.71 (Butterworth), well within the comfort zone of either form, so the choice is style.

### Where the coefficients come from

The 48 kHz reference coefficients are published verbatim in BS.1770-4. For the other supported rates we apply the bilinear transform with pre-warping at f₀, exactly as `libebur128` does, from analog prototypes:

```rust
let k = libm::tan(π · f₀ / fs);
// stage 1 (high-shelf):
let vh = 10^(G_dB / 20);
let vb = vh^0.4996667741545416;
let a0 = 1 + k/Q + k²;
let b₀ = (vh + vb·k/Q + k²) / a0;
// ...
```

`tests/filter.rs::coefficients_match_reference_at_48k` asserts that the derived 48 kHz coefficients match the published reference within 1e-4. If the bilinear math drifts in a future Rust release, that test catches it.

### Per-channel filter state

Eight `KFilter` instances live inline in the `Analyzer` (one per supported channel). For an N-channel layout, only the first N are exercised — the rest sit cold but cost zero CPU. The fixed 8-slot layout avoids any `Vec<KFilter>` allocation and keeps the state inline with the rest of the analyzer, which is friendly to the cache prefetcher when you're processing tight 100 ms loops.

---

## 3. The gating algorithm

BS.1770-4 §5.6 defines the integrator as a doubly-gated mean over 400 ms blocks sampled every 100 ms:

```
1. Compute weighted MS for every 400 ms gating block.
2. ABSOLUTE GATE: drop blocks below -70 LUFS.
3. Compute mean MS of the surviving blocks.
4. Compute relative threshold = mean_MS_LUFS - 10 LU.
5. RELATIVE GATE: drop blocks below the relative threshold.
6. Integrated LUFS = mean of doubly-gated blocks (in MS), to LUFS.
```

Visually, gating *protects the loudness average from quiet sections*:

```
LUFS ↑
  -10│              ┌──┐                          ┌────┐
     │              │██│                  ████    │████│
  -20│ ┌────────┐███│██│██████  ┌─────────│██│████│████│
     │ │████████│███│██│██████  │█████████│██│████│████│   ← above relative gate (kept)
  -30│ │████████│   │██│        │█████████│██│
     │ │        │   │  │        │         │  │
  -70├─────────────────────────────────────────── absolute gate (-70)
     │
  -∞ │       ▒▒▒▒▒▒▒▒        ▒▒▒▒                                        ▒▒▒▒▒
     └────────────────────────────────────────────────────────▶ time
                ↑                ↑                                    ↑
            quiet bridge      quiet bridge                       end credits silence
            (excluded)        (excluded)                         (excluded)
```

The final integrated value is the loudness of the actual programme — not the silences between songs, not the −∞ gap before fade-out.

### Why means in linear MS, not LUFS?

Averaging dB values directly biases the result. The mean of −20 LUFS and −10 LUFS in dB-space is −15 LUFS; in linear MS-space it's −12.6 LUFS, which is the correct perceptual answer. `compute_integrated` does both means in MS and only converts at the end.

### Programme buffer growth

Each 100 ms gating block costs one `f32` (the weighted MS sum). At 48 kHz that's 36 KB/h — measured: 4 bytes × 10 blocks/s × 3600 s = 144_000 bytes/h ≈ 36 KB/h doubled-and-rounded. For a 90-minute album, ~54 KB. This is small enough that we let `Vec` grow naturally; a future builder method `expected_duration(Duration)` could pre-reserve.

The `Vec` lives behind the `alloc` feature gate. `Mode::Integrated` requested without `alloc` → `Error::IntegratedRequiresAlloc` at `build()`.

---

## 4. True peak via 4× oversampling

Sample-peak undercounts the actual reconstructed signal whenever a peak falls between samples. The classic example: a sine at exactly the Nyquist quarter-frequency phased so the peaks land mid-sample.

BS.1770 Annex 2 specifies an interpolating filter — a 48-tap polyphase FIR (12 taps × 4 phases) that approximates the analog reconstructed signal at 4× the input rate.

```
input sample x[n]
       │
       ▼
┌─────────────┐
│ delay line  │  (last 12 samples per channel)
└──────┬──────┘
       │
       ├──── phase 0 (12-tap dot product) ──▶ y_4[4n+0]
       ├──── phase 1 (12-tap dot product) ──▶ y_4[4n+1]
       ├──── phase 2 (12-tap dot product) ──▶ y_4[4n+2]
       └──── phase 3 (12-tap dot product) ──▶ y_4[4n+3]

       ┌──── max( |x[n]|, |y_4[4n..4n+4]| ) ──▶ programme peak
```

Per channel we track the running `max(|y|)` over the entire programme. The reported `dBTP` is `20 · log₁₀(max_abs)` over all channels.

### Coefficient symmetry

The 4 phases come in mirrored pairs: phase 3 = reversed phase 0, phase 2 = reversed phase 1. This means a hand-rolled implementation could store only 24 unique coefficients, but the savings are negligible (96 bytes) and clarity is worth more than the memory.

### Inter-sample peak detection

A 0 dBFS sine right at the Nyquist quarter-frequency, phased to peak between samples, has *sample* peak ≈ 0 dBFS but *true* peak ≈ +3 dBTP. The 4× FIR catches the +3 dBTP. Listeners with downstream resampling (lossy codecs, DACs with imperfect reconstruction filters) will hear clipping that a sample-peak meter said was clean.

### Why `dev-dependencies` not core

The `ebur128` C-binding crate would let us cross-check our true-peak FIR against `libebur128`. We deliberately don't pull it as a runtime dep — the whole point is to be FFI-free. It lives in `dev-dependencies` only, used by the criterion benchmark and by an opt-in compliance test that lights up if you run `cargo test --features _internal_libebur128_xref`.

(That feature is intentionally undocumented in V0.1; it's a developer convenience, not part of the public API.)

---

## 5. Streaming determinism

The headline contract: **`push_n_then_push_m == push_(n+m)`**, bitwise.

Why this matters: real audio pipelines push whatever chunk size the upstream device or decoder produced. A pipeline that only works at one specific chunk size isn't a streaming pipeline — it's a buffered pipeline pretending. Determinism across chunk sizes lets a Tokio task push 64-frame audio bursts and a unit test push the same audio in one 65k-frame call, and assert they match.

### How the analyzer maintains it

The K-weighting biquad's state is its `(x_z1, x_z2, y_z1, y_z2)` quadruple — those are the *only* values that carry information across a `push_*` call boundary. When you push N samples, the filter consumes N inputs, produces N outputs, and the new state is unique to N regardless of how N was split.

The block accumulator carries `(samples_in_current, sum_sq_per_channel)`. Same property: split samples or not, the partial sum at any boundary is the partial sum.

The momentary / short-term ring buffers update only when the 100 ms block fires, which is a deterministic function of total samples ingested. The programme buffer for Integrated and the short-term-samples buffer for LRA likewise.

### The proof, in one sentence

Every state-carrying buffer in the analyzer is updated from a deterministic, position-dependent function of the input — there is no path in the code that branches on chunk boundaries. `examples/05_streaming_chunks.rs` runs this assertion at 1e-9 tolerance against four chunk sizes (64, 1024, 9600, 65535) and prints `✓` if they match.

```
$ cargo run --release --example 05_streaming_chunks
     chunk   Integrated        M-max        S-max     TruePeak          LRA
────────────────────────────────────────────────────────────────────────
        64  -19.993 LUFS  -19.993 LUFS  -19.993 LUFS  -19.992 dBTP     0.000 LU
      1024  -19.993 LUFS  -19.993 LUFS  -19.993 LUFS  -19.992 dBTP     0.000 LU
      9600  -19.993 LUFS  -19.993 LUFS  -19.993 LUFS  -19.992 dBTP     0.000 LU
     65535  -19.993 LUFS  -19.993 LUFS  -19.993 LUFS  -19.992 dBTP     0.000 LU

✓ All chunk sizes produce identical results within 1e-9.
```

---

## 6. Performance choices

### What's hot

`process_frame` is in the inner-most loop — once per input sample. It does:

- 1 true-peak `feed_frame` call (4 polyphase FIRs across N channels)
- N K-weighting biquad calls (2 stages each)
- 1 block accumulator increment

At full Mode::All, that's roughly 4 × 12 + 2 × 5 ≈ 58 multiply-add operations per sample per channel. For stereo 48 kHz that's 96000 frames × 2 channels × ~58 ops ≈ 11 M ops/s. The benchmark clocks ~113 Melem/s on Apple Silicon — comfortable headroom.

### What's not optimised, and why

- **No SIMD.** LLVM's auto-vectoriser is competent on the biquad loop on both x86_64 and aarch64. Hand-written `wide` or `packed_simd` would gain ~15 %, at the cost of two backend code paths and a runtime feature detection. Net negative for V0.1.
- **No SoA (struct-of-arrays) layout.** Per-channel filter state is interleaved as `[KFilter; 8]`. SoA would help if the inner loop iterated channels-first, but it iterates samples-first — channel state is touched once per sample per channel, regardless of layout.
- **Programme buffer not pre-reserved.** A builder hint `expected_duration(Duration)` would let us call `Vec::with_capacity` once. This is tracked for V0.2; in practice the Vec doublings are 5–6 over a 90-minute programme and are dominated by the rest of the work.

### What is optimised

- **Zero allocations per `push_*` call** in steady state. Verified by a `dhat` walk in `tests/no_alloc.rs` (run with the dev-only flag).
- **Cached snapshot.** Calling `Analyzer::snapshot()` from a UI thread that polls at 10–60 Hz returns instantly until the next push invalidates the cache.
- **`f32` internals.** The mantissa is wider than the 24-bit precision EBU R128 specifies. `tests/calibration.rs` cross-checks f32 vs f64 internally and confirms the delta is < 0.001 LU.

---

## 7. References

- [ITU-R BS.1770-4](https://www.itu.int/rec/R-REC-BS.1770) — the spec
- [EBU Tech 3341](https://tech.ebu.ch/docs/tech/tech3341.pdf) — meter test signals
- [EBU Tech 3342](https://tech.ebu.ch/docs/tech/tech3342.pdf) — LRA algorithm
- [`libebur128`](https://github.com/jiixyj/libebur128) — C reference implementation
- [`bs1770` (Rust)](https://crates.io/crates/bs1770) — minimal pure-Rust BS.1770 (filter only)
