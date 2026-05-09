//! 100 ms block aggregator.
//!
//! Per BS.1770-4, loudness is computed in 100 ms blocks. For each
//! channel we sum the squares of the K-weighted samples over a
//! 100 ms window and emit a per-channel mean-square value when the
//! window is full.
//!
//! The aggregator runs at exact 100 ms cadence regardless of input
//! chunk size — this is the streaming-determinism contract.

#![allow(clippy::needless_range_loop)]

use crate::analyzer::MAX_CHANNELS;

/// Number of 100 ms blocks in the momentary (400 ms) window.
pub(crate) const MOMENTARY_BLOCKS: usize = 4;

/// Number of 100 ms blocks in the short-term (3 s) window.
pub(crate) const SHORT_TERM_BLOCKS: usize = 30;

/// Per-channel running sum of squared K-weighted samples over the
/// current 100 ms window.
#[derive(Debug)]
pub(crate) struct BlockAccumulator {
    n_channels: usize,
    samples_per_block: u32,
    samples_in_current: u32,
    sum_sq: [f64; MAX_CHANNELS],
}

impl BlockAccumulator {
    pub(crate) fn new(n_channels: usize, samples_per_block: u32) -> Self {
        Self {
            n_channels,
            samples_per_block,
            samples_in_current: 0,
            sum_sq: [0.0; MAX_CHANNELS],
        }
    }

    /// Push one frame; returns `true` when the 100 ms block has filled
    /// (caller must then call [`Self::take_block`]).
    #[inline]
    pub(crate) fn push_frame(&mut self, frame: &[f32]) -> bool {
        for (i, &s) in frame.iter().enumerate() {
            self.sum_sq[i] += (s as f64) * (s as f64);
        }
        self.samples_in_current += 1;
        self.samples_in_current >= self.samples_per_block
    }

    /// Finalise the current 100 ms block and return per-channel
    /// mean-square values. The accumulator is reset for the next block.
    #[inline]
    pub(crate) fn take_block(&mut self) -> [f32; MAX_CHANNELS] {
        let mut out = [0.0f32; MAX_CHANNELS];
        let n = self.samples_per_block.max(1) as f64;
        for i in 0..self.n_channels {
            out[i] = (self.sum_sq[i] / n) as f32;
            self.sum_sq[i] = 0.0;
        }
        self.samples_in_current = 0;
        out
    }

    pub(crate) fn reset(&mut self) {
        self.sum_sq = [0.0; MAX_CHANNELS];
        self.samples_in_current = 0;
    }
}
