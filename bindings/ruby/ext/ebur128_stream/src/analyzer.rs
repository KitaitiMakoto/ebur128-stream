use crate::{Error, InterleavedSamples, PlanarSamples, Report, Snapshot, parse_channels_arg};
use ebur128_stream_rs as engine;
use magnus::{
    Integer, Module, RArray, RModule, Ruby, Symbol, TryConvert, Value, function, method,
    prelude::*,
    scan_args::{get_kwargs, scan_args},
};
use std::{cell::RefCell, time::Duration};

#[magnus::wrap(class = "EBUR128Stream::Analyzer")]
struct Analyzer {
    analyzer: RefCell<Option<engine::Analyzer>>,
}

impl Analyzer {
    fn new(args: &[Value]) -> Result<Self, Error> {
        let args = scan_args::<(), (), (), (), _, ()>(args)?;
        let kws = get_kwargs::<_, (RArray,), (Option<Integer>, Option<RArray>, Option<Integer>), ()>(
            args.keywords,
            &["channels"],
            &["sample_rate", "modes", "expected_duration"],
        )?;
        let (channels,) = kws.required;
        let (sample_rate, modes, expected_duration) = kws.optional;

        let channels = parse_channels_arg(channels)?;
        let mut builder = engine::AnalyzerBuilder::new().channels(&channels);

        if let Some(sample_rate) = sample_rate {
            builder = builder.sample_rate(sample_rate.to_u32()?);
        }

        if let Some(values) = modes {
            use engine::Mode;

            let mut modes = Mode::empty();
            for value in values.into_iter() {
                let mode = Symbol::try_convert(value)?;
                modes |= match mode.name()?.as_ref() {
                    "integrated" => Mode::Integrated,
                    "momentary" => Mode::Momentary,
                    "short_term" => Mode::ShortTerm,
                    "true_peak" => Mode::TruePeak,
                    "lra" => Mode::Lra,
                    "all" => Mode::All,
                    _ => {
                        return Err(Error::argument(format!("unknown mode: {mode}")))?;
                    }
                }
            }
            builder = builder.modes(modes);
        }
        if let Some(expected_duration) = expected_duration {
            builder = builder.expected_duration(Duration::from_secs(expected_duration.to_u64()?));
        }

        let analyzer = builder.build().map_err(Error::runtime)?;

        Ok(Self {
            analyzer: RefCell::new(Some(analyzer)),
        })
    }

    // Segmentation fault occurs if samples: InterleavedSamples in arguments
    fn push_interleaved(rb_self: &Self, samples: Value) -> Result<(), Error> {
        let samples = InterleavedSamples::try_convert(samples)?;

        let mut analyzer = rb_self.analyzer.try_borrow_mut().map_err(Error::runtime)?;
        let analyzer = analyzer
            .as_mut()
            .ok_or_else(|| Error::runtime("analyzer not initialized"))?;
        analyzer.push_interleaved(samples.as_slice())?;

        Ok(())
    }

    fn push_planar(rb_self: &Self, samples: PlanarSamples) -> Result<(), Error> {
        let mut analyzer = rb_self
            .analyzer
            .try_borrow_mut()
            .map_err(|_| Error::runtime("analyzer already in use"))?;
        let analyzer = analyzer
            .as_mut()
            .ok_or_else(|| Error::runtime("analyzer not initialized"))?;
        analyzer.push_planar(&samples.channel_slices())?;

        Ok(())
    }

    fn snapshot(rb_self: &Self) -> Result<Snapshot, Error> {
        let mut analyzer = rb_self
            .analyzer
            .try_borrow_mut()
            .map_err(|_| Error::runtime("analyzer aldready in use"))?;
        let analyzer = analyzer
            .as_mut()
            .ok_or_else(|| Error::runtime("analyzer not initialized"))?;
        let snapshot = analyzer.snapshot();

        Ok(Snapshot { snapshot })
    }

    fn finalize(rb_self: &Self) -> Result<Report, Error> {
        let mut analyzer = rb_self
            .analyzer
            .try_borrow_mut()
            .map_err(|_| Error::runtime("analyzer already in use"))?;
        let analyzer = analyzer
            .take()
            .ok_or_else(|| Error::runtime("analyzer already finalized"))?;
        let report = analyzer.finalize();

        Ok(Report { report })
    }
}

pub(crate) fn init(ruby: &Ruby, module: &RModule) -> Result<(), Error> {
    let analyzer = module.define_class("Analyzer", ruby.class_object())?;
    analyzer.define_singleton_method("new", function!(Analyzer::new, -1))?;
    analyzer.define_method("push_interleaved", method!(Analyzer::push_interleaved, 1))?;
    analyzer.define_method("push_planar", method!(Analyzer::push_planar, 1))?;
    analyzer.define_method("snapshot", method!(Analyzer::snapshot, 0))?;
    analyzer.define_method("finalize", method!(Analyzer::finalize, 0))?;

    Ok(())
}
