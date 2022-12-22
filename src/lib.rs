//! Simple RAII scope guard
//!
//! ## Usage:
//!
//! #### No argument guard
//! ```
//! use scope_guard::scope_guard;
//!
//! let mut is_run = false;
//! {
//!      scope_guard!(|| {
//!          is_run = true;
//!      });
//! }
//! assert!(is_run);
//! ```
//!
//! #### Single argument guard
//!
//! ```
//! use scope_guard::scope_guard;
//!
//! fn do_stuff(val: &mut u32)  {
//!     let old_val = *val;
//!     let mut val = scope_guard!(|val| {
//!         *val = old_val;
//!     }, val);
//!
//!     **val += 1; //Double * to deref &mut u32
//!
//!     let is_ok = false;//doing some computations
//!     if is_ok {
//!         val.forget();
//!     }
//! }
//!
//! let mut val = 0;
//! do_stuff(&mut val);
//! assert_eq!(val, 0);
//!
//! let mut guard = scope_guard!(|val| {
//!     *val = 1;
//! }, &mut val);
//! drop(guard);
//! assert_eq!(val, 1);
//! ```
//!
//! #### Stacked destructor calls
//!
//! ```
//! use scope_guard::scope_guard;
//!
//! fn do_stuff(val: &mut u32)  {
//!     let old_value = *val;
//!     let val = scope_guard!(|val| {
//!         assert_eq!(*val, old_value);
//!         //Do nothing
//!     }, val);
//!
//!     let mut val = val.stack(|val| {
//!         **val = old_value;
//!     });
//!
//!     **val += 1; //Double * to deref &mut u32
//! }
//!
//! let mut val = 0;
//! do_stuff(&mut val);
//! assert_eq!(val, 0);
//! ```
//!
//! #### Multiple argument guard
//!
//! ```
//! use scope_guard::scope_guard;
//!
//! fn do_stuff(val: &mut u32, is_run: &mut bool)  {
//!     let old_val = *val;
//!     let mut guard = scope_guard!(|(val, is_run)| {
//!         *val = old_val;
//!         *is_run = false;
//!     }, val, is_run);
//!
//!     *guard.0 += 1;
//!     *guard.1 = true;
//!
//!     let is_ok = false; //doing some computations
//!     if is_ok {
//!         let (_val, _is_run) = guard.into_inner(); //analogues to forget
//!     }
//! }
//!
//! let mut is_run = false;
//! let mut val = 0;
//! do_stuff(&mut val, &mut is_run);
//! assert_eq!(val, 0);
//! assert!(!is_run);
//!
//! let mut guard = scope_guard!(|(val, is_run)| {
//!     *val = 1;
//!     *is_run = true;
//! }, &mut val, &mut is_run);
//!
//! drop(guard);
//! assert_eq!(val, 1);
//! assert!(is_run);
//! ```

#![no_std]
#![warn(missing_docs)]
#![cfg_attr(feature = "cargo-clippy", allow(clippy::style))]
#![cfg_attr(feature = "cargo-clippy", allow(clippy::explicit_auto_deref))]

use core::{ptr, mem};

#[cfg(feature = "std")]
mod async_scope;
#[cfg(feature = "std")]
pub use async_scope::async_scope;

///RAII Scope, running closure in destructor.
pub struct Scope<T, F: FnOnce(T)> {
    val: mem::ManuallyDrop<T>,
    dtor: mem::ManuallyDrop<F>
}

impl<T, F: FnOnce(T)> Scope<T, F> {
    #[inline(always)]
    ///Creates new instance
    pub fn new(val: T, dtor: F) -> Self {
        Self {
            val: mem::ManuallyDrop::new(val),
            dtor: mem::ManuallyDrop::new(dtor),
        }
    }

    #[inline(always)]
    fn get_value(&self) -> T {
        unsafe {
            ptr::read(&*self.val)
        }
    }

    #[inline(always)]
    fn get_dtor(&self) -> F {
        unsafe {
            ptr::read(&*self.dtor)
        }
    }

    #[inline]
    ///Returns underlying data, without executing destructor;
    pub fn into_inner(self) -> T {
        let value = self.get_value();
        self.forget();
        value
    }

    #[inline]
    ///Forgets self, preventing closure from running
    pub fn forget(self) {
        self.get_dtor();
        mem::forget(self);
    }
}

impl<T, F: FnOnce(T)> Scope<T, F> {
    ///Adds new function to be invoked in scope of the guard.
    ///
    ///This function is executed before current one.
    ///Similarly to how stack variables dtors are invoked in reverse order.
    ///
    ///Note that stacked function cannot take guarded by value, only original function will retain
    ///owned value.
    pub fn stack<NF: FnOnce(&mut T)>(self, dtor: NF) -> Scope<T, impl FnOnce(T)> {
        let current_dtor = self.get_dtor();
        let value = self.get_value();
        mem::forget(self);
        Scope::new(value, move |mut value| {
            dtor(&mut value);
            current_dtor(value)
        })
    }
}

impl<T, F: FnOnce(T)> core::ops::Deref for Scope<T, F> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &*self.val
    }
}

impl<T, F: FnOnce(T)> core::ops::DerefMut for Scope<T, F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.val
    }
}

impl<T, F: FnOnce(T)> Drop for Scope<T, F> {
    #[inline(always)]
    fn drop(&mut self) {
        let val = self.get_value();
        let func = self.get_dtor();
        func(val);
    }
}

#[macro_export]
///Creates scope guard, allowing to supply plain function with arguments in addition to
///closures.
macro_rules! scope_guard {
    ($dtor:expr) => {
        $crate::Scope::new((), |_| $dtor())
    };
    ($dtor:expr, $arg:expr) => {
        $crate::Scope::new($arg, $dtor)
    };
    ($dtor:expr, $($args:expr),+) => {
        $crate::Scope::new(($($args),+), $dtor)
    };
}
