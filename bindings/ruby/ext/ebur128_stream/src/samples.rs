use crate::{
    error::Error,
    memory_view::{Flags, FlagsChainable, ItemComponent, MemoryView},
};
use magnus::{RArray, Ruby, TryConvert, Value, error::IntoError};

fn is_acceptable_component(component: ItemComponent) -> bool {
    component.offset == 0
        && component.repeat == 1
        && is_acceptable_format(component.format)
}

fn is_acceptable_format(format: char) -> bool {
    match format {
        'f' => true,

        #[cfg(target_endian = "little")]
        'e' => true,

        #[cfg(target_endian = "big")]
        'g' => true,

        _ => false
    }
}

pub(crate) enum InterleavedSamples {
    Array { samples: Vec<f32> },
    MemoryView { view: MemoryView<f32> },
}

impl TryConvert for InterleavedSamples {
    fn try_convert(val: Value) -> Result<Self, magnus::Error> {
        if let Some(view) = Self::consume_memory_view(val) {
            Ok(Self::MemoryView { view })
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

impl InterleavedSamples {
    pub(crate) fn as_slice(&self) -> &[f32] {
        match self {
            Self::Array { samples } => samples,
            Self::MemoryView { view } => view.data(),
        }
    }

    fn consume_memory_view(val: Value) -> Option<MemoryView<f32>> {
        let view = MemoryView::<f32>::get(val, Flags::any_contiguous());
        if let Ok(mut view) = view {
            if Self::is_acceptable(&mut view).unwrap_or(false) {
                return Some(view);
            }
        }
        let view = MemoryView::<f32>::get(val, Flags::simple());
        if let Ok(mut view) = view {
            if Self::is_acceptable(&mut view).unwrap_or(false) {
                return Some(view);
            }
        }
        None
    }

    // TODO: Check format more strictly(size, other expression)
    fn is_acceptable(view: &mut MemoryView<f32>) -> Result<bool, Error> {
        let item_desc = view.item_desc()?;
        Ok(view.ndim() == 1
            && item_desc.len() == 1
            && item_desc
                .into_iter()
                .next()
                .is_some_and(is_acceptable_component))
    }
}

pub(crate) enum WritableInterleavedSamples {
    Array { obj: RArray, samples: Vec<f32> },
    MemoryView { view: MemoryView<f32> },
}

impl TryConvert for WritableInterleavedSamples {
    fn try_convert(val: Value) -> Result<Self, magnus::Error> {
        if let Some(view) = Self::consume_memory_view(val) {
            Ok(Self::MemoryView { view })
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

impl WritableInterleavedSamples {
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

    fn consume_memory_view(val: Value) -> Option<MemoryView<f32>> {
        let view = MemoryView::<f32>::get(val, Flags::writable().any_contiguous());
        if let Ok(mut view) = view {
            if Self::is_acceptable(&mut view).unwrap_or(false) {
                return Some(view);
            }
        }
        let view = MemoryView::<f32>::get(val, Flags::simple());
        if let Ok(mut view) = view {
            if !view.is_readonly() && Self::is_acceptable(&mut view).unwrap_or(false) {
                return Some(view);
            }
        }
        None
    }

    fn is_acceptable(view: &mut MemoryView<f32>) -> Result<bool, Error> {
        let item_desc = view.item_desc()?;
        Ok(view.ndim() == 1
            && item_desc.len() == 1
            && item_desc
                .into_iter()
                .next()
                .is_some_and(is_acceptable_component))
    }
}

pub(crate) enum PlanarSamples {
    Array { samples: Vec<Vec<f32>> },
    MemoryView { view: MemoryView<f32> },
}

impl TryConvert for PlanarSamples {
    fn try_convert(val: Value) -> Result<Self, magnus::Error> {
        if let Some(view) = Self::consume_memory_view(val) {
            Ok(Self::MemoryView { view })
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

    fn consume_memory_view(val: Value) -> Option<MemoryView<f32>> {
        let view = MemoryView::<f32>::get(val, Flags::row_major());
        if let Ok(mut view) = view {
            if Self::is_acceptable(&mut view).unwrap_or(false) {
                return Some(view);
            }
        }
        let view = MemoryView::<f32>::get(val, Flags::simple());
        if let Ok(mut view) = view {
            if Self::is_acceptable(&mut view).unwrap_or(false) {
                return Some(view);
            }
        }
        None
    }

    fn is_acceptable(view: &mut MemoryView<f32>) -> Result<bool, Error> {
        let item_desc = view.item_desc()?;
        Ok(view.ndim() == 2
            && item_desc.len() == 1
            && item_desc
                .into_iter()
                .next()
                .is_some_and(is_acceptable_component))
    }
}
