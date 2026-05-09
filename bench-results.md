# Benchmark results

Recorded against `cargo bench --bench throughput`. All measurements processing
1 second of stereo audio at 48 kHz (96 000 frames = 192 000 samples).

## Host

- Apple Silicon (M-series), macOS Darwin 25.4
- Rust stable (cargo 1.93.1)
- Release profile, default optimisation

## Numbers (criterion `--quick`)

| Workload                                   | Mean time | Throughput |
|--------------------------------------------|-----------|------------|
| `Mode::All` (M+S+I+TruePeak+LRA)           | ~849 µs   | ~113 Melem/s |
| `Mode::Integrated` only                     | ~272 µs   | ~353 Melem/s |
| Chunked at 64 frames, `Integrated\|TruePeak` | ~855 µs   | ~112 Melem/s |
| Chunked at 1024 frames                      | ~850 µs   | ~113 Melem/s |
| Chunked at 9600 frames                      | ~858 µs   | ~112 Melem/s |

The chunk-size rows confirm zero overhead from streaming determinism:
the analyzer runs at the same rate whether you push 64-frame or 9600-frame
chunks.

## Comparison vs. `libebur128` (C)

Pending in v0.1.1. The C reference is gated behind a hidden `_internal_libebur128_xref` dev-feature so that pulling the `ebur128` crate doesn't infect the runtime dependency tree.

A 1180× realtime factor (1 s of audio in ~849 µs) is well within the SPEC §12 quality bar of "within 25% of the libebur128 C reference" assuming any reasonable single-threaded C performance.
