use crate::{Channels, Error, InterleavedSamples, PlanarSamples, Report, Snapshot};
use ebur128_stream_rs as engine;
use magnus::{
    Integer, Module, RArray, RModule, Ruby, Symbol, TryConvert, Value, function, method,
    prelude::*,
    scan_args::{get_kwargs, scan_args},
};
use std::{
    cell::{Ref, RefCell, RefMut},
    time::Duration,
};

#[magnus::wrap(class = "EBUR128Stream::Analyzer")]
struct Analyzer {
    analyzer: RefCell<Option<engine::Analyzer>>,
}

impl Analyzer {
    fn new(args: &[Value]) -> Result<Self, Error> {
        let args = scan_args::<(), (), (), (), _, ()>(args)?;
        let kws =
            get_kwargs::<_, (Channels,), (Option<Integer>, Option<RArray>, Option<Integer>), ()>(
                args.keywords,
                &["channels"],
                &["sample_rate", "modes", "expected_duration"],
            )?;
        let (channels,) = kws.required;
        let (sample_rate, modes, expected_duration) = kws.optional;

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

    fn sample_rate(&self) -> Result<u32, Error> {
        Ok(self.analyzer()?.sample_rate())
    }

    fn channels(ruby: &Ruby, rb_self: &Self) -> Result<RArray, Error> {
        Ok(Channels::from(rb_self.analyzer()?.channels()).try_into_rarray(ruby)?)
    }

    fn modes(ruby: &Ruby, rb_self: &Self) -> Result<RArray, Error> {
        let syms = rb_self
            .analyzer()?
            .modes()
            .iter_names()
            .map(|mode| ruby.to_symbol(mode.0.to_lowercase()))
            .collect::<Vec<Symbol>>();
        Ok(ruby.ary_new_from_values(&syms))
    }

    fn samples_per_block(&self) -> Result<u32, Error> {
        Ok(self.analyzer()?.samples_per_block())
    }

    fn push_interleaved(&self, samples: InterleavedSamples) -> Result<(), Error> {
        Ok(self.analyzer_mut()?.push_interleaved(samples.as_slice())?)
    }

    fn push_planar(&self, samples: PlanarSamples) -> Result<(), Error> {
        Ok(self
            .analyzer_mut()?
            .push_planar(&samples.channel_slices())?)
    }

    fn snapshot(&self) -> Result<Snapshot, Error> {
        let snapshot = self.analyzer_mut()?.snapshot();

        Ok(Snapshot { snapshot })
    }

    fn reset(&self) -> Result<(), Error> {
        self.analyzer_mut()?.reset();

        Ok(())
    }

    fn finalize(&self) -> Result<Report, Error> {
        let mut analyzer = self
            .analyzer
            .try_borrow_mut()
            .map_err(|_| Error::runtime("analyzer already in use"))?;
        let analyzer = analyzer
            .take()
            .ok_or_else(|| Error::runtime("analyzer already finalized"))?;
        let report = analyzer.finalize();

        Ok(Report { report })
    }

    fn analyzer<'a>(&'a self) -> Result<Ref<'a, engine::Analyzer>, Error> {
        let analyzer = self
            .analyzer
            .try_borrow()
            .map_err(|_| Error::runtime("analyzer already in use"))?;

        Ref::filter_map(analyzer, Option::as_ref)
            .map_err(|_| Error::runtime("analyzer not initialized"))
    }

    fn analyzer_mut<'a>(&'a self) -> Result<RefMut<'a, engine::Analyzer>, Error> {
        let analyzer = self
            .analyzer
            .try_borrow_mut()
            .map_err(|_| Error::runtime("analyzer already in use"))?;

        RefMut::filter_map(analyzer, Option::as_mut)
            .map_err(|_| Error::runtime("analyzer not initialized"))
    }
}

pub(crate) fn init(ruby: &Ruby, module: &RModule) -> Result<(), Error> {
    let analyzer = module.define_class("Analyzer", ruby.class_object())?;
    analyzer.define_singleton_method("new", function!(Analyzer::new, -1))?;
    analyzer.define_method("sample_rate", method!(Analyzer::sample_rate, 0))?;
    analyzer.define_method("channels", method!(Analyzer::channels, 0))?;
    analyzer.define_method("modes", method!(Analyzer::modes, 0))?;
    analyzer.define_method("samples_per_block", method!(Analyzer::samples_per_block, 0))?;
    analyzer.define_method("push_interleaved", method!(Analyzer::push_interleaved, 1))?;
    analyzer.define_method("push_planar", method!(Analyzer::push_planar, 1))?;
    analyzer.define_method("snapshot", method!(Analyzer::snapshot, 0))?;
    analyzer.define_method("finalize", method!(Analyzer::finalize, 0))?;
    analyzer.define_method("reset", method!(Analyzer::reset, 0))?;

    Ok(())
}
