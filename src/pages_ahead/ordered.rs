use super::{ChunksExt, Limit, PageTurner, RequestAhead, RequestChunks, RequestIter, TurnedPage};
use crate::PageTurnerOutput;
use futures::{
    future::BoxFuture,
    stream::{FuturesOrdered, TryStreamExt},
    FutureExt,
};
use std::sync::Arc;

pub async fn request_next_page<'p, P, R>(
    mut state: StreamState<'p, P, R>,
) -> Result<
    Option<(Vec<<P as PageTurner<R>>::PageItem>, StreamState<'p, P, R>)>,
    <P as PageTurner<R>>::PageError,
>
where
    P: 'p + ?Sized + PageTurner<R>,
    R: 'static + Send + RequestAhead,
{
    if state.last_page_queried {
        state.tracer.last_succeeded();
        return Ok(None);
    }

    if state.futures.is_empty() {
        match state.req_chunks.next_chunk() {
            // If chunk is some then there is at least 1 request inside
            Some(chunk) => {
                state.tracer.schedule_chunk(chunk.len());
                for req in chunk {
                    state.futures.push_back(state.page_turner.turn_page(req));
                }
            }
            None => {
                state.tracer.last_succeeded();
                return Ok(None);
            }
        }
    } else {
        // At this point the first request succeeded. Let push the next one from the next_chunk in
        // a sliding window maner.
        if let Some(req) = state.req_chunks.next_item() {
            state.tracer.schedule_single();
            state.futures.push_back(state.page_turner.turn_page(req));
        }
    }

    match state.futures.try_next().await? {
        Some(TurnedPage {
            items,
            next_request,
        }) => {
            state.tracer.complete_single();
            state.last_page_queried = next_request.is_none();
            Ok(Some((items, state)))
        }
        None => {
            unreachable!(
                "BUG(page-turner): We ensured that the futures queue is not empty right above"
            )
        }
    }
}

pub async fn request_next_page_owned<P, R>(
    mut state: StreamStateOwned<P, R>,
) -> Result<
    Option<(Vec<<P as PageTurner<R>>::PageItem>, StreamStateOwned<P, R>)>,
    <P as PageTurner<R>>::PageError,
>
where
    P: 'static + PageTurner<R>,
    R: 'static + Send + RequestAhead,
{
    if state.last_page_queried {
        state.tracer.last_succeeded();
        return Ok(None);
    }

    if state.futures.is_empty() {
        match state.req_chunks.next_chunk() {
            // If chunk is some then there is at least 1 request inside
            Some(chunk) => {
                state.tracer.schedule_chunk(chunk.len());
                for req in chunk {
                    let local_page_turner = Arc::clone(&state.page_turner);

                    state
                        .futures
                        .push_back(async move { local_page_turner.turn_page(req).await }.boxed());
                }
            }
            None => {
                state.tracer.last_succeeded();
                return Ok(None);
            }
        }
    } else {
        let local_page_turner = Arc::clone(&state.page_turner);
        // At this point the first request succeeded. Let push the next one from the next_chunk in
        // a sliding window maner.
        if let Some(req) = state.req_chunks.next_item() {
            state.tracer.schedule_single();
            state
                .futures
                .push_back(async move { local_page_turner.turn_page(req).await }.boxed());
        }
    }

    match state.futures.try_next().await? {
        Some(TurnedPage {
            items,
            next_request,
        }) => {
            state.tracer.complete_single();
            state.last_page_queried = next_request.is_none();
            Ok(Some((items, state)))
        }
        None => {
            unreachable!(
                "BUG(page-turner): We ensured that the futures queue is not empty right above"
            )
        }
    }
}

pub struct StreamState<'p, P, R>
where
    P: 'p + ?Sized + PageTurner<R>,
    R: 'static + Send + RequestAhead,
{
    page_turner: &'p P,
    req_chunks: RequestChunks<R>,
    futures: FuturesOrdered<BoxFuture<'p, PageTurnerOutput<P, R>>>,
    last_page_queried: bool,
    tracer: FuturesTracer,
}

impl<'p, P, R> StreamState<'p, P, R>
where
    P: 'p + ?Sized + PageTurner<R>,
    R: 'static + Send + RequestAhead,
{
    pub fn new(page_turner: &'p P, request: R, chunk_size: usize, limit: Limit) -> Self {
        let req_chunks = RequestIter::new(request, limit).chunks(chunk_size);
        Self {
            page_turner,
            req_chunks,
            futures: FuturesOrdered::new(),
            last_page_queried: false,
            tracer: FuturesTracer::default(),
        }
    }
}

pub struct StreamStateOwned<P, R>
where
    P: 'static + PageTurner<R>,
    R: 'static + Send + RequestAhead,
{
    page_turner: Arc<P>,
    req_chunks: RequestChunks<R>,
    futures: FuturesOrdered<BoxFuture<'static, PageTurnerOutput<P, R>>>,
    last_page_queried: bool,
    tracer: FuturesTracer,
}

impl<P, R> StreamStateOwned<P, R>
where
    P: 'static + PageTurner<R>,
    R: 'static + Send + RequestAhead,
{
    pub fn new(page_turner: P, request: R, chunk_size: usize, limit: Limit) -> Self {
        let req_chunks = RequestIter::new(request, limit).chunks(chunk_size);
        Self {
            page_turner: Arc::new(page_turner),
            req_chunks,
            futures: FuturesOrdered::new(),
            last_page_queried: false,
            tracer: FuturesTracer::default(),
        }
    }
}

#[cfg(feature = "trace-logs")]
#[derive(Default, Clone, Copy)]
struct FuturesTracer {
    succeeded_requests_count: usize,
    scheduled_requests_count: usize,
}

#[cfg(feature = "trace-logs")]
impl FuturesTracer {
    fn schedule_chunk(&mut self, chunk_size: usize) {
        self.scheduled_requests_count += chunk_size;
        log::trace!(
            "pages_ahead: scheduling initial requests in a chunk of size {chunk_size}\n{self:?}"
        );
    }

    fn schedule_single(&mut self) {
        self.scheduled_requests_count += 1;
        log::trace!("pages_ahead: scheduling one more future in a sliding window maner\n{self:?}");
    }

    fn complete_single(&mut self) {
        self.succeeded_requests_count += 1;
        log::trace!("pages_ahead: A future succeeded\n{self:?}");
    }

    fn last_succeeded(&self) {
        log::trace!("pages_ahead: The last future succeeded\n{self:?}");
    }
}

#[cfg(feature = "trace-logs")]
impl std::fmt::Debug for FuturesTracer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        struct RngDebug(std::ops::RangeInclusive<usize>);
        impl std::fmt::Debug for RngDebug {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_list()
                    .entries(self.0.clone().map(|i| i.to_string()))
                    .finish()
            }
        }

        let mut f = f.debug_list();

        fn fmt_completed(i: usize) -> String {
            format!("{i}+")
        }

        if self.succeeded_requests_count < 3 {
            f.entries((1..=self.succeeded_requests_count).map(fmt_completed));
        } else {
            f.entry(&"..");
            f.entries(
                (self.succeeded_requests_count - 1..=self.succeeded_requests_count)
                    .map(fmt_completed),
            );
        }

        f.entry(&RngDebug(
            self.succeeded_requests_count + 1..=self.scheduled_requests_count,
        ))
        .finish()
    }
}

#[cfg(not(feature = "trace-logs"))]
#[derive(Default, Clone, Copy)]
struct FuturesTracer;

#[cfg(not(feature = "trace-logs"))]
impl FuturesTracer {
    fn schedule_chunk(&mut self, chunk_size: usize) {}

    fn schedule_single(&mut self) {}

    fn complete_single(&mut self) {}

    fn last_succeeded(&self) {}
}

#[cfg(all(test, features = "trace-logs"))]
mod tests {
    use super::FuturesTracer;

    #[test]
    fn futures_tracer() {
        let mut tracer = FuturesTracer::default();
        tracer.schedule_chunk(6);

        assert_eq!(
            format!("{tracer:?}"),
            "[[\"1\", \"2\", \"3\", \"4\", \"5\", \"6\"]]"
        );

        tracer.complete_single();
        tracer.schedule_single();

        assert_eq!(
            format!("{tracer:?}"),
            "[\"1+\", [\"2\", \"3\", \"4\", \"5\", \"6\", \"7\"]]"
        );
    }
}
