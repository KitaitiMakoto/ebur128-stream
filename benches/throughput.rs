//! Criterion benchmark — `push_interleaved` and end-to-end throughput.
//!
//! The SPEC §12 quality bar wants this within 25% of `libebur128`
//! (the C reference). The C reference is *not* a runtime dependency;
//! a comparison bench would require pulling the `ebur128` crate as a
//! dev-dep and a working C toolchain. To keep the dev-dep tree lean,
//! this bench measures only the Rust crate; the comparison number is
//! recorded in `bench-results.md` from a one-off run.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use ebur128_stream::{AnalyzerBuilder, Channel, Mode};

const FS: u32 = 48_000;

fn synth_stereo(seconds: f32) -> Vec<f32> {
    let n = (FS as f32 * seconds) as usize;
    let two_pi_f = 2.0 * std::f32::consts::PI * 1000.0 / FS as f32;
    let mut buf = Vec::with_capacity(n * 2);
    for i in 0..n {
        let v = 0.1 * (two_pi_f * i as f32).sin();
        buf.push(v);
        buf.push(v);
    }
    buf
}

fn bench_push_interleaved_stereo(c: &mut Criterion) {
    let signal = synth_stereo(1.0); // 1 s stereo @ 48 kHz = 96 000 samples
    let mut group = c.benchmark_group("push_interleaved_stereo_48k");
    group.throughput(Throughput::Elements(signal.len() as u64));
    group.bench_function("Mode::All", |b| {
        b.iter(|| {
            let mut a = AnalyzerBuilder::new()
                .sample_rate(FS)
                .channels(&[Channel::Left, Channel::Right])
                .modes(Mode::All)
                .build()
                .unwrap();
            a.push_interleaved::<f32>(black_box(&signal)).unwrap();
            black_box(a.finalize());
        })
    });
    group.bench_function("Mode::Integrated only", |b| {
        b.iter(|| {
            let mut a = AnalyzerBuilder::new()
                .sample_rate(FS)
                .channels(&[Channel::Left, Channel::Right])
                .modes(Mode::Integrated)
                .build()
                .unwrap();
            a.push_interleaved::<f32>(black_box(&signal)).unwrap();
            black_box(a.finalize());
        })
    });
    group.finish();
}

fn bench_chunked(c: &mut Criterion) {
    let signal = synth_stereo(1.0);
    let mut group = c.benchmark_group("push_interleaved_chunk_size");
    group.throughput(Throughput::Elements(signal.len() as u64));
    for &chunk_frames in &[64usize, 1_024, 9_600] {
        group.bench_with_input(
            criterion::BenchmarkId::new("chunk_frames", chunk_frames),
            &chunk_frames,
            |b, &cf| {
                b.iter(|| {
                    let mut a = AnalyzerBuilder::new()
                        .sample_rate(FS)
                        .channels(&[Channel::Left, Channel::Right])
                        .modes(Mode::Integrated | Mode::TruePeak)
                        .build()
                        .unwrap();
                    let cs = cf * 2;
                    for c in signal.chunks(cs) {
                        a.push_interleaved::<f32>(black_box(c)).unwrap();
                    }
                    black_box(a.finalize());
                })
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_push_interleaved_stereo, bench_chunked);
criterion_main!(benches);
