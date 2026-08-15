use magnus::{Error, Ruby, Value};
use rb_sys::{
    Qnil, VALUE, rb_memory_view_get, rb_memory_view_parse_item_format, rb_memory_view_release,
    rb_memory_view_t,
    ruby_memory_view_flags::{
        RUBY_MEMORY_VIEW_ANY_CONTIGUOUS, RUBY_MEMORY_VIEW_COLUMN_MAJOR, RUBY_MEMORY_VIEW_FORMAT,
        RUBY_MEMORY_VIEW_INDIRECT, RUBY_MEMORY_VIEW_MULTI_DIMENSIONAL, RUBY_MEMORY_VIEW_ROW_MAJOR,
        RUBY_MEMORY_VIEW_SIMPLE, RUBY_MEMORY_VIEW_STRIDES, RUBY_MEMORY_VIEW_WRITABLE,
    },
};
use std::{ffi::CStr, marker::PhantomData, mem::MaybeUninit, ptr, slice};

pub struct Flags(i32);

impl From<Flags> for i32 {
    fn from(value: Flags) -> Self {
        value.0
    }
}

impl Flags {
    pub fn simple() -> Self {
        Self(RUBY_MEMORY_VIEW_SIMPLE as i32)
    }

    pub fn writable() -> Self {
        Self(RUBY_MEMORY_VIEW_WRITABLE as i32)
    }

    pub fn format() -> Self {
        Self(RUBY_MEMORY_VIEW_FORMAT as i32)
    }

    pub fn multi_dimensional() -> Self {
        Self(RUBY_MEMORY_VIEW_MULTI_DIMENSIONAL as i32)
    }

    pub fn strides() -> Self {
        Self(RUBY_MEMORY_VIEW_STRIDES as i32)
    }

    pub fn row_major() -> Self {
        Self(RUBY_MEMORY_VIEW_ROW_MAJOR as i32)
    }

    pub fn column_major() -> Self {
        Self(RUBY_MEMORY_VIEW_COLUMN_MAJOR as i32)
    }

    pub fn any_contiguous() -> Self {
        Self(RUBY_MEMORY_VIEW_ANY_CONTIGUOUS as i32)
    }

    pub fn indirect() -> Self {
        Self(RUBY_MEMORY_VIEW_INDIRECT as i32)
    }
}

pub trait FlagsChainable {
    fn writable(self) -> Self;
    fn format(self) -> Self;
    fn multi_dimensional(self) -> Self;
    fn strides(self) -> Self;
    fn row_major(self) -> Self;
    fn column_major(self) -> Self;
    fn any_contiguous(self) -> Self;
    fn indirect(self) -> Self;
}

impl FlagsChainable for Flags {
    fn writable(mut self) -> Self {
        self.0 |= RUBY_MEMORY_VIEW_WRITABLE as i32;
        self
    }

    fn format(mut self) -> Self {
        self.0 |= RUBY_MEMORY_VIEW_FORMAT as i32;
        self
    }

    fn multi_dimensional(mut self) -> Self {
        self.0 |= RUBY_MEMORY_VIEW_MULTI_DIMENSIONAL as i32;
        self
    }

    fn strides(mut self) -> Self {
        self.0 |= RUBY_MEMORY_VIEW_STRIDES as i32;
        self
    }

    fn row_major(mut self) -> Self {
        self.0 |= RUBY_MEMORY_VIEW_ROW_MAJOR as i32;
        self
    }

    fn column_major(mut self) -> Self {
        self.0 |= RUBY_MEMORY_VIEW_COLUMN_MAJOR as i32;
        self
    }

    fn any_contiguous(mut self) -> Self {
        self.0 |= RUBY_MEMORY_VIEW_ANY_CONTIGUOUS as i32;
        self
    }

    fn indirect(mut self) -> Self {
        self.0 |= RUBY_MEMORY_VIEW_INDIRECT as i32;
        self
    }
}

pub struct MemoryView<T> {
    inner: rb_memory_view_t,
    marker: PhantomData<T>,
}

impl<T> Drop for MemoryView<T> {
    // Causes segmentation fault in some cases. Needs the investigation.
    fn drop(&mut self) {
        let _ = magnus::rb_sys::protect(|| {
            // SAFETY: This guard is generated from successfully allocated rb_memory_view_t and drop() is called only once so it's not released more than once
            unsafe {
                rb_memory_view_release(&mut self.inner);
            }
            Qnil.into()
        });
    }
}

impl<T> MemoryView<T> {
    pub fn get(obj: Value, flags: Flags) -> Result<Self, Error> {
        let ruby = Ruby::get_with(obj);

        let obj = unsafe { std::mem::transmute::<Value, VALUE>(obj) };
        let mut view = MaybeUninit::uninit();

        let mut result = false;
        magnus::rb_sys::protect(|| {
            result = unsafe { rb_memory_view_get(obj, view.as_mut_ptr(), flags.into()) };
            Qnil.into()
        })?;
        if !result {
            return Err(Error::new(
                ruby.exception_runtime_error(),
                "MemoryView not got",
            ));
        }

        let view = unsafe { view.assume_init() };
        let view = Self {
            inner: view,
            marker: PhantomData,
        };

        // Validation
        let ndim = usize::try_from(view.inner.ndim)
            .map_err(|_| Error::new(ruby.exception_arg_error(), "invalid ndim"))?;
        usize::try_from(view.inner.byte_size)
            .map_err(|_| Error::new(ruby.exception_arg_error(), "invalid byte_size"))?;
        let item_size = usize::try_from(view.inner.item_size)
            .map_err(|_| Error::new(ruby.exception_arg_error(), "invalid item_size"))?;

        let format = view.inner.format;
        if !format.is_null() {
            let format = unsafe { CStr::from_ptr(format) };
            let item_size_by_format = Self::validate_format(&ruby, format)?;
            if item_size_by_format != item_size {
                Err(Error::new(
                    ruby.exception_arg_error(),
                    "item_size and item size calculated by format not match",
                ))?;
            }
        }

        if view.inner.shape.is_null() {
            if ndim > 1 {
                Err(Error::new(
                    ruby.exception_arg_error(),
                    "ndim > 1 but shape is NULL",
                ))?;
            }
        } else {
            // SAFETY: rb_memory_view_t.shape is *ssize_t
            let shape = unsafe { slice::from_raw_parts(view.inner.shape, ndim) };
            if !view.inner.shape.cast::<usize>().is_aligned() {
                Err(Error::new(
                    ruby.exception_arg_error(),
                    "shape not aligned for usize",
                ))?;
            }
            for &dim in shape {
                usize::try_from(dim).map_err(|_| {
                    Error::new(
                        ruby.exception_arg_error(),
                        format!("dimension {dim} of shape invalid"),
                    )
                })?;
            }
        }

        let data = view.inner.data;
        if data.is_null() {
            Err(Error::new(ruby.exception_runtime_error(), "data is NULL"))?;
        }
        let ptr = data.cast::<T>();
        if !ptr.is_aligned() {
            Err(Error::new(
                ruby.exception_runtime_error(),
                "data not aligned",
            ))?;
        }

        Ok(view)
    }

    pub fn ndim(&self) -> usize {
        usize::try_from(self.inner.ndim).expect("ndim validated in get()")
    }

    pub fn byte_size(&self) -> usize {
        usize::try_from(self.inner.byte_size).expect("byte_size validated in get()")
    }

    pub fn shape(&self) -> Option<&[usize]> {
        if self.inner.shape.is_null() {
            return None;
        }
        // SAFETY: validated in get()
        Some(unsafe { slice::from_raw_parts(self.inner.shape.cast::<usize>(), self.ndim()) })
    }

    pub fn is_readonly(&self) -> bool {
        self.inner.readonly
    }

    pub fn format(&self) -> Option<&str> {
        if self.inner.format.is_null() {
            None
        } else {
            // SAFETY: format is valid because parse_item_format() is called in get()
            Some(
                unsafe { CStr::from_ptr(self.inner.format) }
                    .to_str()
                    .unwrap(),
            )
        }
    }

    pub fn data(&self) -> &[T] {
        let n_items = self.byte_size() / size_of::<T>();
        let data = self.inner.data;
        let ptr = data.cast::<T>();

        unsafe { slice::from_raw_parts(ptr, n_items) }
    }

    pub fn data_as_mut(&mut self) -> &mut [T] {
        let n_items = self.byte_size() / size_of::<T>();
        let data = self.inner.data;
        let ptr = data.cast::<T>();

        unsafe { slice::from_raw_parts_mut(ptr, n_items) }
    }

    // Just for validation and retrieving item size
    // TODO: Implement parse_item_format() properly
    fn validate_format(ruby: &Ruby, format: &CStr) -> Result<usize, Error> {
        let mut item_size = -1;

        magnus::rb_sys::protect(|| {
            item_size = unsafe {
                rb_memory_view_parse_item_format(
                    format.as_ptr(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            };
            Qnil.into()
        })?;

        if item_size < 0 {
            Err(Error::new(
                ruby.exception_runtime_error(),
                "failed to parse format",
            ))
        } else {
            let item_size = usize::try_from(item_size).map_err(|_| {
                Error::new(
                    ruby.exception_runtime_error(),
                    "format parse returned invalid item_size",
                )
            })?;
            Ok(item_size)
        }
    }
}
