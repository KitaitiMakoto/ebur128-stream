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

struct Channel(engine::Channel);

impl From<Channel> for engine::Channel {
    fn from(value: Channel) -> Self {
        value.0
    }
}

impl TryConvert for Channel {
    fn try_convert(val: Value) -> Result<Self, magnus::Error> {
        let channel = Symbol::try_convert(val)?;
        Ok(match channel.name()?.as_ref() {
            "left" => Self(engine::Channel::Left),
            "right" => Self(engine::Channel::Right),
            "center" => Self(engine::Channel::Center),
            "left_surround" => Self(engine::Channel::LeftSurround),
            "right_surround" => Self(engine::Channel::RightSurround),
            "lfe" => Self(engine::Channel::Lfe),
            "other" => Self(engine::Channel::Other),
            _ => {
                let ruby = Ruby::get_with(val);
                return Err(magnus::Error::new(
                    ruby.exception_arg_error(),
                    format!("unknown channel: {val}"),
                ));
            }
        })
    }
}

pub(crate) struct Channels {
    inner: Vec<engine::Channel>,
}

impl Deref for Channels {
    type Target = Vec<engine::Channel>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a> From<&'a [engine::Channel]> for Channels {
    fn from(value: &'a [engine::Channel]) -> Self {
        Self {
            inner: value.to_vec().into_iter().collect(),
        }
    }
}

impl TryConvert for Channels {
    fn try_convert(val: Value) -> Result<Self, magnus::Error> {
        Ok(Self {
            inner: RArray::try_convert(val)?
                .into_iter()
                .map(|value| Ok(Channel::try_convert(value)?.into()))
                .collect::<Result<Vec<engine::Channel>, magnus::Error>>()?,
        })
    }
}

impl Channels {
    fn try_into_rarray(&self, ruby: &Ruby) -> Result<RArray, magnus::Error> {
        let syms = self
            .inner
            .iter()
            .map(|channel| {
                use engine::Channel::*;

                let str = match channel {
                    Left => "left",
                    Right => "right",
                    Center => "center",
                    LeftSurround => "left_surround",
                    RightSurround => "right_surround",
                    Lfe => "lfe",
                    Other => "other",
                    _ => {
                        return Err(magnus::Error::new(
                            ruby.exception_runtime_error(),
                            "couldn't convert to Symbol: {channel}",
                        ));
                    }
                };
                Ok(ruby.to_symbol(str))
            })
            .collect::<Result<Vec<Symbol>, magnus::Error>>()?;
        Ok(ruby.ary_new_from_values(&syms))
    }

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
