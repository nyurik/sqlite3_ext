use super::{ffi, types::*, value::*, Connection};
pub use context::*;
use std::{ffi::CString, ptr, slice};

mod context;

pub trait ScalarFunction<T: ToContextResult>: Fn(&Context, &[&ValueRef]) -> Result<T> {}
impl<T: ToContextResult, X: Fn(&Context, &[&ValueRef]) -> Result<T>> ScalarFunction<T> for X {}

pub trait AggregateFunction: Default {
    type Return: ToContextResult;

    const DEFAULT_VALUE: Self::Return;

    /// Add a new row to the aggregate.
    ///
    /// This function should return the current value of the aggregate after adding the
    /// row. Note that step is not allowed to fail, and so failures must be stored and
    /// returned by [value](AggregateFunction::value).
    fn step(&mut self, context: &Context, args: &[&ValueRef]);

    /// Return the current value of the aggregate function.
    fn value(&self, context: &Context) -> Result<Self::Return>;

    /// Remove the oldest presently aggregated row.
    ///
    /// The args are the same that were passed to [AggregateFunction::step] when this row
    /// was added.
    fn inverse(&mut self, context: &Context, args: &[&ValueRef]);
}

impl Connection {
    pub fn create_scalar_function<T: ToContextResult, F: ScalarFunction<T>>(
        &self,
        name: &str,
        n_args: isize,
        flags: usize,
        func: F,
    ) -> Result<()> {
        let name = unsafe { CString::from_vec_unchecked(name.as_bytes().into()) };
        let func = Box::new(func);
        unsafe {
            Error::from_sqlite(ffi::sqlite3_create_function_v2(
                self.as_ptr(),
                name.as_ptr() as _,
                n_args as _,
                flags as _,
                Box::into_raw(func) as _,
                Some(call_scalar::<T, F>),
                None,
                None,
                Some(ffi::drop_boxed::<F>),
            ))
        }
    }

    pub fn create_aggregate_function<F: AggregateFunction + 'static>(
        &self,
        name: &str,
        n_args: isize,
        flags: usize,
    ) -> Result<()> {
        let name = unsafe { CString::from_vec_unchecked(name.as_bytes().into()) };
        unsafe {
            Error::from_sqlite(ffi::sqlite3_create_window_function(
                self.as_ptr(),
                name.as_ptr() as _,
                n_args as _,
                flags as _,
                ptr::null_mut(),
                Some(aggregate_step::<F>),
                Some(aggregate_final::<F>),
                Some(aggregate_value::<F>),
                Some(aggregate_inverse::<F>),
                None,
            ))
        }
    }
}

unsafe extern "C" fn call_scalar<T: ToContextResult, F: ScalarFunction<T>>(
    context: *mut ffi::sqlite3_context,
    argc: i32,
    argv: *mut *mut ffi::sqlite3_value,
) {
    let func = &*(ffi::sqlite3_user_data(context) as *const F);
    let context = &mut *(context as *mut InternalContext);
    let args = slice::from_raw_parts(argv as *mut &ValueRef, argc as _);
    let ret = func(&context.get(), args);
    context.set_result(ret);
}

unsafe extern "C" fn aggregate_step<F: AggregateFunction + 'static>(
    context: *mut ffi::sqlite3_context,
    argc: i32,
    argv: *mut *mut ffi::sqlite3_value,
) {
    let context = InternalContext::from_ptr(context);
    let ctx = &context.get();
    let agg = context.aggregate_context::<F>().unwrap();
    let args = slice::from_raw_parts(argv as *mut &ValueRef, argc as _);
    agg.step(ctx, args);
}

unsafe extern "C" fn aggregate_final<F: AggregateFunction + 'static>(
    context: *mut ffi::sqlite3_context,
) {
    let context = InternalContext::from_ptr(context);
    match context.try_aggregate_context::<F>() {
        Some(agg) => context.set_result(agg.value(&context.get())),
        None => context.set_result(F::DEFAULT_VALUE),
    };
}

unsafe extern "C" fn aggregate_value<F: AggregateFunction + 'static>(
    context: *mut ffi::sqlite3_context,
) {
    let context = InternalContext::from_ptr(context);
    let ctx = &context.get();
    let agg = context.aggregate_context::<F>().unwrap();
    let ret = agg.value(ctx);
    context.set_result(ret);
}

unsafe extern "C" fn aggregate_inverse<F: AggregateFunction + 'static>(
    context: *mut ffi::sqlite3_context,
    argc: i32,
    argv: *mut *mut ffi::sqlite3_value,
) {
    let context = InternalContext::from_ptr(context);
    let ctx = &context.get();
    let agg = context.aggregate_context::<F>().unwrap();
    let args = slice::from_raw_parts(argv as *mut &ValueRef, argc as _);
    agg.inverse(ctx, args);
}
