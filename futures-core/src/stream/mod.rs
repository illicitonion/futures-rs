//! Asynchronous streams.

use core::marker::Unpin;
use core::mem::PinMut;

use Poll;
use task;

/// A stream of values produced asynchronously.
///
/// If `Future<Output = T>` is an asynchronous version of `T`, then `Stream<Item
/// = T>` is an asynchronous version of `Iterator<Item = T>`. A stream
/// represents a sequence of value-producing events that occur asynchronously to
/// the caller.
///
/// The trait is modeled after `Future`, but allows `poll_next` to be called
/// even after a value has been produced, yielding `None` once the stream has
/// been fully exhausted.
pub trait Stream: Unpin {
    /// Values yielded by the stream.
    type Item;

    /// Attempt to pull out the next value of this stream, registering the
    /// current task for wakeup if the value is not yet available, and returning
    /// `None` if the stream is exhausted.
    ///
    /// # Return value
    ///
    /// There are several possible return values, each indicating a distinct
    /// stream state:
    ///
    /// - [`Pending`](::Poll) means that this stream's next value is not ready
    /// yet. Implementations will ensure that the current task will be notified
    /// when the next value may be ready.
    ///
    /// - [`Ready(Some(val))`](::Poll) means that the stream has successfully
    /// produced a value, `val`, and may produce further values on subsequent
    /// `poll_next` calls.
    ///
    /// - [`Ready(None)`](::Poll) means that the stream has terminated, and
    /// `poll_next` should not be invoked again.
    ///
    /// # Panics
    ///
    /// Once a stream is finished, i.e. `Ready(None)` has been returned, further
    /// calls to `poll_next` may result in a panic or other "bad behavior".  If this
    /// is difficult to guard against then the `fuse` adapter can be used to
    /// ensure that `poll_next` always returns `Ready(None)` in subsequent calls.
    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>>;
}

impl<'a, S: ?Sized + Stream + Unpin> Stream for &'a mut S {
    type Item = S::Item;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
        (**self).poll_next(cx)
    }
}

impl<'a, S: ?Sized + Stream> Stream for PinMut<'a, S> {
    type Item = S::Item;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
        (**self).poll_next(cx)
    }
}

if_std! {
    use std::boxed::{Box, PinBox};

    impl<S: ?Sized + Stream> Stream for Box<S> {
        type Item = S::Item;

        fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
            (**self).poll_next(cx)
        }
    }

    impl<S: ?Sized + Stream> Stream for PinBox<S> {
        type Item = S::Item;

        fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
            (**self).poll_next(cx)
        }
    }

    impl<S: Stream> Stream for ::std::panic::AssertUnwindSafe<S> {
        type Item = S::Item;

        fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<S::Item>> {
            self.0.poll_next(cx)
        }
    }

    impl<T: Unpin> Stream for ::std::collections::VecDeque<T> {
        type Item = T;

        fn poll_next(&mut self, _cx: &mut task::Context) -> Poll<Option<Self::Item>> {
            Poll::Ready(self.pop_front())
        }
    }
}