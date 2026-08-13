use magnus::{Error, Ruby, Value};
use rb_sys::{VALUE, rb_memory_view_get, rb_memory_view_release, rb_memory_view_t};
use std::{ffi::CStr, mem::MaybeUninit, slice};

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
    pub fn get(obj: Value, flags: i32) -> Result<Self, Error> {
        let ruby = Ruby::get_with(obj);

        let obj = unsafe { std::mem::transmute::<Value, VALUE>(obj) };
        let mut view = MaybeUninit::uninit();
        let result = unsafe { rb_memory_view_get(obj, view.as_mut_ptr(), flags) };
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
