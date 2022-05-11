#![no_std]
#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod utils;
use core::{sync::atomic::{Ordering, AtomicU8}, mem::{MaybeUninit, ManuallyDrop}, cell::{UnsafeCell}, ops::{Deref, DerefMut}};

#[cfg(not(debug_assertions))]
use core::hint::unreachable_unchecked;

cfg_if::cfg_if! {
    if #[cfg(feature = "futures")] {
        mod asnc;
        pub use asnc::*;
    }
}

/// The lazy type.
/// Lazy values aren't initialized until requested by some part of the program. 
/// When requested, ```Lazy``` will initialize the value and return a reference to it
#[derive(Debug)]
pub struct Lazy<T, F = fn() -> T> {
    state: AtomicU8,
    value: UnsafeCell<MaybeUninit<T>>,
    f: UnsafeCell<MaybeUninit<F>>
}

impl<T, F> Lazy<T, F> {
    /// Builds a new ```Lazy``` value
    #[inline(always)]
    pub const fn new (f: F) -> Self {
        Self {
            state: AtomicU8::new(0),
            value: UnsafeCell::new(MaybeUninit::uninit()),
            f: UnsafeCell::new(MaybeUninit::new(f))
        }
    }

    /// Builds a ```Lazy``` value that's already initialized
    #[inline(always)]
    pub const fn init (value: T) -> Self {
        Self {
            state: AtomicU8::new(2),
            value: UnsafeCell::new(MaybeUninit::new(value)),
            f: UnsafeCell::new(MaybeUninit::uninit())
        }
    }

    /// Returns ```true``` if the value is uninitialized, ```false``` otherwise
    #[inline(always)]
    pub fn is_uninit (&self) -> bool {
        self.state.load(Ordering::Acquire) == 0
    }
    
    /// Returns ```true``` if the value is currently initializing, ```false``` otherwise
    #[inline(always)]
    pub fn is_init (&self) -> bool {
        self.state.load(Ordering::Acquire) == 1
    }
    
    /// Returns ```true``` if the value has already initialized, ```false``` otherwise
    #[inline(always)]
    pub fn has_init (&self) -> bool {
        self.state.load(Ordering::Acquire) == 2
    }
}

impl<T, F: FnOnce() -> T> Lazy<T, F> {
    /// Returns a reference to the inner value, initializing or waiting for it of necesary
    #[inline(always)]
    pub fn get (&self) -> &T {
        match self.state.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed) {
            // uninitialized
            Ok(0) => unsafe {
                let f = core::mem::replace(&mut *self.f.get(), MaybeUninit::uninit());
                (&mut *self.value.get()).write((f.assume_init())());

                #[cfg(debug_assertions)]
                assert_eq!(self.state.swap(2, Ordering::Release), 1);
                #[cfg(not(debug_assertions))]
                self.state.store(2, Ordering::Release);
            },

            // currently initializing
            Err(1) => while self.state.load(Ordering::Acquire) == 1 { core::hint::spin_loop() },

            // initialized
            Err(2) => {},

            #[cfg(debug_assertions)]
            _ => unreachable!(),
            #[cfg(not(debug_assertions))]
            _ => unsafe { unreachable_unchecked() }
        }

        unsafe { (&*self.value.get()).assume_init_ref() }
    }

    /// Returns a mutable reference to the inner value, initializing or waiting for it of necesary
    #[inline(always)]
    pub fn get_mut (&mut self) -> &mut T {
        match self.state.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed) {
            // uninitialized
            Ok(0) => unsafe {
                let f = core::mem::replace(&mut *self.f.get(), MaybeUninit::uninit());
                self.value.get_mut().write((f.assume_init())());

                #[cfg(debug_assertions)]
                assert_eq!(self.state.swap(2, Ordering::Release), 1);
                #[cfg(not(debug_assertions))]
                self.state.store(2, Ordering::Release);
            },

            // currently initializing
            Err(1) => while self.state.load(Ordering::Acquire) == 1 { core::hint::spin_loop() },

            // initialized
            Err(2) => {},

            #[cfg(debug_assertions)]
            _ => unreachable!(),
            #[cfg(not(debug_assertions))]
            _ => unsafe { unreachable_unchecked() }
        }

        unsafe { self.value.get_mut().assume_init_mut() }
    }

    /// Returns ```Some(ref value)``` if the value has already initialized, ```None``` otherwise
    #[inline(always)]
    pub fn try_get (&self) -> Option<&T> {
        match self.state.load(Ordering::Acquire) {
            2 => unsafe { Some((&*self.value.get()).assume_init_ref()) }
            _ => None
        }
    }

    /// Returns ```Some(ref mut value)``` if the value has already initialized, ```None``` otherwise
    #[inline(always)]
    pub fn try_get_mut (&mut self) -> Option<&mut T> {
        match self.state.load(Ordering::Acquire) {
            2 => unsafe { Some(self.value.get_mut().assume_init_mut()) }
            _ => None
        }
    }

    /// Returns the inner value, initializing it if necessary
    #[inline(always)]
    pub fn into_inner (self) -> T {
        let mut this = ManuallyDrop::new(self);

        match this.state.load(Ordering::Relaxed) {
            // uninit (init value)
            0 => unsafe { 
                let f = core::mem::replace(this.f.get_mut(), MaybeUninit::uninit()).assume_init();
                f()
            },

            // initializing (shouldn't happen)
            #[cfg(debug_assertions)]
            1 => unreachable!(),
            #[cfg(not(debug_assertions))]
            1 => unsafe { unreachable_unchecked() },

            // init
            _ => unsafe {
                let value = core::mem::replace(this.value.get_mut(), MaybeUninit::uninit());
                value.assume_init()
            }
        }
    }

    /// Attempts to return the inner value, returning an error if it hasn't initialized yet. The error contains the value's initializer
    #[inline(always)]
    pub fn try_into_inner (self) -> Result<T, F> {
        let mut this = ManuallyDrop::new(self);

        match this.state.load(Ordering::Relaxed) {
            // uninit (get function)
            0 => unsafe { 
                let f = core::mem::replace(this.f.get_mut(), MaybeUninit::uninit());
                Err(f.assume_init())
            },

            // initializing (shouldn't happen)
            #[cfg(debug_assertions)]
            1 => unreachable!(),
            #[cfg(not(debug_assertions))]
            1 => unsafe { unreachable_unchecked() },

            // init (get value)
            _ => unsafe {
                let value = core::mem::replace(this.value.get_mut(), MaybeUninit::uninit());
                Ok(value.assume_init())
            }
        }
    }
}

impl<T, F: FnOnce() -> T> Deref for Lazy<T, F> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T, F: FnOnce() -> T> DerefMut for Lazy<T, F> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

impl<T: Default> Default for Lazy<T, fn() -> T> {
    #[inline(always)]
    fn default() -> Self {
        Self::new(Default::default)
    }
}

impl<T, F> From<T> for Lazy<T, F> {
    #[inline(always)]
    fn from(x: T) -> Self {
        Self::init(x)
    }
}

impl<T, F> Drop for Lazy<T, F> {
    #[inline(always)]
    fn drop(&mut self) {
        match self.state.load(Ordering::Relaxed) {
            // uninit (drop function)
            0 => return unsafe { self.f.get_mut().assume_init_drop() },

            // currently initializing (wait for value)
            1 => while self.state.load(Ordering::Acquire) == 1 { core::hint::spin_loop() },

            // init (drop value)
            _ => {},
        }

        unsafe { self.value.get_mut().assume_init_drop() }
    }
}

unsafe impl<T: Send, F: Send> Send for Lazy<T, F> {}
unsafe impl<T: Sync, F: Sync> Sync for Lazy<T, F> {}