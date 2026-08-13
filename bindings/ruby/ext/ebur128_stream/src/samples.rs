use crate::{error::Error, memory_view::MemoryView};
use magnus::{RArray, Ruby, TryConvert, Value};
use rb_sys::ruby_memory_view_flags::{
    RUBY_MEMORY_VIEW_ANY_CONTIGUOUS, RUBY_MEMORY_VIEW_SIMPLE, RUBY_MEMORY_VIEW_WRITABLE,
};

pub(crate) enum InterleavedSamples {
    Array { obj: RArray, samples: Vec<f32> },
    MemoryView { view: MemoryView },
}

impl TryConvert for InterleavedSamples {
    fn try_convert(val: Value) -> Result<Self, magnus::Error> {
        let memory_view = Self::try_consume_memory_view(val);
        let samples = if let Some(memory_view) = memory_view {
            memory_view
        } else if let Some(obj) = RArray::from_value(val) {
            let samples = obj
                .into_iter()
                .map(TryConvert::try_convert)
                .collect::<Result<Vec<f32>, magnus::Error>>()?;
            Self::Array { obj, samples }
        } else {
            let ruby = Ruby::get_with(val);
            return Err(magnus::Error::new(
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

    fn try_consume_memory_view(val: Value) -> Option<Self> {
        let view = MemoryView::get(val, RUBY_MEMORY_VIEW_SIMPLE as i32);
        if let Ok(view) = view {
            if !view.is_readonly() && view.format().is_some() && view.format().unwrap() == "f" {
                // TODO: Check format more strictly(size, other expression)
                return Some(Self::MemoryView { view });
            }
        }
        let view = MemoryView::get(
            val,
            RUBY_MEMORY_VIEW_WRITABLE as i32 | RUBY_MEMORY_VIEW_ANY_CONTIGUOUS as i32,
        );
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
