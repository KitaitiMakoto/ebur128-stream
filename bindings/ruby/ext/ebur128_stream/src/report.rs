use ebur128_stream_rs as engine;
use magnus::{Error, RModule, Ruby, prelude::*, method};

#[magnus::wrap(class = "EBUR128Stream::Report")]
pub(crate) struct Report {
    pub(crate) report: engine::Report,
}

impl Report {
    fn integrated_lufs(rb_self: &Self) -> Option<f64> {
        rb_self.report.integrated_lufs()
    }

    fn loudness_range_lu(rb_self: &Self) -> Option<f64> {
        rb_self.report.loudness_range_lu()
    }

    fn true_peak_dbtp(rb_self: &Self) -> Option<f64> {
        rb_self.report.true_peak_dbtp()
    }

    fn momentary_max_lufs(rb_self: &Self) -> Option<f64> {
        rb_self.report.momentary_max_lufs()
    }

    fn short_term_max_lufs(rb_self: &Self) -> Option<f64> {
        rb_self.report.short_term_max_lufs()
    }

    fn programme_duration_seconds(rb_self: &Self) -> f64 {
        rb_self.report.programme_duration_seconds()
    }
}

pub(crate) fn init(ruby: &Ruby, module: &RModule) -> Result<(), Error> {
    let report = module.define_class("Report", ruby.class_object())?;
    report.define_method("integrated_lufs", method!(Report::integrated_lufs, 0))?;
    report.define_method("loudness_range_lu", method!(Report::loudness_range_lu, 0))?;
    report.define_method("true_peak_dbtp", method!(Report::true_peak_dbtp, 0))?;
    report.define_method("momentary_max_lufs", method!(Report::momentary_max_lufs, 0))?;
    report.define_method("short_term_max_lufs", method!(Report::short_term_max_lufs, 0))?;
    report.define_method("programme_duration_seconds", method!(Report::programme_duration_seconds, 0))?;

    Ok(())
}
