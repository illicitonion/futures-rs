use crate::stream::{StreamExt, Fuse};
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::stream::{Stream, TryStream};
use futures_core::task::{Context, Poll};
use futures_sink::Sink;
use pin_utils::{unsafe_pinned, unsafe_unpinned};

const INVALID_POLL: &str = "polled `Forward` after completion";

/// Future for the [`forward`](super::StreamExt::forward) method.
#[derive(Debug)]
#[must_use = "steams do nothing unless polled"]
pub struct Forward<St: TryStream, Si: Sink<St::Ok>> {
    sink: Option<Si>,
    stream: Fuse<St>,
    buffered_item: Option<St::Ok>,
}

impl<St: TryStream + Unpin, Si: Sink<St::Ok> + Unpin> Unpin for Forward<St, Si> {}

impl<St, Si> Forward<St, Si>
where
    Si: Sink<St::Ok, SinkError = St::Error>,
    St: TryStream + Stream,
{
    unsafe_pinned!(sink: Option<Si>);
    unsafe_pinned!(stream: Fuse<St>);
    unsafe_unpinned!(buffered_item: Option<St::Ok>);

    pub(super) fn new(stream: St, sink: Si) -> Self {
        Forward {
            sink: Some(sink),
            stream: stream.fuse(),
                buffered_item: None,
        }
    }

    fn try_start_send(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        item: St::Ok,
    ) -> Poll<Result<(), Si::SinkError>> {
        debug_assert!(self.buffered_item.is_none());
        {
            let mut sink = self.as_mut().sink().as_pin_mut().unwrap();
            if sink.as_mut().poll_ready(cx)?.is_ready() {
                return Poll::Ready(sink.start_send(item));
            }
        }
        *self.as_mut().buffered_item() = Some(item);
        Poll::Pending
    }
}

impl<St: TryStream, Si: Sink<St::Ok> + Unpin> FusedFuture for Forward<St, Si> {
    fn is_terminated(&self) -> bool {
        self.sink.is_none()
    }
}

impl<St, Si, Item, E> Future for Forward<St, Si>
where
    Si: Sink<Item, SinkError = E>,
    St: Stream<Item = Result<Item, E>>,
{
    type Output = Result<(), Si::SinkError>;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        // If we've got an item buffered already, we need to write it to the
        // sink before we can do anything else
        if let Some(item) = self.as_mut().buffered_item().take() {
            ready!(self.as_mut().try_start_send(cx, item))?;
        }

        loop {
            match self.as_mut().stream().poll_next(cx) {
                Poll::Ready(Some(Ok(item))) =>
                   ready!(self.as_mut().try_start_send(cx, item))?,
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Err(e)),
                Poll::Ready(None) => {
                    ready!(self.as_mut().sink().as_pin_mut().expect(INVALID_POLL).poll_close(cx))?;
                    self.as_mut().sink().set(None);
                    return Poll::Ready(Ok(()))
                }
                Poll::Pending => {
                    ready!(self.as_mut().sink().as_pin_mut().expect(INVALID_POLL).poll_flush(cx))?;
                    return Poll::Pending
                }
            }
        }
    }
}
