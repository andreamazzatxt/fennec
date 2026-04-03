use std::ffi::c_void;

// FFI to our C shim
extern "C" {
    fn accel_init();
    fn accel_run();
    fn accel_set_callback(cb: unsafe extern "C" fn(*mut c_void, f64, f64, f64), ctx: *mut c_void);
}

pub fn start<F>(on_sample: F) -> Result<(), String>
where
    F: Fn(f64, f64, f64) + Send + 'static,
{
    unsafe {
        accel_init();

        let callback: Box<Box<dyn Fn(f64, f64, f64)>> = Box::new(Box::new(on_sample));
        let ctx = Box::into_raw(callback) as *mut c_void;
        accel_set_callback(rust_callback, ctx);

        // This blocks forever (CFRunLoop)
        accel_run();
    }

    Ok(())
}

unsafe extern "C" fn rust_callback(ctx: *mut c_void, x: f64, y: f64, z: f64) {
    let cb = &*(ctx as *const Box<dyn Fn(f64, f64, f64)>);
    cb(x, y, z);
}
