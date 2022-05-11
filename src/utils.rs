cfg_if::cfg_if! {
    if #[cfg(feature = "futures")] {
        use core::sync::atomic::AtomicU8;
        use core::{pin::Pin, task::{Context, Poll}};
        use futures::{Future};
        use futures::task::AtomicWaker;

        /// Flag awaiter
        pub struct AwaitInit<'a> {
            state: &'a AtomicU8,
            waker: &'a AtomicWaker,
            target: u8
        }

        impl<'a> AwaitInit<'a> {
            #[inline(always)]
            pub const fn new (target: u8, state: &'a AtomicU8, waker: &'a AtomicWaker) -> Self {
                Self {
                    state,
                    waker,
                    target
                }
            }
        }

        impl Future for AwaitInit<'_> {
            type Output = ();

            #[inline(always)]
            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                self.waker.register(cx.waker());

                if self.state.load(core::sync::atomic::Ordering::Acquire) == self.target {
                    return Poll::Ready(())
                }

                Poll::Pending
            }
        }
    }
}