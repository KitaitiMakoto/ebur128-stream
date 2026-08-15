use crate::Error;
use ebur128_stream_rs as engine;
use magnus::{RModule, Ruby, method, prelude::*};

#[magnus::wrap(class = "EBUR128Stream::Snapshot")]
pub(crate) struct Snapshot {
    pub(crate) snapshot: engine::Snapshot,
}

impl Snapshot {
    fn momentary_lufs(&self) -> Option<f64> {
        self.snapshot.momentary_lufs()
    }

    fn short_term_lufs(&self) -> Option<f64> {
        self.snapshot.short_term_lufs()
    }

    fn integrated_lufs(&self) -> Option<f64> {
        self.snapshot.integrated_lufs()
    }

    fn true_peak_dbtp(&self) -> Option<f64> {
        self.snapshot.true_peak_dbtp()
    }

    fn loudness_range_lu(&self) -> Option<f64> {
        self.snapshot.loudness_range_lu()
    }

    fn programme_duration_seconds(&self) -> f64 {
        self.snapshot.programme_duration_seconds()
    }
}

pub(crate) fn init(ruby: &Ruby, module: &RModule) -> Result<(), Error> {
    let snapshot = module.define_class("Snapshot", ruby.class_object())?;
    snapshot.define_method("momentary_lufs", method!(Snapshot::momentary_lufs, 0))?;
    snapshot.define_method("short_term_lufs", method!(Snapshot::short_term_lufs, 0))?;
    snapshot.define_method("integrated_lufs", method!(Snapshot::integrated_lufs, 0))?;
    snapshot.define_method("loudness_range_lu", method!(Snapshot::loudness_range_lu, 0))?;
    snapshot.define_method("true_peak_dbtp", method!(Snapshot::true_peak_dbtp, 0))?;
    snapshot.define_method(
        "programme_duration_seconds",
        method!(Snapshot::programme_duration_seconds, 0),
    )?;

    Ok(())
}
