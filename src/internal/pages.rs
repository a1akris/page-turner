pub struct PagesState<P, R> {
    pub page_turner: P,
    pub next_request: Option<R>,
}

impl<P, R> PagesState<P, R> {
    pub fn new(page_turner: P, request: R) -> Self {
        Self {
            page_turner,
            next_request: Some(request),
        }
    }
}

macro_rules! request_next_page_decl {
    ($($extra_bounds:tt)*) => {
        async fn request_next_page<P, R>(
            mut state: crate::internal::pages::PagesState<P, R>,
        ) -> Result<Option<(PageItems<P, R>, crate::internal::pages::PagesState<P, R>)>, PageError<P, R>>
        where
            P: PageTurner<R>,
            $($extra_bounds)*
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
    };
}

pub(crate) use request_next_page_decl;
