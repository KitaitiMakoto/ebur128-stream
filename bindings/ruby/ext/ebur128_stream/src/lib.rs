mod analyzer;
mod error;
mod memory_view;
mod normalize;
mod report;
mod samples;
mod snapshot;

use crate::{
    error::Error,
    report::Report,
    samples::{InterleavedSamples, PlanarSamples},
    snapshot::Snapshot,
};
use ebur128_stream_rs as engine;
use magnus::{RArray, Ruby, Symbol, TryConvert, Value};
use std::ops::Deref;

pub(crate) struct Channels {
    inner: Vec<engine::Channel>,
}

impl Deref for Channels {
    type Target = Vec<engine::Channel>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl TryConvert for Channels {
    fn try_convert(val: Value) -> Result<Self, magnus::Error> {
        use engine::Channel::*;

        let array = RArray::try_convert(val)?;
        let result: Result<Vec<engine::Channel>, magnus::Error> = array
            .into_iter()
            .map(|value| {
                let ch = Symbol::try_convert(value)?;
                match ch.name()?.as_ref() {
                    "left" => Ok(Left),
                    "right" => Ok(Right),
                    "center" => Ok(Center),
                    "left_surround" => Ok(LeftSurround),
                    "right_surround" => Ok(RightSurround),
                    "lfe" => Ok(Lfe),
                    "other" => Ok(Other),
                    _ => {
                        let ruby = Ruby::get_with(val);
                        Err(magnus::Error::new(
                            ruby.exception_runtime_error(),
                            format!("unknown channel: {ch}"),
                        ))
                    }
                }
            })
            .collect();
        Ok(Self { inner: result? })
    }
}

impl Channels {
    fn into_boxed_slice(self) -> Box<[engine::Channel]> {
        self.inner.into_boxed_slice()
    }
}

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let ebur128_stream = ruby.define_module("EBUR128Stream")?;

    analyzer::init(ruby, &ebur128_stream)?;
    snapshot::init(ruby, &ebur128_stream)?;
    report::init(ruby, &ebur128_stream)?;
    normalize::init(ruby, &ebur128_stream)?;

    Ok(())
}
