use ebur128_stream_rs as engine;
use magnus::{Error, RModule, Ruby, prelude::*};

#[magnus::wrap(class = "EBUR128Stream::Report")]
pub(crate) struct Report {
    pub(crate) report: engine::Report,
}

pub(crate) fn init(ruby: &Ruby, module: &RModule) -> Result<(), Error> {
    let report = module.define_class("Report", ruby.class_object())?;

    Ok(())
}
