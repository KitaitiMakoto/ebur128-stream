# `ebur128-stream` — Library & Documentation Spec

**Working name:** `ebur128-stream` (placeholder — see [§13 Naming](#13-naming))
**Status:** Draft v0.1
**Target ship:** 2 weekends (~16h library + ~6h documentation)
**Author:** Vanja Modrinjak

---

## 1. Goal

A pure-Rust, zero-allocation, streaming implementation of EBU R128 loudness measurement, published to crates.io with documentation that **functions as the writeup** for a blog post you didn't write.

This is two artifacts in one repo:

1. **The library** — a small, sharp Rust crate for streaming loudness analysis
2. **The documentation** — README + docs.rs page + examples that explain *why decisions were made*, not just *how to use it*

The documentation is not an afterthought. It is treated as a first-class deliverable equivalent in importance to the code.

---

## 2. Why this exists (the crates.io gap)

| Existing crate | What it does | Gap it leaves |
|---|---|---|
| [`ebur128`](https://crates.io/crates/ebur128) | Rust bindings to libebur128 (C) | C dependency; pull-based; allocates internally |
| [`bs1770`](https://crates.io/crates/bs1770) | Pure Rust ITU BS.1770 | Implements only K-weighting, no R128 gating, no streaming API |
| [`loudness`](https://crates.io/crates/loudness) | Wrapper around `ebur128` | Same FFI dependency |

What's missing on crates.io:

- **Pure Rust** (no libebur128 FFI)
- **Push-based streaming API** that accepts arbitrary chunk sizes deterministically
- **Zero allocations in the hot path** (chunk-processing fn allocates nothing)
- **`no_std` compatible** (with `alloc` feature for `Vec`-based modes)
- **Async-friendly** (Stream/Sink integration)

This combination doesn't exist. That's the niche.

---

## 3. Library scope

### In scope (V0.1)

- ITU BS.1770-4 K-weighting filter (2-stage biquad)
- Multi-channel loudness with weighting (L/R/C unweighted, Ls/Rs +1.5dB, LFE excluded)
- Three measurements:
  - **M** — momentary (400ms window)
  - **S** — short-term (3s window)
  - **I** — integrated (gated, full programme)
- True peak via 4× oversampling FIR (per Annex 2 of BS.1770)
- LRA (loudness range, EBU Tech 3342)
- Push API: `analyzer.push_planar(&[&[f32]; N])` and `push_interleaved(&[f32])`
- Stream-friendly: deterministic output regardless of chunk boundaries
- `f32` and `f64` sample formats
- Sample rates: 44.1 / 48 / 88.2 / 96 / 192 kHz (validated)

### Out of scope (V0.1)

- File I/O — caller decodes; library only sees PCM samples
- Resampling — caller resamples; library asserts input rate
- Visualization — caller renders; library returns numbers
- Loudness *correction* (normalization) — this is read-only; correction is a separate crate someday

### Out of scope, ever

- libebur128 FFI mode — defeats the purpose
- Async runtime — keep it sync; runtime integration is the caller's job

---

## 4. Public API surface

The whole API. Small on purpose. Each item below is a documented public symbol.

```rust
// Top-level
pub struct Analyzer { /* ... */ }
pub struct AnalyzerBuilder { /* ... */ }
pub enum Channel { Left, Right, Center, LeftSurround, RightSurround, Lfe, Other }
pub enum Mode { Integrated, Momentary, ShortTerm, TruePeak, Lra }
pub struct Snapshot { /* current values */ }
pub struct Report  { /* final values after `finalize()` */ }
pub enum Error    { /* InvalidSampleRate, ChannelMismatch, ... */ }

impl AnalyzerBuilder {
    pub fn new() -> Self;
    pub fn sample_rate(self, hz: u32) -> Self;
    pub fn channels(self, layout: &[Channel]) -> Self;
    pub fn modes(self, modes: Mode) -> Self;          // bitflags
    pub fn build(self) -> Result<Analyzer, Error>;
}

impl Analyzer {
    pub fn push_planar<S: Sample>(&mut self, channels: &[&[S]]) -> Result<(), Error>;
    pub fn push_interleaved<S: Sample>(&mut self, samples: &[S]) -> Result<(), Error>;
    pub fn snapshot(&self) -> Snapshot;
    pub fn finalize(self) -> Report;
    pub fn reset(&mut self);
}

impl Report {
    pub fn integrated_lufs(&self) -> Option<f64>;
    pub fn loudness_range_lu(&self) -> Option<f64>;
    pub fn true_peak_dbtp(&self) -> Option<f64>;
    pub fn momentary_max_lufs(&self) -> Option<f64>;
    pub fn short_term_max_lufs(&self) -> Option<f64>;
    pub fn programme_duration_seconds(&self) -> f64;
}
```

That's it. ~7 public types, ~15 public methods. The smaller this stays, the more the documentation can do justice to each item.

### Cargo features

| Feature | Default | What it adds |
|---|---|---|
| `std` | ✓ | Standard library; off enables `no_std` mode |
| `alloc` | ✓ | LRA support (needs `Vec`); without it, LRA is unavailable |
| `f64` | — | f64 sample input support |
| `serde` | — | `Serialize` / `Deserialize` for `Snapshot` and `Report` |
| `tokio` | — | `Sink<f32>` impl for streaming integration |

---

## 5. README spec

The README is the single most-clicked artifact in the project. Treat it as a self-contained essay. Reference projects to match: [`tokio`](https://github.com/tokio-rs/tokio), [`serde`](https://github.com/serde-rs/serde), [`bevy`](https://github.com/bevyengine/bevy) — not generic crate boilerplate.

### Structure (in order)

```markdown
1. Title + tagline (one line)
2. Badges row (crates.io, docs.rs, CI, license, MSRV)
3. ★ 30-second hero example (working code, copy-pasteable)
4. ★ Animated demo (terminal recording or GIF showing live LUFS readout)
5. Why this exists (the crates.io gap from §2)
6. Comparison table (this crate vs. ebur128 vs. bs1770)
7. Installation (one line)
8. Concepts (3-paragraph primer on EBU R128 for non-broadcast engineers)
9. Usage examples (3 progressive: file, real-time, multi-channel)
10. Architecture diagram (3 boxes: filter → block aggregator → integrator)
11. Performance numbers (criterion benchmarks vs. ebur128 C)
12. Compliance (EBU Tech 3341 reference test results: 14/14 passing)
13. Trade-offs I made (3-5 bullets, see §10 ARCHITECTURE.md)
14. Used by (link to QC API + Mediq + FestivalPlayout when those depend on it)
15. Contributing (small section, sets expectations)
16. License (MIT or Apache-2.0; pick one)
```

### Sections that earn the README its keep

These are the parts that make this *the* writeup and replace a blog post:

#### "Why this exists" — must be 80% of a real blog post

Not "I needed loudness analysis." The honest version:

> The Rust ecosystem has three loudness crates. Two wrap a C library. One is incomplete. None expose a streaming API that lets a real-time pipeline push 64-sample blocks and read back current LUFS without reallocating. This crate does that. Here's why streaming matters: [3 paragraphs about real-time broadcast use cases, deterministic output, why pull-based APIs break when you have a Tokio task driving an audio stream].

This section makes the difference between "another loudness crate" and "the crate to reach for."

#### "Trade-offs I made" — proves you made choices

```markdown
- **Push-based API instead of pull-based**, because the consumers I care about
  (FestivalPlayout, Mediq) drive the audio clock — they push, the analyzer
  reacts. Pull-based forces a buffer between caller and analyzer that
  defeats the point.

- **Programme buffer for LRA is `Vec`-based, not ring-buffered.** LRA is a
  statistical measurement over the full programme, so the cost is unavoidable.
  This is why `lra` requires the `alloc` feature.

- **f32 internally, even when input is f64.** EBU R128 specifies 24-bit
  precision; f32 mantissa exceeds that. The accuracy delta vs. f64 internal
  is ≤ 0.001 LU, well below the 0.1 LU spec tolerance. Code is simpler and
  ~30% faster as a result.

- **No SIMD in V0.1.** The K-weighting filter is the hot path; auto-vectorization
  via LLVM gets us within 15% of hand-written SIMD on x86_64 and aarch64.
  Adding `wide`/`packed_simd` was net complexity for a 15% gain. Reconsidering
  in V0.2.

- **`no_std` mode disables LRA.** LRA needs a growable buffer; can't do that
  without `alloc`. Documented loudly so embedded users aren't surprised.
```

This is the section a hiring manager screenshots and asks you about in an interview. Five concrete trade-offs with reasoning is more engineering signal than 5,000 lines of code without explanation.

### Things that go in the README but get cut from the docs.rs version

- The animated demo GIF (renders weirdly on docs.rs)
- The "Used by" section (changes over time; goes in CHANGELOG instead)
- The contributing section (lives in CONTRIBUTING.md too)

---

## 6. docs.rs / lib docs spec

docs.rs is the *engineering* face of the project. The README is the marketing face. They're allowed to differ.

### Crate-level docs (`//!` at top of `lib.rs`)

The first 500 lines of `lib.rs` are docs, not code. Structure:

```rust
//! # ebur128-stream
//!
//! Streaming, zero-alloc EBU R128 loudness measurement in pure Rust.
//!
//! ## Quick start
//! [working example with #[doc] attribute that compiles]
//!
//! ## Concepts
//! ### What is loudness?
//! [3 paragraphs: dB SPL vs. LUFS, why peak is wrong, K-weighting]
//!
//! ### Momentary, short-term, integrated
//! [explanation with timing diagram in ASCII art]
//!
//! ### True peak vs. sample peak
//! [why oversampling, what 4x means, when it matters]
//!
//! ## Choosing a mode
//! [decision table: which mode for which use case]
//!
//! ## Performance characteristics
//! [latency, allocations per push, computational complexity]
//!
//! ## Compliance
//! [link to EBU Tech 3341 test results]
//!
//! ## See also
//! - [`bs1770`] for filter-only, no gating
//! - [`ebur128`] for libebur128 FFI bindings
//! - [Mediq](https://github.com/vanja/Mediq) — terminal video player using this crate
```

The "Concepts" section in the docs is what makes this crate's docs.rs page a *teaching artifact*. Most loudness libraries assume you know what LUFS is. Most of your readers don't. Explain it.

### Module-level docs

Every module has a 5–15 line `//!` header explaining its role and how it fits.

### Doctest discipline

**Every public function has a runnable example via `#[doc]`.** No exceptions. This is what makes docs.rs feel polished — every signature has a green-checkmark "Run" button next to it.

```rust
/// Pushes interleaved samples into the analyzer.
///
/// # Example
///
/// ```
/// use ebur128_stream::AnalyzerBuilder;
///
/// let mut a = AnalyzerBuilder::new()
///     .sample_rate(48_000)
///     .channels(&[Channel::Left, Channel::Right])
///     .build()?;
///
/// let samples = vec![0.1f32; 9_600]; // 100ms of stereo
/// a.push_interleaved(&samples)?;
/// # Ok::<(), ebur128_stream::Error>(())
/// ```
pub fn push_interleaved<S: Sample>(&mut self, samples: &[S]) -> Result<(), Error> { /* ... */ }
```

Run `cargo test --doc` in CI. Treat doctest failures the same as unit-test failures.

---

## 7. `examples/` directory spec

Six example programs. Each runnable with `cargo run --example <name>`. Each ≤ 80 lines.

| Example | Demonstrates |
|---|---|
| `01_basic_lufs.rs` | Synthetic sine wave → integrated LUFS readout |
| `02_file_analysis.rs` | Read WAV via `hound`, run full analysis, print report |
| `03_realtime_monitor.rs` | `cpal` mic input → live LUFS in terminal (refreshes 10×/s) |
| `04_multichannel_5_1.rs` | 5.1 surround, channel-weighted loudness |
| `05_streaming_chunks.rs` | Same audio chunked at 64, 1024, and 65535 samples; show identical results (determinism proof) |
| `06_axum_endpoint.rs` | Tiny HTTP server wrapping the library; pairs with the QC API project |

The `05_streaming_chunks.rs` example is **critical** — it's the proof-by-demonstration that the streaming claim is real. Without this, "deterministic regardless of chunk size" is just marketing.

`06_axum_endpoint.rs` is a deliberate hook into your QC API portfolio piece — both projects link to each other.

---

## 8. ARCHITECTURE.md spec

A 200–400 line markdown file at repo root explaining how the library is built. Six sections:

1. **High-level dataflow** — ASCII diagram of: input samples → K-weighting filter → 100ms blocks → 400ms momentary / 3s short-term aggregator → integrator with gating → outputs
2. **The K-weighting filter** — biquad coefficients (with the magic numbers from BS.1770), why two stages, why this order, where they came from
3. **The gating algorithm** — absolute vs. relative gate, why -70 / -10, what "gating" means visually with a diagram of a programme with quiet sections excluded
4. **True peak via oversampling** — why sample peak isn't enough, the 4× FIR coefficients, what "inter-sample peak" means, a diagram
5. **Streaming determinism** — how the analyzer handles chunk boundaries that fall mid-block, the internal state machine, the proof that `push_n_then_push_m == push_(n+m)`
6. **Performance choices** — what's hot, what's not, what isn't optimized and why (see "Trade-offs" in README §5)

This file is the technical-writing artifact. It's where the depth lives. A reviewer who clicks through gets *more* engineering signal here than from the source code itself.

---

## 9. CHANGELOG, releases, semver

### CHANGELOG.md

Keep-a-Changelog format. Every release entry has:
- Version + date
- Added / Changed / Fixed / Deprecated / Removed sections
- Links to GitHub release notes for full diff

### Versioning

- 0.1.x — V0.1 surface; can change between 0.1.0 → 0.1.1 if needed (it's pre-1.0)
- 1.0.0 — only after the API has been stable for ≥ 3 months and one external user has filed a bug
- Don't version-bump for documentation changes. Don't version-bump for `Cargo.toml` metadata. Real changes only.

### Release checklist (in `RELEASING.md`)

- [ ] All tests pass (`cargo test --all-features`)
- [ ] All doctests pass (`cargo test --doc`)
- [ ] All examples run (`cargo run --example <each>`)
- [ ] EBU Tech 3341 compliance suite passes 14/14
- [ ] Benchmarks recorded in `bench-results.md`
- [ ] CHANGELOG updated
- [ ] `cargo publish --dry-run` clean
- [ ] Tag pushed; GitHub release created with notes
- [ ] `cargo publish`

The checklist is itself a portfolio artifact. Reviewers reading `RELEASING.md` see "this person ships with discipline."

---

## 10. Visual assets

### Required

1. **Architecture diagram** — `docs/architecture.svg`, sourced from a Mermaid or Excalidraw file checked into the repo. Embedded in README and docs.rs lib doc.
2. **Demo GIF** — terminal recording of `cargo run --example 03_realtime_monitor` showing LUFS values updating live. ≤ 4 MB. Recorded with `vhs` (charmbracelet) or `asciinema → agg` for reproducibility.
3. **Comparison table** — Markdown table in README §5 (already specced).

### Optional

4. **Compliance test results image** — checkerboard SVG of EBU 3341 tests passing
5. **Benchmark chart** — `criterion`-generated comparison vs. C reference, embedded in README

### Anti-recommendation

Don't add a logo. Logos on small libraries read as overdone. Wait until 1.0.

---

## 11. CI and tooling

`.github/workflows/ci.yml` runs on every PR + main:

| Job | What |
|---|---|
| `test` | `cargo test --all-features` on Linux, macOS, Windows |
| `test-no-std` | `cargo test --no-default-features --features alloc` |
| `doctest` | `cargo test --doc` |
| `examples` | `cargo build --examples` |
| `clippy` | `cargo clippy --all-features --all-targets -- -D warnings` |
| `fmt` | `cargo fmt --check` |
| `msrv` | `cargo +1.74 build` (pin MSRV; bump deliberately) |
| `compliance` | Custom job running EBU 3341 test suite; fails build if regression |
| `bench` | `cargo bench` (informational, not blocking) |
| `publish-dry-run` | `cargo publish --dry-run` on PRs that touch Cargo.toml |

Codecov / coverage badge optional; `cargo-llvm-cov` produces it.

`pre-commit` config: fmt + clippy + cargo-deny.

---

## 12. Quality bar — what "book-quality" means

Concretely measurable. If any of these fails, the project is not done:

- [ ] README opens with a working `cargo add` command and a 10-line example that compiles when copied
- [ ] Demo GIF visible in the first screen of the README on GitHub mobile
- [ ] docs.rs renders with no broken intra-doc links (`cargo doc --no-deps -- -D rustdoc::broken_intra_doc_links`)
- [ ] Every public symbol has a doc comment (enforced via `#![deny(missing_docs)]` in lib.rs)
- [ ] Every public function has a runnable doctest
- [ ] All 6 examples run without error on a fresh `cargo install` of the repo
- [ ] EBU Tech 3341 compliance test suite passes 14/14 (this is the integrity test — you cannot ship a loudness library that fails this)
- [ ] Benchmarks within 25% of the libebur128 C reference for `push_interleaved` throughput
- [ ] ARCHITECTURE.md is at least 200 lines of substantive prose
- [ ] CHANGELOG has a real entry for v0.1.0 (not just "initial release")
- [ ] CI green on Linux, macOS, Windows
- [ ] `cargo deny check` passes (no licenses incompatible with MIT/Apache-2.0)

If a reviewer can poke at the project for 10 minutes and find a missing doctest, a stale link, or a failing example, this isn't book-quality.

### Reference projects to match

These crates have documentation worth copying the *style* of:

- **`tokio`** — for crate-level lib doc structure
- **`serde`** — for examples-driven docs
- **`clap`** — for "decision rationale in the README" pattern
- **`bevy`** — for visual storytelling in README
- **`rodio`** — same domain (audio); shorter, simpler tone — closer to what you want for a small crate

Read each of their READMEs and `lib.rs` doc comments before writing yours. The bar is set there, not at average crates.io quality.

---

## 13. Naming

Working name `ebur128-stream`. Acceptable. Final-name candidates:

- `ebur128-stream` — descriptive, SEO-friendly, slightly long
- `loudkit` — short, brand-able, vague
- `lufs` — taken on crates.io
- `r128` — taken
- `streamloudness` — descriptive, ugly
- `lufstream` — short, evocative, available

**Decision rule:** if the descriptive name (`ebur128-stream`) is available, take it. Naming uniqueness matters less than search-ability for a niche library. People will find it via Google "rust streaming ebur128," not via the name being clever.

Verify availability before publishing: `cargo search ebur128-stream` + check crates.io.

---

## 14. Timebox & milestones

**Hard stop: 2 weekends.**

| Milestone | Time | Done when |
|---|---|---|
| **M1: Library skeleton** | Wknd 1, Sat AM (4h) | `cargo new`, builder API compiling, basic test passes |
| **M2: K-weighting + momentary** | Wknd 1, Sat PM (4h) | Sine wave at -23 LUFS reads back as -23 LUFS ± 0.1 |
| **M3: Integrated + gating** | Wknd 1, Sun (6h) | Programme test passes, EBU 3341 tests 1–4 green |
| **M4: True peak + LRA** | Wknd 2, Sat AM (4h) | All 14 EBU 3341 tests green |
| **M5: Examples + benchmarks** | Wknd 2, Sat PM (4h) | All 6 examples run; criterion bench published |
| **M6: README + docs.rs polish** | Wknd 2, Sun (6h) | Quality-bar checklist (§12) passes; v0.1.0 published to crates.io |

If a milestone slips, drop in this order: f64 input support → tokio Sink → SIMD → no_std mode. Never drop the documentation.

---

## 15. Anti-scope

Things to consciously NOT add. Re-read this list every time you're tempted.

- ~~SIMD acceleration~~ — V0.2 maybe
- ~~Resampling~~ — caller's job
- ~~File I/O~~ — caller's job
- ~~A separate "real-time" feature flag~~ — the whole library is real-time
- ~~Loudness *correction*~~ — different crate, different problem
- ~~A CLI tool~~ — separate project; can be `lufs` (binary crate) later
- ~~Custom error types per module~~ — one `Error` enum with all variants
- ~~Async trait~~ — push API is sync; tokio Sink is the integration point
- ~~Dolby Dialog Intelligence-equivalent~~ — proprietary, out of scope
- ~~A web playground~~ — too much work for a library; do this for the QC API instead

---

## 16. How this fits the rest of the portfolio

This crate is a **dependency** of three other portfolio pieces:

1. **QC API (broadcast-qc-api)** — wraps this crate; README links to it ("Powered by [`ebur128-stream`]")
2. **Mediq** — its loudness analysis comes from here; if Mediq's README is updated, link back
3. **FestivalPlayout** — uses this for live loudness monitoring; same linking back

This means: when this crate ships, you go update three other READMEs to link to it. The result is a **portfolio that reads as a connected ecosystem** instead of a list of unrelated projects. That coherence is itself a senior-engineer signal.

---

## 17. Success metrics

For a portfolio piece, "success" is what a reviewer concludes. Concretely:

- A reviewer landing on the GitHub page can answer "what does this do" in 10 seconds
- A reviewer reading the README for 5 minutes can explain the trade-offs in their own words
- A reviewer checking docs.rs sees green doctest indicators and zero broken links
- The `cargo install --example` flow works without warnings on macOS, Linux, Windows
- A `Show /r/rust` or `Show HN` post receives ≥ 50 upvotes (loose proxy for "the niche cared")
- Within 6 months, the crate has ≥ 1 external user filing an issue or PR (proves the library works for someone you don't know)

If any of these fails, the documentation work isn't done — the *code* might be done, but the artifact is not.

---

## 18. References

- [ITU-R BS.1770-4 — Algorithms to measure audio programme loudness](https://www.itu.int/rec/R-REC-BS.1770) — the spec
- [EBU Tech 3341 — Loudness Metering](https://tech.ebu.ch/docs/tech/tech3341.pdf) — the test vectors
- [EBU Tech 3342 — Loudness Range](https://tech.ebu.ch/docs/tech/tech3342.pdf) — for LRA
- [`libebur128` source](https://github.com/jiixyj/libebur128) — reference C implementation; correctness benchmark
- [`tokio/README.md`](https://github.com/tokio-rs/tokio/blob/master/README.md) — README quality bar
- [`serde-rs/serde`](https://github.com/serde-rs/serde) — example-driven docs
- [Keep a Changelog](https://keepachangelog.com/) — CHANGELOG format
- Internal: `Mediq` source — has working K-weighting + integrator code to extract
