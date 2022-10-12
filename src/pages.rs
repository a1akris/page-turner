//! Helpers to implement `pages` functionality
use super::{PageTurner, TurnedPage};

pub(crate) async fn request_next_page<P, R>(
    mut state: StreamState<P, R>,
) -> Result<
    Option<(Vec<<P as PageTurner<R>>::PageItem>, StreamState<P, R>)>,
    <P as PageTurner<R>>::PageError,
>
where
    P: PageTurner<R>,
    R: 'static + Send,
{
    let request = match state.next_request {
        Some(request) => request,
        None => return Ok(None),
    };

    let TurnedPage {
        items,
        next_request,
    } = state.page_turner.turn_page(request).await?;

    state.next_request = next_request;
    Ok(Some((items, state)))
}

pub(crate) struct StreamState<P, R> {
    page_turner: P,
    next_request: Option<R>,
}

impl<P, R> StreamState<P, R> {
    pub(crate) fn new(page_turner: P, request: R) -> Self {
        Self {
            page_turner,
            next_request: Some(request),
        }
    }
}
