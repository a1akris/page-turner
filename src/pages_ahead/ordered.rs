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
        return Ok(None);
    }

    if state.futures.is_empty() {
        match state.req_chunks.next_chunk() {
            // If chunk is some then there is at least 1 request inside
            Some(chunk) => {
                for req in chunk {
                    state.futures.push_back(state.page_turner.turn_page(req));
                }
            }
            None => {
                return Ok(None);
            }
        }
    }

    match state.futures.try_next().await? {
        Some(TurnedPage {
            items,
            next_request,
        }) => {
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
        return Ok(None);
    }

    if state.futures.is_empty() {
        match state.req_chunks.next_chunk() {
            // If chunk is some then there is at least 1 request inside
            Some(chunk) => {
                for req in chunk {
                    let local_page_turner = Arc::clone(&state.page_turner);

                    state
                        .futures
                        .push_back(async move { local_page_turner.turn_page(req).await }.boxed());
                }
            }
            None => {
                return Ok(None);
            }
        }
    }

    match state.futures.try_next().await? {
        Some(TurnedPage {
            items,
            next_request,
        }) => {
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
        }
    }
}
