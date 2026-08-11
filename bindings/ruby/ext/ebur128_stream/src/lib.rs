use core::time::Duration;
use ebur128_stream_rs::{Analyzer as AnalyzerRs, AnalyzerBuilder, Channel, Mode};
use magnus::{
    Error, Integer, RArray, Ruby, Symbol, Value, function, method,
    prelude::*,
    scan_args::{get_kwargs, scan_args},
};

#[magnus::wrap(class = "EBUR128Stream::Analyzer")]
struct Analyzer {
    builder: Option<AnalyzerBuilder>,
    analyzer: Option<AnalyzerRs>,
}

impl Analyzer {
    fn new(ruby: &Ruby, args: &[Value]) -> Result<Self, Error> {
        let args = scan_args::<(), (), (), (), _, ()>(args)?;
        let kws =
            get_kwargs::<_, (RArray,), (Option<Integer>, Option<RArray>, Option<Integer>), ()>(
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
                    _ => return Err(Error::new(ruby.exception_arg_error(), format!("unknown mode: {mode:?}")))
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
            analyzer: Some(analyzer),
        })
    }
}

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let ebur128_stream = ruby.define_module("EBUR128Stream")?;
    let analyzer = ebur128_stream.define_class("Analyzer", ruby.class_object())?;
    analyzer.define_singleton_method("new", function!(Analyzer::new, -1))?;

    Ok(())
}
