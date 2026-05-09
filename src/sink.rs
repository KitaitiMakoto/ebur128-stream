//! Optional `futures::Sink<Vec<f32>>` adapter for the analyzer.
//!
//! Enabled by the `tokio` Cargo feature. Pass owned `Vec<f32>` chunks
//! through the sink and the analyzer ingests them as interleaved
//! samples. The sink is always ready — pushing samples is synchronous
//! and never blocks — but the trait conformance lets you slot the
//! analyzer into a Tokio / Futures pipeline as a stream consumer:
//!
//! ```ignore
//! use ebur128_stream::{AnalyzerBuilder, Channel, Mode, AnalyzerSink};
//! use futures_util::SinkExt;
//!
//! let analyzer = AnalyzerBuilder::new()
//!     .sample_rate(48_000)
//!     .channels(&[Channel::Left, Channel::Right])
//!     .modes(Mode::Integrated | Mode::TruePeak)
//!     .build()?;
//! let mut sink = AnalyzerSink::new(analyzer);
//!
//! let chunk: Vec<f32> = vec![0.0; 9_600];
//! sink.send(chunk).await?;
//! let report = sink.into_inner().unwrap().finalize();
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! The crate does not depend on `tokio` itself — only `futures-sink`
//! for the `Sink` trait. Use any executor that drives the sink.

use core::pin::Pin;
use core::task::{Context, Poll};

use crate::analyzer::Analyzer;
use crate::error::Error;

/// A [`futures_sink::Sink`] that ingests `Vec<f32>` interleaved sample
/// chunks into the wrapped [`Analyzer`].
///
/// The sink is permanently in the ready state; `send` and friends never
/// block waiting for capacity. Errors from
/// [`Analyzer::push_interleaved`] surface as `Sink::Error`.
#[derive(Debug)]
pub struct AnalyzerSink {
    analyzer: Option<Analyzer>,
}

impl AnalyzerSink {
    /// Wrap the analyzer.
    #[must_use]
    pub fn new(analyzer: Analyzer) -> Self {
        Self {
            analyzer: Some(analyzer),
        }
    }

    /// Recover the inner analyzer, e.g. to call
    /// [`Analyzer::finalize`].
    ///
    /// Returns `None` if the sink was already consumed.
    pub fn into_inner(mut self) -> Option<Analyzer> {
        self.analyzer.take()
    }

    /// Borrow the inner analyzer for snapshotting.
    pub fn analyzer_mut(&mut self) -> Option<&mut Analyzer> {
        self.analyzer.as_mut()
    }
}

impl futures_sink::Sink<Vec<f32>> for AnalyzerSink {
    type Error = Error;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: Vec<f32>) -> Result<(), Self::Error> {
        let this = self.get_mut();
        if let Some(a) = this.analyzer.as_mut() {
            a.push_interleaved::<f32>(&item)?;
        }
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Channel, Mode};
    use core::task::Waker;
    use futures_sink::Sink;

    #[test]
    fn sink_ingests_chunks() {
        let analyzer = crate::AnalyzerBuilder::new()
            .sample_rate(48_000)
            .channels(&[Channel::Left, Channel::Right])
            .modes(Mode::Momentary)
            .build()
            .unwrap();
        let mut sink = AnalyzerSink::new(analyzer);
        // `Waker::noop` is stable since 1.85 — no unsafe needed.
        let waker: &Waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        let mut s = Pin::new(&mut sink);
        match s.as_mut().poll_ready(&mut cx) {
            Poll::Ready(Ok(())) => {}
            _ => panic!("AnalyzerSink is always ready"),
        }
        s.as_mut().start_send(vec![0.1f32; 9_600]).unwrap();

        let a = sink.into_inner().unwrap();
        let mut a = a;
        let snap = a.snapshot();
        assert!((snap.programme_duration_seconds() - 0.1).abs() < 1e-9);
    }
}
