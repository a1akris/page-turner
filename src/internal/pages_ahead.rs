macro_rules! pages_ahead_state_def {
    ($($extra_bounds:tt)*) => {
        struct PagesAheadState<'p, P, R>
        where
            P: 'p + PageTurner<R>,
            $($extra_bounds)*
        {
            page_turner: P,
            requests: RequestChunks<R>,
            in_progress: FuturesOrdered<PageTurnerFuture<'p, P, R>>,
            last_page_queried: bool,
        }

        impl<'p, P, R> PagesAheadState<'p, P, R>
        where
            P: 'p + PageTurner<R>,
            R: 'p + RequestAhead,
            $($extra_bounds)*
        {
            pub fn new(page_turner: P, request: R, chunk_size: usize, limit: Limit) -> Self {
                let requests = RequestIter::new(request, limit).chunks(chunk_size);
                Self {
                    page_turner,
                    requests,
                    in_progress: FuturesOrdered::new(),
                    last_page_queried: false,
                }
            }
        }
    };
}

macro_rules! request_pages_ahead_decl {
    ($($extra_bounds:tt)*) => {
        async fn request_pages_ahead<'p, P, R>(
            mut state: Box<PagesAheadState<'p, P, R>>,
        ) -> Result<Option<(PageItems<P, R>, Box<PagesAheadState<'p, P, R>>)>, PageError<P, R>>
        where
            P: 'p + Clone + PageTurner<R>,
            R: 'p + RequestAhead,
            $($extra_bounds)*
        {
            if state.last_page_queried {
                return Ok(None);
            }

            if state.in_progress.is_empty() {
                match state.requests.next_chunk() {
                    // If chunk is some then there is at least 1 request inside
                    Some(chunk) => {
                        for req in chunk {
                            let local_page_turner = state.page_turner.clone();
                            state.in_progress.push_back(Box::pin(async move {
                                local_page_turner.turn_page(req).await
                            }));
                        }
                    }
                    None => {
                        return Ok(None);
                    }
                }
            } else {
                // At this point the first request succeeded. Lets push the next one from the next_chunk to proceed in
                // a sliding window maner.
                if let Some(req) = state.requests.next_item() {
                    let local_page_turner = state.page_turner.clone();
                    state.in_progress.push_back(Box::pin(
                        async move { local_page_turner.turn_page(req).await },
                    ))
                }
            }

            match state.in_progress.try_next().await? {
                Some(TurnedPage {
                    items,
                    next_request,
                }) => {
                    state.last_page_queried = next_request.is_none();
                    Ok(Some((items, state)))
                }
                None => {
                    unreachable!(
                        "BUG(page-turner): We ensured that the ordered futures queue is not empty right above"
                    )
                }
            }
        }

    };
}

pub(crate) use pages_ahead_state_def;
pub(crate) use request_pages_ahead_decl;
