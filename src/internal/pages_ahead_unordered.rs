macro_rules! pages_ahead_unordered_state_def {
    ($($extra_bounds:tt)*) => {
        struct PagesAheadUnorderedState<'p, P, R>
        where
            P: 'p + PageTurner<R>,
            $($extra_bounds)*
        {
            page_turner: P,
            numbered_requests: EnumerableRequestChunks<R>,
            in_progress: FuturesUnordered<NumberedRequestFuture<'p, P, R>>,
            first_error: Option<(usize, PageError<P, R>)>,
            last_page: Option<usize>,
        }

        impl<'p, P, R> PagesAheadUnorderedState<'p, P, R>
        where
            P: 'p + PageTurner<R>,
            R: 'p + RequestAhead,
            $($extra_bounds)*
        {
            fn new(page_turner: P, request: R, chunk_size: usize, limit: Limit) -> Self {
                let numbered_requests = RequestIter::new(request, limit)
                    .enumerate()
                    .chunks(chunk_size);

                Self {
                    page_turner,
                    numbered_requests,
                    in_progress: FuturesUnordered::new(),
                    first_error: None,
                    last_page: None,
                }
            }

            /// Updates the error so that an error with the least `new_err_num` remains while other ones
            /// get discarded
            fn update_err(&mut self, new_err_num: usize, new_err: PageError<P, R>) {
                match &self.first_error {
                    Some((old_err_num, _)) if new_err_num < *old_err_num => {
                        self.first_error = Some((new_err_num, new_err));
                    }
                    Some(_) => {}
                    None => self.first_error = Some((new_err_num, new_err)),
                }
            }
        }
    };
}

macro_rules! request_pages_ahead_unordered_decl {
    ($($extra_bounds:tt)*) => {
        async fn request_pages_ahead_unordered<'p, P, R>(
            mut state: Box<PagesAheadUnorderedState<'p, P, R>>,
        ) -> Result<Option<(PageItems<P, R>, Box<PagesAheadUnorderedState<'p, P, R>>)>, PageError<P, R>>
        where
            P: 'p + Clone + PageTurner<R>,
            R: 'p + RequestAhead,
            $($extra_bounds)*
        {
            // This and nested loops are required to discard all errors except the error for the first failed request without yielding them to the user.
            loop {
                // Once we're in this branch no code below will be executed
                if let Some(last_page_num) = state.last_page {
                    while let Some((num, result)) = state.in_progress.next().await {
                        match result {
                            Ok(turned_page) => return Ok(Some((turned_page.items, state))),
                            Err(new_err) => {
                                state.update_err(num, new_err);
                            }
                        }
                    }

                    match state.first_error.take() {
                        Some((err_num, err)) if err_num <= last_page_num => {
                            return Err(err);
                        }
                        // If an error occured past the last existing page it will be discarded at this
                        // point
                        _ => {
                            return Ok(None);
                        }
                    }
                }

                // Once we're in this branch no code below will be executed
                while state.first_error.is_some() {
                    match state.in_progress.next().await {
                        Some((num, result)) => match result {
                            Ok(TurnedPage {
                                items,
                                next_request,
                            }) => {
                                if next_request.is_none() {
                                    state.last_page = Some(num);
                                }

                                return Ok(Some((items, state)));
                            }
                            Err(new_err) => state.update_err(num, new_err),
                        },
                        // If at least one of `requests_ahead_count` futures returned an error and
                        // we haven't found the last page in other responses - return the first error
                        None => return Err(state.first_error.unwrap().1),
                    }
                }

                // Schedule
                if state.in_progress.is_empty() {
                    // Initial schedule of the first futures chunk
                    match state.numbered_requests.next_chunk() {
                        // If chunk is some then there is at least 1 request inside
                        Some(chunk) => {
                            for req in chunk {
                                let local_page_turner = state.page_turner.clone();
                                state.in_progress.push(Box::pin(async move {
                                    (req.0, local_page_turner.turn_page(req.1).await)
                                }));
                            }
                        }
                        None => {
                            return Ok(None);
                        }
                    }
                } else {
                    // At this point one of the first requests succeeded. Lets push the next one from the next_chunk to proceed in
                    // a sliding window maner.
                    if let Some(req) = state.numbered_requests.next_item() {
                        let local_page_turner = state.page_turner.clone();
                        state.in_progress.push(Box::pin(async move {
                            (req.0, local_page_turner.turn_page(req.1).await)
                        }))
                    }
                }

                match state.in_progress.next().await {
                    Some((num, result)) => match result {
                        Ok(TurnedPage {
                            items,
                            next_request,
                        }) => {
                            if next_request.is_none() {
                                state.last_page = Some(num);
                            }

                            return Ok(Some((items, state)));
                        }
                        // Don't return an error immediately, continue the loop to find the one for the
                        // first failed page instead, or to discard an error if it occured past the last existing page
                        Err(new_err) => state.update_err(num, new_err),
                    },
                    None => {
                        unreachable!(
                                "BUG(page-turner): We ensured that the unordered futures queue is not empty right above")
                    }
                }
            }
        }
    };
}

pub(crate) use pages_ahead_unordered_state_def;
pub(crate) use request_pages_ahead_unordered_decl;
