use crate::{
    error::Error,
    memory_view::{Flags, FlagsChainable, MemoryView},
};
use magnus::{RArray, Ruby, TryConvert, Value, error::IntoError};

fn is_acceptable_format(format: &str) -> bool {
    match format.chars().next() {
        Some('f') => true,

        #[cfg(target_endian = "little")]
        Some('e') => true,

        #[cfg(target_endian = "big")]
        Some('g') => true,

        _ => false,
    }
}

pub(crate) enum InterleavedSamples {
    Array { obj: RArray, samples: Vec<f32> },
    MemoryView { view: MemoryView<f32> },
}

impl TryConvert for InterleavedSamples {
    fn try_convert(val: Value) -> Result<Self, magnus::Error> {
        if let Some(memory_view) = Self::consume_memory_view(val) {
            Ok(memory_view)
        } else if let Some(obj) = RArray::from_value(val) {
            Ok(Self::Array {
                obj,
                samples: obj.to_vec()?,
            })
        } else {
            Err(Error::argument(format!("unsupported samples type: {val}"))
                .into_error(&Ruby::get_with(val)))
        }
    }
}

impl InterleavedSamples {
    // Instead of this method, should add Writable/ReadableInterleavedSamples,
    // or try_concert_mut()?
    pub(crate) fn is_writable(&self) -> bool {
        match self {
            Self::Array { .. } => true,
            Self::MemoryView { view } => !view.is_readonly(),
        }
    }

    pub(crate) fn as_slice(&self) -> &[f32] {
        match self {
            Self::Array { obj: _, samples } => samples,
            Self::MemoryView { view } => view.data(),
        }
    }

    pub(crate) fn as_mut_slice(&mut self) -> &mut [f32] {
        match self {
            Self::Array { obj: _, samples } => samples,
            Self::MemoryView { view } => view.data_as_mut(),
        }
    }

    pub(crate) fn write_back_in_place(self) -> Result<(), Error> {
        match self {
            Self::Array { obj, samples } => {
                let ruby = Ruby::get_with(obj);
                obj.replace(ruby.ary_from_vec(samples))?;
            }
            Self::MemoryView { view: _ } => {}
        }

        Ok(())
    }

    fn consume_memory_view(val: Value) -> Option<Self> {
        let view = MemoryView::<f32>::get(val, Flags::writable().format().any_contiguous());
        if let Ok(view) = view {
            if view.ndim() == 1
                && let Some(format) = view.format()
                && is_acceptable_format(format)
            {
                // TODO: Check format more strictly(size, other expression)
                return Some(Self::MemoryView { view });
            }
        }
        let view = MemoryView::<f32>::get(val, Flags::format().any_contiguous());
        if let Ok(view) = view {
            if view.ndim() == 1
                && let Some(format) = view.format()
                && is_acceptable_format(format)
            {
                return Some(Self::MemoryView { view });
            }
        }
        let view = MemoryView::<f32>::get(val, Flags::simple());
        if let Ok(view) = view {
            if view.ndim() == 1
                && let Some(format) = view.format()
                && is_acceptable_format(format)
            {
                return Some(Self::MemoryView { view });
            }
        }
        None
    }
}

pub(crate) enum PlanarSamples {
    Array { samples: Vec<Vec<f32>> },
    MemoryView { view: MemoryView<f32> },
}

impl TryConvert for PlanarSamples {
    fn try_convert(val: Value) -> Result<Self, magnus::Error> {
        if let Some(memory_view) = Self::consume_memory_view(val) {
            Ok(memory_view)
        } else if let Some(obj) = RArray::from_value(val) {
            Ok(Self::Array {
                samples: obj.to_vec()?,
            })
        } else {
            Err(Error::argument(format!("unsupported samples type: {val}"))
                .into_error(&Ruby::get_with(val)))
        }
    }
}

impl PlanarSamples {
    pub fn channel_slices(&self) -> Vec<&[f32]> {
        match self {
            Self::Array { samples } => samples.iter().map(Vec::as_slice).collect(),
            Self::MemoryView { view } => {
                let shape = view.shape().expect("ndim > 1 is checked when calling ");
                let n_channels = shape[0];
                let channel_len = shape[1];
                if channel_len == 0 {
                    (0..n_channels).map(|_| &view.data()[..0]).collect()
                } else {
                    view.data().chunks_exact(channel_len).collect()
                }
            }
        }
    }

    fn consume_memory_view(val: Value) -> Option<Self> {
        let view = MemoryView::<f32>::get(val, Flags::writable().format().row_major());
        if let Ok(view) = view {
            if view.ndim() == 2
                && let Some(format) = view.format()
                && is_acceptable_format(format)
            {
                return Some(Self::MemoryView { view });
            }
        }
        let view = MemoryView::<f32>::get(val, Flags::simple());
        if let Ok(view) = view {
            if !view.is_readonly()
                && view.ndim() == 2
                && let Some(format) = view.format()
                && is_acceptable_format(format)
            {
                return Some(Self::MemoryView { view });
            }
        }
        None
    }
}
