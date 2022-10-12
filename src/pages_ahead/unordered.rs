use super::{ChunksExt, Limit, PageTurner, RequestAhead, RequestChunks, RequestIter, TurnedPage};
use crate::PageTurnerOutput;
use futures::{
    future::BoxFuture,
    stream::{FuturesUnordered, TryStreamExt},
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
    loop {
        if state.futures.is_empty() {
            if state.last_page_queried {
                return Ok(None);
            }

            if let Some(err) = state.last_error {
                return Err(err);
            }

            match state.req_chunks.next_chunk() {
                // If chunk is some then there is at least 1 request inside
                Some(chunk) => {
                    for req in chunk {
                        state.futures.push(state.page_turner.turn_page(req));
                    }
                }
                None => {
                    return Ok(None);
                }
            }
        }

        loop {
            match state.futures.try_next().await {
                Ok(Some(TurnedPage {
                    items,
                    next_request,
                })) => {
                    state.last_page_queried = next_request.is_none();
                    return Ok(Some((items, state)));
                }
                Ok(None) => {
                    break;
                }
                Err(e) => {
                    state.last_error = Some(e);
                }
            }
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
    loop {
        if state.futures.is_empty() {
            if state.last_page_queried {
                return Ok(None);
            }

            if let Some(err) = state.last_error {
                return Err(err);
            }

            match state.req_chunks.next_chunk() {
                // If chunk is some then there is at least 1 request inside
                Some(chunk) => {
                    for req in chunk {
                        let local_page_turner = Arc::clone(&state.page_turner);

                        state
                            .futures
                            .push(async move { local_page_turner.turn_page(req).await }.boxed());
                    }
                }
                None => {
                    return Ok(None);
                }
            }
        }

        loop {
            match state.futures.try_next().await {
                Ok(Some(TurnedPage {
                    items,
                    next_request,
                })) => {
                    state.last_page_queried = next_request.is_none();
                    return Ok(Some((items, state)));
                }
                Ok(None) => {
                    break;
                }
                Err(e) => {
                    state.last_error = Some(e);
                }
            }
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
    futures: FuturesUnordered<BoxFuture<'p, PageTurnerOutput<P, R>>>,
    last_page_queried: bool,
    last_error: Option<<P as PageTurner<R>>::PageError>,
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
            futures: FuturesUnordered::new(),
            last_page_queried: false,
            last_error: None,
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
    futures: FuturesUnordered<BoxFuture<'static, PageTurnerOutput<P, R>>>,
    last_page_queried: bool,
    last_error: Option<<P as PageTurner<R>>::PageError>,
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
            futures: FuturesUnordered::new(),
            last_page_queried: false,
            last_error: None,
        }
    }
}
