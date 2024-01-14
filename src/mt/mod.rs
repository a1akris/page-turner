//! A page turner suitable for multithreaded executors. This is what you need in most cases. See
//! [`dynamic`] if you also need `dyn PageTurner` objects for some reason.

use crate::internal::*;
use futures::stream::{self, FuturesOrdered, FuturesUnordered, Stream, StreamExt, TryStreamExt};
use std::{future::Future, pin::Pin};

pub use crate::{Limit, RequestAhead, TurnedPage};
#[doc = include_str!("../doc/prelude")]
pub mod prelude {
    pub use super::{Limit, PageTurner, PagesStream, RequestAhead, TurnedPage, TurnedPageResult};
}

#[doc = include_str!("../doc/PageItems")]
pub type PageItems<P, R> = <P as PageTurner<R>>::PageItems;
#[doc = include_str!("../doc/PageError")]
pub type PageError<P, R> = <P as PageTurner<R>>::PageError;
#[doc = include_str!("../doc/TurnedPageResult")]
pub type TurnedPageResult<P, R> = Result<TurnedPage<PageItems<P, R>, R>, PageError<P, R>>;
#[doc = include_str!("../doc/PageTurnerFuture")]
pub type PageTurnerFuture<'a, P, R> =
    Pin<Box<dyn 'a + Send + Future<Output = TurnedPageResult<P, R>>>>;

type NumberedRequestFuture<'a, P, R> =
    Pin<Box<dyn 'a + Send + Future<Output = (usize, TurnedPageResult<P, R>)>>>;

/// A page turner suitable for use in multithreaded contexts
///
#[doc = include_str!("../doc/PageTurner")]
pub trait PageTurner<R>: Sized + Send + Sync
where
    R: Send,
{
    type PageItems: Send;
    type PageError: Send;

    #[doc = include_str!("../doc/PageTurner__turn_page")]
    fn turn_page(&self, request: R) -> impl Send + Future<Output = TurnedPageResult<Self, R>>;

    #[doc = include_str!("../doc/PageTurner__pages")]
    fn pages<'s>(&'s self, request: R) -> impl PagesStream<'s, Self::PageItems, Self::PageError>
    where
        R: 's,
    {
        stream::try_unfold(PagesState::new(self, request), request_next_page)
    }

    #[doc = include_str!("../doc/PageTurner__into_pages")]
    fn into_pages<'s>(self, request: R) -> impl PagesStream<'s, Self::PageItems, Self::PageError>
    where
        R: 's,
        Self: 's,
    {
        stream::try_unfold(PagesState::new(self, request), request_next_page)
    }

    #[doc = include_str!("../doc/PageTurner__pages_ahead")]
    fn pages_ahead<'s>(
        &'s self,
        requests_ahead_count: usize,
        limit: Limit,
        request: R,
    ) -> impl PagesStream<'s, Self::PageItems, Self::PageError>
    where
        R: 's + RequestAhead,
    {
        stream::try_unfold(
            Box::new(PagesAheadState::new(
                self,
                request,
                requests_ahead_count,
                limit,
            )),
            request_pages_ahead,
        )
    }

    #[doc = include_str!("../doc/PageTurner__into_pages_ahead")]
    fn into_pages_ahead<'s>(
        self,
        requests_ahead_count: usize,
        limit: Limit,
        request: R,
    ) -> impl PagesStream<'s, Self::PageItems, Self::PageError>
    where
        R: 's + RequestAhead,
        Self: 's + Clone,
    {
        stream::try_unfold(
            Box::new(PagesAheadState::new(
                self,
                request,
                requests_ahead_count,
                limit,
            )),
            request_pages_ahead,
        )
    }

    #[doc = include_str!("../doc/PageTurner__pages_ahead_unordered")]
    fn pages_ahead_unordered<'s>(
        &'s self,
        requests_ahead_count: usize,
        limit: Limit,
        request: R,
    ) -> impl PagesStream<'s, Self::PageItems, Self::PageError>
    where
        R: 's + RequestAhead,
    {
        stream::try_unfold(
            Box::new(PagesAheadUnorderedState::new(
                self,
                request,
                requests_ahead_count,
                limit,
            )),
            request_pages_ahead_unordered,
        )
    }

    #[doc = include_str!("../doc/PageTurner__into_pages_ahead_unordered")]
    fn into_pages_ahead_unordered<'s>(
        self,
        requests_ahead_count: usize,
        limit: Limit,
        request: R,
    ) -> impl PagesStream<'s, Self::PageItems, Self::PageError>
    where
        Self: 's + Clone,
        R: 's + RequestAhead,
    {
        stream::try_unfold(
            Box::new(PagesAheadUnorderedState::new(
                self,
                request,
                requests_ahead_count,
                limit,
            )),
            request_pages_ahead_unordered,
        )
    }
}

impl<D, P, R> PageTurner<R> for D
where
    D: Send + Sync + std::ops::Deref<Target = P>,
    P: PageTurner<R>,
    R: Send,
{
    type PageItems = PageItems<P, R>;
    type PageError = PageError<P, R>;

    async fn turn_page(&self, request: R) -> TurnedPageResult<Self, R> {
        self.deref().turn_page(request).await
    }
}

#[doc = include_str!("../doc/PagesStream")]
pub trait PagesStream<'a, T, E>: Send + Stream<Item = Result<T, E>>
where
    T: Send,
    E: Send,
{
    #[doc = include_str!("../doc/PagesStream__items")]
    fn items(self) -> impl 'a + Send + Stream<Item = Result<<T as IntoIterator>::Item, E>>
    where
        Self: 'a,
        T: IntoIterator,
        <T as IntoIterator>::Item: Send,
        <T as IntoIterator>::IntoIter: Send;
}

impl<'a, S, T, E> PagesStream<'a, T, E> for S
where
    T: Send,
    E: Send,
    S: Send + Stream<Item = Result<T, E>>,
{
    fn items(self) -> impl 'a + Send + Stream<Item = Result<<T as IntoIterator>::Item, E>>
    where
        Self: 'a,
        T: IntoIterator,
        <T as IntoIterator>::Item: Send,
        <T as IntoIterator>::IntoIter: Send,
    {
        self.map_ok(|items| stream::iter(items.into_iter().map(Ok)))
            .try_flatten()
    }
}

pages_ahead_state_def!(R: Send);
pages_ahead_unordered_state_def!(R: Send);

request_next_page_decl!(R: Send);
request_pages_ahead_decl!(R: Send);
request_pages_ahead_unordered_decl!(R: Send);

#[cfg(feature = "dynamic")]
#[cfg_attr(docsrs, doc(cfg(feature = "dynamic")))]
pub mod dynamic {
    //! A page turner that can be used as a `dyn` object and which yields concrete boxed types

    use crate::internal::*;
    use async_trait::async_trait;
    use futures::stream::{
        self, BoxStream, FuturesOrdered, FuturesUnordered, Stream, StreamExt, TryStreamExt,
    };
    use std::{future::Future, pin::Pin};

    pub use super::PagesStream;
    pub use crate::{Limit, RequestAhead, TurnedPage};
    #[doc = include_str!("../doc/prelude")]
    pub mod prelude {
        pub use super::{
            BoxedPagesStream, Limit, PageTurner, PagesStream, RequestAhead, TurnedPage,
            TurnedPageResult,
        };
    }

    #[doc = include_str!("../doc/PageItems")]
    pub type PageItems<P, R> = <P as PageTurner<R>>::PageItems;
    #[doc = include_str!("../doc/PageError")]
    pub type PageError<P, R> = <P as PageTurner<R>>::PageError;
    #[doc = include_str!("../doc/TurnedPageResult")]
    pub type TurnedPageResult<P, R> = Result<TurnedPage<PageItems<P, R>, R>, PageError<P, R>>;
    #[doc = include_str!("../doc/PageTurnerFuture")]
    pub type PageTurnerFuture<'a, P, R> =
        Pin<Box<dyn 'a + Send + Future<Output = TurnedPageResult<P, R>>>>;

    type NumberedRequestFuture<'a, P, R> =
        Pin<Box<dyn 'a + Send + Future<Output = (usize, TurnedPageResult<P, R>)>>>;

    /// A page turner which yields dynamic objects. All methods are object safe and can be used
    /// with dynamic dispatch. Requires `#[async_trait]` to be implemented
    ///
    #[doc = include_str!("../doc/PageTurner")]
    #[async_trait]
    pub trait PageTurner<R>: Send + Sync
    where
        R: 'static + Send,
    {
        type PageItems: 'static + Send;
        type PageError: 'static + Send;

        #[doc = include_str!("../doc/PageTurner__turn_page")]
        async fn turn_page(&self, request: R) -> TurnedPageResult<Self, R>;

        #[doc = include_str!("../doc/PageTurner__pages")]
        fn pages(&self, request: R) -> BoxedPagesStream<'_, Self::PageItems, Self::PageError> {
            BoxedPagesStream(
                stream::try_unfold(PagesState::new(self, request), request_next_page).boxed(),
            )
        }

        #[doc = include_str!("../doc/PageTurner__into_pages")]
        fn into_pages<'s>(
            self,
            request: R,
        ) -> BoxedPagesStream<'s, Self::PageItems, Self::PageError>
        where
            Self: 's + Sized,
        {
            BoxedPagesStream(
                stream::try_unfold(PagesState::new(self, request), request_next_page).boxed(),
            )
        }

        #[doc = include_str!("../doc/PageTurner__pages_ahead")]
        fn pages_ahead<'s>(
            &'s self,
            requests_ahead_count: usize,
            limit: Limit,
            request: R,
        ) -> BoxedPagesStream<'s, Self::PageItems, Self::PageError>
        where
            R: 's + RequestAhead,
        {
            BoxedPagesStream(
                stream::try_unfold(
                    Box::new(PagesAheadState::new(
                        self,
                        request,
                        requests_ahead_count,
                        limit,
                    )),
                    request_pages_ahead,
                )
                .boxed(),
            )
        }

        #[doc = include_str!("../doc/PageTurner__into_pages_ahead")]
        fn into_pages_ahead<'s>(
            self,
            requests_ahead_count: usize,
            limit: Limit,
            request: R,
        ) -> BoxedPagesStream<'s, Self::PageItems, Self::PageError>
        where
            Self: 's + Clone + Sized,
            R: RequestAhead,
        {
            BoxedPagesStream(
                stream::try_unfold(
                    Box::new(PagesAheadState::new(
                        self,
                        request,
                        requests_ahead_count,
                        limit,
                    )),
                    request_pages_ahead,
                )
                .boxed(),
            )
        }

        #[doc = include_str!("../doc/PageTurner__pages_ahead_unordered")]
        fn pages_ahead_unordered<'s>(
            &'s self,
            requests_ahead_count: usize,
            limit: Limit,
            request: R,
        ) -> BoxedPagesStream<'s, Self::PageItems, Self::PageError>
        where
            R: 's + RequestAhead,
        {
            BoxedPagesStream(
                stream::try_unfold(
                    Box::new(PagesAheadUnorderedState::new(
                        self,
                        request,
                        requests_ahead_count,
                        limit,
                    )),
                    request_pages_ahead_unordered,
                )
                .boxed(),
            )
        }

        #[doc = include_str!("../doc/PageTurner__into_pages_ahead_unordered")]
        fn into_pages_ahead_unordered<'s>(
            self,
            requests_ahead_count: usize,
            limit: Limit,
            request: R,
        ) -> BoxedPagesStream<'s, Self::PageItems, Self::PageError>
        where
            Self: 's + Clone + Sized,
            R: RequestAhead,
        {
            BoxedPagesStream(
                stream::try_unfold(
                    Box::new(PagesAheadUnorderedState::new(
                        self,
                        request,
                        requests_ahead_count,
                        limit,
                    )),
                    request_pages_ahead_unordered,
                )
                .boxed(),
            )
        }
    }

    #[async_trait]
    impl<D, P, R> PageTurner<R> for D
    where
        D: Send + Sync + std::ops::Deref<Target = P>,
        P: ?Sized + PageTurner<R>,
        R: 'static + Send,
    {
        type PageItems = PageItems<P, R>;
        type PageError = PageError<P, R>;

        async fn turn_page(&self, request: R) -> TurnedPageResult<Self, R> {
            self.deref().turn_page(request).await
        }
    }

    /// A boxed version of a pages stream to satisfy object safety requirements
    /// of [`PageTurner`]
    pub struct BoxedPagesStream<'a, T, E>(BoxStream<'a, Result<T, E>>);

    impl<'a, T, E> Stream for BoxedPagesStream<'a, T, E>
    where
        T: 'static + Send,
        E: 'static + Send,
    {
        type Item = Result<T, E>;

        fn poll_next(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Self::Item>> {
            self.0.poll_next_unpin(cx)
        }
    }

    pages_ahead_state_def!(R: 'static + Send);
    pages_ahead_unordered_state_def!(R: 'static + Send);

    request_next_page_decl!(R: 'static + Send);
    request_pages_ahead_decl!(R: 'static + Send);
    request_pages_ahead_unordered_decl!(R: 'static + Send);
}

#[cfg(test)]
mod tests;
