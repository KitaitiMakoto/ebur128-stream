use magnus::{Error, Ruby, Value};
use rb_sys::{
    VALUE, rb_memory_view_get, rb_memory_view_release, rb_memory_view_t,
    ruby_memory_view_flags::{
        RUBY_MEMORY_VIEW_ANY_CONTIGUOUS, RUBY_MEMORY_VIEW_COLUMN_MAJOR, RUBY_MEMORY_VIEW_FORMAT,
        RUBY_MEMORY_VIEW_INDIRECT, RUBY_MEMORY_VIEW_MULTI_DIMENSIONAL, RUBY_MEMORY_VIEW_ROW_MAJOR,
        RUBY_MEMORY_VIEW_SIMPLE, RUBY_MEMORY_VIEW_STRIDES, RUBY_MEMORY_VIEW_WRITABLE,
    },
};
use std::{ffi::CStr, mem::MaybeUninit, slice};

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

pub struct MemoryView {
    inner: rb_memory_view_t,
}

impl Drop for MemoryView {
    fn drop(&mut self) {
        // SAFETY: This guard is generated from successfully allocated rb_memory_view_t and drop() is called only once so it's not released more than once
        unsafe {
            rb_memory_view_release(&mut self.inner);
        }
    }
}

impl MemoryView {
    pub fn get(obj: Value, flags: Flags) -> Result<Self, Error> {
        let ruby = Ruby::get_with(obj);

        let obj = unsafe { std::mem::transmute::<Value, VALUE>(obj) };
        let mut view = MaybeUninit::uninit();
        let result = unsafe { rb_memory_view_get(obj, view.as_mut_ptr(), flags.into()) };
        if !result {
            return Err(Error::new(
                ruby.exception_runtime_error(),
                "MemoryView not got",
            ));
        }
        let view = unsafe { view.assume_init() };

        Ok(Self { inner: view })
    }

    fn obj(&self) -> Value {
        unsafe { std::mem::transmute::<VALUE, Value>(self.inner.obj) }
    }

    pub fn byte_size(&self) -> Result<usize, Error> {
        usize::try_from(self.inner.byte_size).map_err(|err| {
            let obj = self.obj();
            let ruby = Ruby::get_with(obj);
            Error::new(ruby.exception_runtime_error(), format!("{err}"))
        })
    }

    pub fn is_readonly(&self) -> bool {
        self.inner.readonly
    }

    pub fn format(&self) -> Option<&str> {
        if self.inner.format.is_null() {
            None
        } else {
            unsafe { CStr::from_ptr(self.inner.format).to_str() }.ok()
        }
    }

    pub fn data<T>(&self) -> Result<&[T], Error> {
        let n_items = self.byte_size()? / size_of::<T>();
        let data = self.inner.data;
        if data.is_null() {
            let ruby = Ruby::get_with(self.obj());
            return Err(Error::new(ruby.exception_runtime_error(), "data is NULL"));
        }
        let ptr = data.cast::<T>();
        if !ptr.is_aligned() {
            let ruby = Ruby::get_with(self.obj());
            return Err(Error::new(
                ruby.exception_runtime_error(),
                "data not aligned",
            ));
        }

        Ok(unsafe { slice::from_raw_parts(ptr, n_items) })
    }

    pub fn data_as_mut<T>(&mut self) -> Result<&mut [T], Error> {
        if self.is_readonly() {
            let ruby = Ruby::get_with(self.obj());
            return Err(Error::new(ruby.exception_runtime_error(), "not mutable"));
        }
        let n_items = self.byte_size()? / size_of::<T>();
        let data = self.inner.data;
        if data.is_null() {
            let ruby = Ruby::get_with(self.obj());
            return Err(Error::new(ruby.exception_runtime_error(), "data is NULL"));
        }
        let ptr = data.cast::<T>();
        if !ptr.is_aligned() {
            let ruby = Ruby::get_with(self.obj());
            return Err(Error::new(
                ruby.exception_runtime_error(),
                "data not aligned",
            ));
        }

        Ok(unsafe { slice::from_raw_parts_mut(ptr, n_items) })
    }
}
