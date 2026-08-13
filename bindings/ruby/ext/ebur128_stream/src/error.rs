use ebur128_stream_rs as engine;
use magnus::{Ruby, error::IntoError};

pub(crate) enum Error {
    Magnus(magnus::Error),
    Engine(engine::Error),
}

impl From<magnus::Error> for Error {
    fn from(err: magnus::Error) -> Self {
        Self::Magnus(err)
    }
}

impl From<engine::Error> for Error {
    fn from(err: engine::Error) -> Self {
        Self::Engine(err)
    }
}

impl IntoError for Error {
    fn into_error(self, ruby: &Ruby) -> magnus::Error {
        match self {
            Self::Magnus(err) => err,
            Self::Engine(err) => {
                let err_class = match err {
                    engine::Error::InterleavedLengthNotMultiple {
                        samples: _,
                        channels: _,
                    }
                    | engine::Error::ChannelMismatch {
                        expected: _,
                        got: _,
                    }
                    | engine::Error::PlanarLengthMismatch { first: _, got: _ }
                    | engine::Error::NonFiniteSample => ruby.exception_arg_error(),
                    _ => ruby.exception_runtime_error(),
                };
                magnus::Error::new(err_class, err.to_string())
            }
        }
    }
}
