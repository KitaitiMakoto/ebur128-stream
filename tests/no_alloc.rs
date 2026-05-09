//! Verifies that `Analyzer::push_*` performs zero heap allocations
//! once the programme buffer has been pre-reserved via
//! `AnalyzerBuilder::expected_duration`.
//!
//! Implementation: a custom global allocator that counts every
//! `alloc` / `dealloc` call. We measure the count *during* a push,
//! after the analyzer has been warm-up-pushed for one full programme
//! buffer extent so all `Vec` doublings have happened.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use ebur128_stream::{AnalyzerBuilder, Channel, Mode};

struct CountingAllocator;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static DEALLOCS: AtomicUsize = AtomicUsize::new(0);
static COUNTING: AtomicBool = AtomicBool::new(false);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if COUNTING.load(Ordering::Relaxed) {
            DEALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) };
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

fn snapshot_counters() -> (usize, usize) {
    (
        ALLOCS.load(Ordering::Relaxed),
        DEALLOCS.load(Ordering::Relaxed),
    )
}

#[test]
fn push_interleaved_zero_alloc_steady_state() {
    use core::time::Duration;
    // Build the analyzer with all modes + an `expected_duration` hint
    // that pre-reserves the programme buffer.
    let mut a = AnalyzerBuilder::new()
        .sample_rate(48_000)
        .channels(&[Channel::Left, Channel::Right])
        .modes(Mode::All)
        .expected_duration(Duration::from_secs(120))
        .build()
        .unwrap();

    // Warm-up: push the first 30 s. This forces any one-time growth
    // (the FIR delay lines settle, the programme/short-term Vecs
    // confirm their pre-reserved capacity is sufficient).
    let chunk: Vec<f32> = vec![0.05; 9_600 * 2]; // 100 ms stereo
    for _ in 0..300 {
        a.push_interleaved::<f32>(&chunk).unwrap();
    }

    // Now count allocs over a few seconds of steady-state push.
    COUNTING.store(true, Ordering::Relaxed);
    let (a0, _) = snapshot_counters();
    for _ in 0..50 {
        a.push_interleaved::<f32>(&chunk).unwrap();
    }
    let (a1, _) = snapshot_counters();
    COUNTING.store(false, Ordering::Relaxed);

    let n_allocs = a1 - a0;
    assert!(
        n_allocs == 0,
        "expected zero allocations during steady-state push, got {n_allocs}"
    );
}

#[test]
fn snapshot_zero_alloc_when_cached() {
    let mut a = AnalyzerBuilder::new()
        .sample_rate(48_000)
        .channels(&[Channel::Left, Channel::Right])
        .modes(Mode::Momentary | Mode::ShortTerm)
        .build()
        .unwrap();
    let chunk: Vec<f32> = vec![0.05; 9_600 * 2];
    for _ in 0..40 {
        a.push_interleaved::<f32>(&chunk).unwrap();
    }
    // First snapshot fills the cache.
    let _ = a.snapshot();

    COUNTING.store(true, Ordering::Relaxed);
    let (a0, _) = snapshot_counters();
    for _ in 0..1_000 {
        let _ = a.snapshot();
    }
    let (a1, _) = snapshot_counters();
    COUNTING.store(false, Ordering::Relaxed);

    let n = a1 - a0;
    assert!(n == 0, "snapshot should be zero-alloc once cached, got {n}");
}
