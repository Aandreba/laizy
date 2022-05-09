use core::{mem::MaybeUninit, sync::atomic::{Ordering, AtomicU8}, cell::UnsafeCell};
use core::pin::Pin;
use futures::{Future, task::AtomicWaker};
use crate::utils::AwaitInit;

#[cfg(not(debug_assertions))]
use core::hint::unreachable_unchecked;

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;

cfg_if::cfg_if! {
    if #[cfg(any(feature = "std", feature = "alloc"))] {
        #[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
        pub type DynFuture<'a, T> = Pin<Box<dyn 'a + Future<Output = T> + Sync>>;

        #[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
        pub type DynAsyncLazy<'a, T, F = fn() -> DynFuture<'a, T>> = AsyncLazy<T, DynFuture<'a, T>, F>;

        impl<T, F: FnOnce() -> dyn Future<Output = T>> DynAsyncLazy<'_, T, F> {
            #[inline(always)]
            pub const fn new_boxed<Fn: > (f: F) -> Self {
                let f = move || Box::pin(f);
                Self::new()
            }
        }
    }
}

/// A lazy value that initializes via future
pub struct AsyncLazy<T, Fut, F = fn() -> Fut> {
    state: AtomicU8,
    waker: AtomicWaker,
    f: UnsafeCell<MaybeUninit<F>>,
    fut: UnsafeCell<MaybeUninit<Fut>>,
    value: UnsafeCell<MaybeUninit<T>>
}

impl<T, Fut, F> AsyncLazy<T, Fut, F> {
    #[inline(always)]
    pub const fn new (f: F) -> Self {
        Self {
            state: AtomicU8::new(0),
            waker: AtomicWaker::new(),
            f: UnsafeCell::new(MaybeUninit::new(f)),
            fut: UnsafeCell::new(MaybeUninit::uninit()),
            value: UnsafeCell::new(MaybeUninit::uninit())
        }
    }

    #[inline(always)]
    pub const fn by_future (fut: Fut) -> Self {
        Self {
            state: AtomicU8::new(2),
            waker: AtomicWaker::new(),
            f: UnsafeCell::new(MaybeUninit::uninit()),
            fut: UnsafeCell::new(MaybeUninit::new(fut)),
            value: UnsafeCell::new(MaybeUninit::uninit())
        }
    }
}

impl<T, Fut: Future<Output = T>, F: FnOnce() -> Fut> AsyncLazy<T, Fut, F> {
    pub async fn get (&self) -> &Fut::Output {
        self.init_fut().await;
        
        match self.state.compare_exchange(2, 3, Ordering::AcqRel, Ordering::Relaxed) {
            // uninitialized
            Ok(2) => unsafe {
                let fut = core::mem::replace(&mut *self.fut.get(), MaybeUninit::uninit());
                (&mut *self.value.get()).write(fut.assume_init().await);

                #[cfg(debug_assertions)]
                assert_eq!(self.state.swap(4, Ordering::Release), 3);
                #[cfg(not(debug_assertions))]
                self.state.store(4, Ordering::Release);
            },

            // currently initializing
            Ok(3) => AwaitInit::new(4, &self.state, &self.waker).await,

            // initialized
            Ok(4) => {},

            #[cfg(debug_assertions)]
            _ => unreachable!(),
            #[cfg(not(debug_assertions))]
            _ => unsafe { unreachable_unchecked() },
        }

        unsafe { (&*self.value.get()).assume_init_ref() }
    }

    pub async fn get_mut (&mut self) -> &mut Fut::Output {
        self.init_fut().await;
        
        match self.state.compare_exchange(2, 3, Ordering::AcqRel, Ordering::Relaxed) {
            // uninitialized
            Ok(2) => unsafe {
                let fut = core::mem::replace(&mut *self.fut.get(), MaybeUninit::uninit());
                (&mut *self.value.get()).write(fut.assume_init().await);
                self.waker.wake();

                #[cfg(debug_assertions)]
                assert_eq!(self.state.swap(4, Ordering::Release), 3);
                #[cfg(not(debug_assertions))]
                self.state.store(4, Ordering::Release);
            },

            // currently initializing
            Ok(3) => AwaitInit::new(4, &self.state, &self.waker).await,

            // initialized
            Ok(4) => {},

            #[cfg(debug_assertions)]
            _ => unreachable!(),
            #[cfg(not(debug_assertions))]
            _ => unsafe { unreachable_unchecked() },
        }

        unsafe { self.value.get_mut().assume_init_mut() }
    }

    /// Asserts that the future is initialized, and initializes it if needed
    async fn init_fut (&self) {
        match self.state.compare_exchange(0, 1, Ordering::AcqRel, Ordering::Relaxed) {
            #[cfg(debug_assertions)]
            Err(_) => unreachable!(),
            #[cfg(not(debug_assertions))]
            Err(_) => unsafe { unreachable_unchecked() },

            // uninitialized
            Ok(0) => unsafe {
                let f = core::mem::replace(&mut *self.f.get(), MaybeUninit::uninit());
                (&mut *self.fut.get()).write((f.assume_init())());
                self.waker.wake();

                #[cfg(debug_assertions)]
                assert_eq!(self.state.swap(2, Ordering::Release), 1);
                #[cfg(not(debug_assertions))]
                self.state.store(2, Ordering::Release);
            },

            // currently initializing
            Ok(1) => AwaitInit::new(2, &self.state, &self.waker).await,

            // initialized
            _ => {}
        }
    }
}

impl<T, Fut, F> Drop for AsyncLazy<T, Fut, F> {
    #[inline(always)]
    fn drop(&mut self) {
        match self.state.load(Ordering::Relaxed) {
            // future uninit
            0 => unsafe { self.f.get_mut().assume_init_drop() },

            // initializing future
            1 => todo!(),

            // future init
            2 => unsafe { self.fut.get_mut().assume_init_drop() },

            // initializing value
            3 => todo!(),

            // value init
            4 => unsafe { self.value.get_mut().assume_init_drop() },

            #[cfg(debug_assertions)]
            _ => unreachable!(),
            #[cfg(not(debug_assertions))]
            _ => unsafe { unreachable_unchecked() }
        }
    }
}

unsafe impl<T: Sync, Fut: Sync, F: Sync> Sync for AsyncLazy<T, Fut, F> {}