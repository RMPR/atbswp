//! Tiny `dlopen` helper for the Linux recorders, so no system library is a
//! build-time or link-time dependency: PipeWire and libdbus are looked up at
//! run time, exactly like the player does with libei.

use std::ffi::{CString, c_void};

pub struct Lib(*mut c_void);
unsafe impl Send for Lib {}
unsafe impl Sync for Lib {}

impl Lib {
    pub fn open(name: &str) -> Result<Lib, String> {
        let c = CString::new(name).unwrap();
        let h = unsafe { libc::dlopen(c.as_ptr(), libc::RTLD_NOW) };
        if h.is_null() {
            return Err(format!("{name} is not available on this system"));
        }
        Ok(Lib(h))
    }

    /// Look up a symbol; the caller supplies the function pointer type.
    pub fn sym<T: Copy>(&self, name: &str) -> Result<T, String> {
        assert_eq!(std::mem::size_of::<T>(), std::mem::size_of::<*mut c_void>());
        let c = CString::new(name).unwrap();
        let p = unsafe { libc::dlsym(self.0, c.as_ptr()) };
        if p.is_null() {
            return Err(format!("missing symbol {name}"));
        }
        Ok(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&p) })
    }
}

/// Declares a struct of function pointers plus a loader for them.
macro_rules! dl_api {
    ($vis:vis struct $name:ident from $lib:literal { $( $fn:ident : $ty:ty ),* $(,)? }) => {
        #[allow(non_snake_case, dead_code)]
        $vis struct $name {
            _lib: $crate::record::dl::Lib,
            $( $vis $fn: $ty, )*
        }
        impl $name {
            $vis fn load() -> Result<Self, String> {
                let lib = $crate::record::dl::Lib::open($lib)?;
                Ok(Self {
                    $( $fn: lib.sym::<$ty>(stringify!($fn))?, )*
                    _lib: lib,
                })
            }
        }
    };
}
pub(crate) use dl_api;
