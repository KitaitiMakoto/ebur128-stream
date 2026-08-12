use crate::parse_channels_arg;
use ebur128_stream_rs as engine;
use magnus::{
    Error, RArray, RModule, Ruby, Value, function,
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
    fn new(ruby: &Ruby, args: &[Value]) -> Result<Self, Error> {
        let args = scan_args::<(), (), (), (), _, ()>(args)?;
        let kws = get_kwargs::<_, (u32, RArray), (Option<f64>, Option<f64>), ()>(
            args.keywords,
            &["sample_rate", "channels"],
            &["target_lufs", "true_peak_ceiling_dbtp"],
        )?;
        let (sample_rate, channels) = kws.required;
        let (target_lufs, true_peak_ceiling_dbtp) = kws.optional;

        let channels = parse_channels_arg(ruby, channels)?;

        Ok(Self {
            sample_rate,
            channels: channels.into_boxed_slice(),
            target_lufs,
            true_peak_ceiling_dbtp,
        })
    }
}

pub(crate) fn init(ruby: &Ruby, module: &RModule) -> Result<(), Error> {
    let normalizer = module.define_class("Normalizer", ruby.class_object())?;
    normalizer.define_singleton_method("new", function!(Normalizer::new, -1))?;

    Ok(())
}
