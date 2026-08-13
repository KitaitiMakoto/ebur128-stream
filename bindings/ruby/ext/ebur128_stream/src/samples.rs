use crate::{
    error::Error,
    memory_view::{Flags, FlagsChainable, MemoryView},
};
use magnus::{RArray, Ruby, TryConvert, Value, error::IntoError};
use rb_sys::ruby_memory_view_flags::{
    RUBY_MEMORY_VIEW_ANY_CONTIGUOUS, RUBY_MEMORY_VIEW_SIMPLE, RUBY_MEMORY_VIEW_WRITABLE,
};

pub(crate) enum InterleavedSamples {
    Array { obj: RArray, samples: Vec<f32> },
    MemoryView { view: MemoryView },
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
    pub(crate) fn as_slice(&self) -> &[f32] {
        match self {
            Self::Array { obj: _, samples } => samples,
            Self::MemoryView { view } => view.data().unwrap_or(&[]),
        }
    }

    pub(crate) fn as_mut_slice(&mut self) -> &mut [f32] {
        match self {
            Self::Array { obj: _, samples } => samples,
            Self::MemoryView { view } => view.data_as_mut().unwrap_or(&mut []),
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
        let view = MemoryView::get(val, Flags::simple());
        if let Ok(view) = view {
            if !view.is_readonly() && view.format().is_some() && view.format().unwrap() == "f" {
                // TODO: Check format more strictly(size, other expression)
                return Some(Self::MemoryView { view });
            }
        }
        let view = MemoryView::get(val, Flags::writable().any_contiguous());
        if let Ok(view) = view {
            if let Some(format) = view.format() {
                if format == "f" {
                    return Some(Self::MemoryView { view });
                }
            }
        }
        None
    }
}

pub(crate) enum PlanarSamples {
    Array { samples: Vec<Vec<f32>> },
    MemoryView { view: MemoryView },
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
            Self::MemoryView { view } => view.data().unwrap_or(&[]).to_vec(),
        }
    }

    fn consume_memory_view(val: Value) -> Option<Self> {
        let view = MemoryView::get(val, Flags::writable().format().row_major());
        if let Ok(view) = view {
            if view.ndim() == 2
                && let Some(format) = view.format()
                && format == "f"
            {
                return Some(Self::MemoryView { view });
            }
        }
        let view = MemoryView::get(val, Flags::simple());
        if let Ok(view) = view {
            if !view.is_readonly()
                && view.ndim() == 2
                && view.format().is_some()
                && view.format().unwrap() == "f"
            {
                return Some(Self::MemoryView { view });
            }
        }
        None
    }
}
