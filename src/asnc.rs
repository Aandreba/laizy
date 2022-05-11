use core::{mem::MaybeUninit, sync::atomic::{Ordering, AtomicU8}, cell::UnsafeCell};
use core::{mem::ManuallyDrop};
use futures::{Future, task::AtomicWaker};
use crate::{utils::{AwaitInit}};

#[cfg(not(debug_assertions))]
use core::hint::unreachable_unchecked;

/// A lazy value that initializes via future
#[cfg_attr(docsrs, doc(cfg(feature = "futures")))]
#[derive(Debug)]
pub struct AsyncLazy<T, F> {
    state: AtomicU8,
    waker: AtomicWaker,
    value: UnsafeCell<MaybeUninit<T>>,
    f: UnsafeCell<MaybeUninit<F>>
}

impl<T, F> AsyncLazy<T, F> {
    /// Builds a new ```AsyncLazy``` value
    #[inline(always)]
    pub const fn new (f: F) -> Self {
        Self {
            state: AtomicU8::new(0),
            waker: AtomicWaker::new(),
            value: UnsafeCell::new(MaybeUninit::uninit()),
            f: UnsafeCell::new(MaybeUninit::new(f))
        }
    }

    /// Builds an ```AsyncLazy``` value that's already initialized
    #[inline(always)]
    pub const fn init (value: T) -> Self {
        Self {
            state: AtomicU8::new(2),
            waker: AtomicWaker::new(),
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
        self.state.load(Ordering::Acquire) > 1
    }
}

impl<T, F: Future<Output = T>> AsyncLazy<T, F> {
    /// Returns a reference to the inner value, initializing or waiting for it of necesary
    #[inline(always)]
    pub async fn get (&self) -> &T {
        match self.state.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed) {
            // uninitialized
            Ok(0) => unsafe {
                let f = core::mem::replace(&mut *self.f.get(), MaybeUninit::uninit());
                (&mut *self.value.get()).write(f.assume_init().await);

                #[cfg(debug_assertions)]
                assert_eq!(self.state.swap(2, Ordering::Release), 1);
                #[cfg(not(debug_assertions))]
                self.state.store(2, Ordering::Release);
                self.waker.wake();
            },

            // currently initializing
            Err(1) => AwaitInit::new(2, &self.state, &self.waker).await,

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
    pub async fn get_mut (&mut self) -> &mut T {
        match self.state.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed) {
            // uninitialized
            Ok(0) => unsafe {
                let f = core::mem::replace(&mut *self.f.get(), MaybeUninit::uninit());
                (&mut *self.value.get()).write(f.assume_init().await);

                #[cfg(debug_assertions)]
                assert_eq!(self.state.swap(2, Ordering::Release), 1);
                #[cfg(not(debug_assertions))]
                self.state.store(2, Ordering::Release);
                self.waker.wake();
            },

            // currently initializing
            Err(1) => AwaitInit::new(2, &self.state, &self.waker).await,

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
    pub async fn into_inner (self) -> T {
        let mut this = ManuallyDrop::new(self);

        match this.state.load(Ordering::Relaxed) {
            // uninit (init value)
            0 => unsafe { 
                let f = core::mem::replace(this.f.get_mut(), MaybeUninit::uninit()).assume_init();
                f.await
            },

            // currently initializing
            1 => unsafe {
                AwaitInit::new(2, &this.state, &this.waker).await;
                let value = core::mem::replace(this.value.get_mut(), MaybeUninit::uninit());
                value.assume_init()
            },

            // init
            _ => unsafe {
                let value = core::mem::replace(this.value.get_mut(), MaybeUninit::uninit());
                value.assume_init()
            }
        }
    }
}

impl<T, F> From<T> for AsyncLazy<T, F> {
    #[inline(always)]
    fn from(x: T) -> Self {
        Self::init(x)
    }
}

impl<T, F> Drop for AsyncLazy<T, F> {
    #[inline(always)]
    fn drop(&mut self) {
        match self.state.load(Ordering::Relaxed) {
            // uninit (drop future)
            0 => return unsafe { self.f.get_mut().assume_init_drop() },

            // currently initializing
            1 => while self.state.load(Ordering::Acquire) == 1 { core::hint::spin_loop() },

            // init (drop value)
            _ => {}
        }

        unsafe { self.value.get_mut().assume_init_drop() }
    }
}

unsafe impl<T: Send, F: Send> Send for AsyncLazy<T, F> {}
unsafe impl<T: Sync, F: Sync> Sync for AsyncLazy<T, F> {}

/// Creates a new ```AsyncLazy``` without having to specify the future's return type
#[cfg_attr(docsrs, doc(cfg(feature = "futures")))]
#[cfg(feature = "nightly")]
#[inline(always)]
pub const fn async_lazy<F: Future> (f: F) -> AsyncLazy<F::Output, F> {
    AsyncLazy::new(f)
}

/// Creates a new ```AsyncLazy``` without having to specify the future's return type
#[cfg_attr(docsrs, doc(cfg(feature = "futures")))]
#[cfg(not(feature = "nightly"))]
#[inline(always)]
pub fn async_lazy<F: Future> (f: F) -> AsyncLazy<F::Output, F> {
    AsyncLazy::new(f)
}