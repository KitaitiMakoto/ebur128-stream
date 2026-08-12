use magnus::{Error, RArray, Ruby, TryConvert, Value};

pub(crate) enum InterleavedSamples {
    Array { obj: RArray, samples: Vec<f32> },
    // TODO: MemoryView support
}

impl TryConvert for InterleavedSamples {
    fn try_convert(val: Value) -> Result<Self, Error> {
        let samples = if let Some(obj) = RArray::from_value(val) {
            let samples = obj
                .into_iter()
                .map(TryConvert::try_convert)
                .collect::<Result<Vec<f32>, Error>>()?;
            Self::Array { obj, samples }
        } else {
            let ruby = Ruby::get_with(val);
            return Err(Error::new(
                ruby.exception_arg_error(),
                format!("unsupported samples type: {val}"),
            ));
        };

        Ok(samples)
    }
}

impl InterleavedSamples {
    pub(crate) fn as_mut_slice(&mut self) -> &mut [f32] {
        match self {
            InterleavedSamples::Array { obj: _, samples } => samples,
        }
    }

    pub(crate) fn write_back_in_place(self) -> Result<(), Error> {
        match self {
            InterleavedSamples::Array { obj, samples } => {
                let ruby = Ruby::get_with(obj);
                obj.replace(ruby.ary_from_vec(samples))?;
            }
        }

        Ok(())
    }
}
