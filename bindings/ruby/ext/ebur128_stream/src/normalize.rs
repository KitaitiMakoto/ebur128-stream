use crate::{error::Error, parse_channels_arg, samples::InterleavedSamples};
use ebur128_stream_rs as engine;
use magnus::{
    RArray, RModule, Ruby, TryConvert, Value, function, method,
    prelude::*,
    scan_args::{get_kwargs, scan_args},
};

// Members are the same to engine::normalize::Normalizer
#[magnus::wrap(class = "EBUR128Stream::Normalizer")]
struct Normalizer {
    sample_rate: u32,
    channels: Box<[engine::Channel]>,
    target_lufs: Option<f64>,
    true_peak_ceiling_dbtp: Option<f64>,
}

impl Normalizer {
    fn new(args: &[Value]) -> Result<Self, Error> {
        let args = scan_args::<(), (), (), (), _, ()>(args)?;
        let kws = get_kwargs::<_, (u32, RArray), (Option<f64>, Option<f64>), ()>(
            args.keywords,
            &["sample_rate", "channels"],
            &["target_lufs", "true_peak_ceiling_dbtp"],
        )?;
        let (sample_rate, channels) = kws.required;
        let (target_lufs, true_peak_ceiling_dbtp) = kws.optional;

        let channels = parse_channels_arg(channels)?;

        Ok(Self {
            sample_rate,
            channels: channels.into_boxed_slice(),
            target_lufs,
            true_peak_ceiling_dbtp,
        })
    }

    fn normalize_in_place(rb_self: &Self, samples: Value) -> Result<NormalizeReport, Error> {
        let mut frames = InterleavedSamples::try_convert(samples)?;
        let mut normalizer =
            engine::normalize::Normalizer::new(rb_self.sample_rate, &rb_self.channels);
        if let Some(target_lufs) = rb_self.target_lufs {
            normalizer = normalizer.target_lufs(target_lufs);
        }
        if let Some(true_peak_ceiling_dbtp) = rb_self.true_peak_ceiling_dbtp {
            normalizer = normalizer.true_peak_ceiling_dbtp(true_peak_ceiling_dbtp);
        }

        let report = normalizer
            .normalize_in_place(frames.as_mut_slice())
            .map_err(Error::runtime)?;
        frames.write_back_in_place()?;

        Ok(NormalizeReport { report })
    }
}

#[magnus::wrap(class = "EBUR128Stream::NormalizeReport")]
struct NormalizeReport {
    report: engine::normalize::NormalizeReport,
}

impl NormalizeReport {
    fn measured_integrated_lufs(rb_self: &Self) -> Option<f64> {
        rb_self.report.measured_integrated_lufs
    }

    fn measured_true_peak_dbtp(rb_self: &Self) -> Option<f64> {
        rb_self.report.measured_true_peak_dbtp
    }

    fn target_lufs(rb_self: &Self) -> f64 {
        rb_self.report.target_lufs
    }

    fn true_peak_ceiling_dbtp(rb_self: &Self) -> Option<f64> {
        rb_self.report.true_peak_ceiling_dbtp
    }

    fn applied_gain_db(rb_self: &Self) -> f64 {
        rb_self.report.applied_gain_db
    }

    fn limited_by_true_peak(rb_self: &Self) -> bool {
        rb_self.report.limited_by_true_peak
    }
}

pub(crate) fn init(ruby: &Ruby, module: &RModule) -> Result<(), Error> {
    let normalizer = module.define_class("Normalizer", ruby.class_object())?;
    normalizer.define_singleton_method("new", function!(Normalizer::new, -1))?;
    normalizer.define_method(
        "normalize_in_place",
        method!(Normalizer::normalize_in_place, 1),
    )?;

    let normalize_report = module.define_class("NormalizeReport", ruby.class_object())?;
    normalize_report.define_method(
        "measured_integrated_lufs",
        method!(NormalizeReport::measured_integrated_lufs, 0),
    )?;
    normalize_report.define_method(
        "measured_true_peak_dbtp",
        method!(NormalizeReport::measured_true_peak_dbtp, 0),
    )?;
    normalize_report.define_method("target_lufs", method!(NormalizeReport::target_lufs, 0))?;
    normalize_report.define_method(
        "true_peak_ceiling_dbtp",
        method!(NormalizeReport::true_peak_ceiling_dbtp, 0),
    )?;
    normalize_report.define_method(
        "applied_gain_db",
        method!(NormalizeReport::applied_gain_db, 0),
    )?;
    normalize_report.define_method(
        "limited_by_true_peak",
        method!(NormalizeReport::limited_by_true_peak, 0),
    )?;

    Ok(())
}
