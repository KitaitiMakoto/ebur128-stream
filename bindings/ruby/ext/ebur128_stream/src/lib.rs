use core::time::Duration;
use ebur128_stream_rs::{
    Analyzer as AnalyzerRs, AnalyzerBuilder, Channel, Error as RsError, Mode, Report as RsReport,
};
use magnus::{
    Error, Integer, RArray, Ruby, Symbol, TryConvert, Value, function, method,
    prelude::*,
    scan_args::{get_kwargs, scan_args},
};
use std::cell::RefCell;

#[magnus::wrap(class = "EBUR128Stream::Analyzer")]
struct Analyzer {
    builder: Option<AnalyzerBuilder>,
    analyzer: RefCell<Option<AnalyzerRs>>,
}

impl Analyzer {
    fn new(ruby: &Ruby, args: &[Value]) -> Result<Self, Error> {
        let args = scan_args::<(), (), (), (), _, ()>(args)?;
        let kws = get_kwargs::<_, (RArray,), (Option<Integer>, Option<RArray>, Option<Integer>), ()>(
            args.keywords,
            &["channels"],
            &["sample_rate", "modes", "expected_duration"],
        )?;
        let (channels,) = kws.required;
        let (sample_rate, modes, expected_duration) = kws.optional;

        let channels: Vec<Channel> = channels
            .into_iter()
            .map(|value| {
                use Channel::*;

                let ch = Symbol::try_convert(value)?;
                match ch.name()?.as_ref() {
                    "left" => Ok(Left),
                    "right" => Ok(Right),
                    "center" => Ok(Center),
                    "left_surround" => Ok(LeftSurround),
                    "right_surround" => Ok(RightSurround),
                    "lfe" => Ok(Lfe),
                    "other" => Ok(Other),
                    _ => Err(Error::new(
                        ruby.exception_arg_error(),
                        format!("unknown channel: {ch:?}"),
                    )),
                }
            })
            .collect::<Result<_, Error>>()?;

        let mut builder = AnalyzerBuilder::new().channels(&channels);

        if let Some(sample_rate) = sample_rate {
            builder = builder.sample_rate(sample_rate.to_u32()?);
        }

        if let Some(values) = modes {
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
                        return Err(Error::new(
                            ruby.exception_arg_error(),
                            format!("unknown mode: {mode:?}"),
                        ));
                    }
                }
            }
            builder = builder.modes(modes);
        }
        if let Some(expected_duration) = expected_duration {
            builder = builder.expected_duration(Duration::from_secs(expected_duration.to_u64()?));
        }

        // TODO: Pend build() and call it just before calling push_xxx()
        let analyzer = builder
            .build()
            .map_err(|err| Error::new(ruby.exception_runtime_error(), format!("{}", err)))?;

        Ok(Self {
            builder: None,
            analyzer: RefCell::new(Some(analyzer)),
        })
    }

    fn push_interleaved(ruby: &Ruby, rb_self: &Self, samples: Value) -> Result<(), Error> {
        // TODO: Accept MemoryView producer
        // TODO: Consider chunking instead of converting whole samples at once
        let samples = if let Some(array) = RArray::from_value(samples) {
            array
                .into_iter()
                .map(TryConvert::try_convert)
                .collect::<Result<Vec<f32>, Error>>()?
        } else {
            return Err(Error::new(
                ruby.exception_arg_error(),
                format!("unsupported samples type: {samples:?}"),
            ));
        };

        let mut analyzer = rb_self
            .analyzer
            .try_borrow_mut()
            .map_err(|_| Error::new(ruby.exception_runtime_error(), "analyser already in use"))?;
        let analyzer = analyzer.as_mut().ok_or_else(|| {
            Error::new(ruby.exception_runtime_error(), "analyzer not initialized")
        })?;
        analyzer
            .push_interleaved(&samples)
            .map_err(|err| match err {
                RsError::InterleavedLengthNotMultiple {
                    samples: _,
                    channels: _,
                } => Error::new(ruby.exception_arg_error(), format!("{err:?}")),
                RsError::NonFiniteSample => {
                    Error::new(ruby.exception_arg_error(), format!("{err:?}"))
                }
                _ => unreachable!(),
            })?;

        Ok(())
    }

    fn push_planar(ruby: &Ruby, rb_self: &Self, samples: Value) -> Result<(), Error> {
        let samples: Vec<Vec<f32>> = if let Some(channels) = RArray::from_value(samples) {
            channels
                .into_iter()
                .map(|channel| {
                    let ch = RArray::from_value(channel).ok_or_else(|| {
                        Error::new(
                            ruby.exception_arg_error(),
                            format!("channel not Array: {channel:?}"),
                        )
                    })?;
                    ch.into_iter()
                        .map(f32::try_convert)
                        .collect::<Result<Vec<f32>, Error>>()
                })
                .collect::<Result<Vec<Vec<f32>>, Error>>()?
        } else {
            return Err(Error::new(
                ruby.exception_arg_error(),
                format!("unsupported samples type: {samples:?}"),
            ));
        };
        let samples: Vec<&[f32]> = samples.iter().map(|channel| &channel[..]).collect();

        let mut analyzer = rb_self
            .analyzer
            .try_borrow_mut()
            .map_err(|_| Error::new(ruby.exception_runtime_error(), "analyzer already in use"))?;
        let analyzer = analyzer.as_mut().ok_or_else(|| {
            Error::new(ruby.exception_runtime_error(), "analyzer not initialized")
        })?;
        analyzer.push_planar(&samples).map_err(|err| match err {
            RsError::ChannelMismatch {
                expected: _,
                got: _,
            } => Error::new(ruby.exception_arg_error(), format!("{err:?}")),
            RsError::PlanarLengthMismatch { first: _, got: _ } => {
                Error::new(ruby.exception_arg_error(), format!("{err:?}"))
            }
            RsError::NonFiniteSample => Error::new(ruby.exception_arg_error(), format!("{err:?}")),
            _ => unreachable!(),
        })?;

        Ok(())
    }

    fn finalize(ruby: &Ruby, rb_self: &Self) -> Result<Report, Error> {
        let mut analyzer = rb_self
            .analyzer
            .try_borrow_mut()
            .map_err(|_| Error::new(ruby.exception_runtime_error(), "analyzer already in use"))?;
        let analyzer = analyzer.take().ok_or_else(|| {
            Error::new(ruby.exception_runtime_error(), "analyzer already finalized")
        })?;
        let report = analyzer.finalize();

        Ok(Report { report })
    }
}

#[magnus::wrap(class = "EBUR128Stream::Report")]
struct Report {
    report: RsReport,
}

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let ebur128_stream = ruby.define_module("EBUR128Stream")?;
    let analyzer = ebur128_stream.define_class("Analyzer", ruby.class_object())?;
    analyzer.define_singleton_method("new", function!(Analyzer::new, -1))?;
    analyzer.define_method("push_interleaved", method!(Analyzer::push_interleaved, 1))?;
    analyzer.define_method("push_planar", method!(Analyzer::push_planar, 1))?;
    analyzer.define_method("finalize", method!(Analyzer::finalize, 0))?;
    let report = ebur128_stream.define_class("Report", ruby.class_object())?;

    Ok(())
}
