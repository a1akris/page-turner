//! A page turner suitable for singlethreaded executors. See [`mutable`] for a version that
//! allows to use &mut self in methods

use crate::internal::*;
use futures::{
    stream::{self, FuturesOrdered, FuturesUnordered},
    Stream, StreamExt, TryStreamExt,
};
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
pub type PageTurnerFuture<'a, P, R> = Pin<Box<dyn 'a + Future<Output = TurnedPageResult<P, R>>>>;

type NumberedRequestFuture<'a, P, R> =
    Pin<Box<dyn 'a + Future<Output = (usize, TurnedPageResult<P, R>)>>>;

/// This is one of the less constrained page turners which produces `?Send`(may be Send) futures
/// and streams that should run on single threaded executors. Occasionally, it might also work with
/// multithreaded executors but it's not recommended to abuse that if you write a maintainable
/// code.
///
#[doc = include_str!("../doc/PageTurner")]
pub trait PageTurner<R>: Sized {
    type PageItems;
    type PageError;

    #[doc = include_str!("../doc/PageTurner__turn_page")]
    fn turn_page(&self, request: R) -> impl Future<Output = TurnedPageResult<Self, R>>;

    #[doc = include_str!("../doc/PageTurner__pages")]
    fn pages<'s>(&self, request: R) -> impl PagesStream<'s, Self::PageItems, Self::PageError>
    where
        R: 's,
    {
        stream::try_unfold(PagesState::new(self, request), request_next_page)
    }

    #[doc = include_str!("../doc/PageTurner__into_pages")]
    fn into_pages<'s>(self, request: R) -> impl PagesStream<'s, Self::PageItems, Self::PageError>
    where
        Self: 's,
        R: 's,
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
        Self: 's + Clone,
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
    D: std::ops::Deref<Target = P>,
    P: PageTurner<R>,
{
    type PageItems = PageItems<P, R>;
    type PageError = PageError<P, R>;

    async fn turn_page(&self, request: R) -> TurnedPageResult<Self, R> {
        self.deref().turn_page(request).await
    }
}

#[doc = include_str!("../doc/PagesStream")]
pub trait PagesStream<'a, T, E>: Stream<Item = Result<T, E>> {
    #[doc = include_str!("../doc/PagesStream__items")]
    fn items(self) -> impl 'a + Stream<Item = Result<<T as IntoIterator>::Item, E>>
    where
        Self: 'a,
        T: IntoIterator;
}

impl<'a, S, T, E> PagesStream<'a, T, E> for S
where
    S: Stream<Item = Result<T, E>>,
{
    fn items(self) -> impl 'a + Stream<Item = Result<<T as IntoIterator>::Item, E>>
    where
        Self: 'a,
        T: IntoIterator,
    {
        self.map_ok(|items| stream::iter(items.into_iter().map(Ok)))
            .try_flatten()
    }
}

pages_ahead_state_def!();
pages_ahead_unordered_state_def!();

request_next_page_decl!();
request_pages_ahead_decl!();
request_pages_ahead_unordered_decl!();

#[cfg(feature = "mutable")]
#[cfg_attr(docsrs, doc(cfg(feature = "mutable")))]
pub mod mutable {
    //! Provides a page turner which takes `&mut self` instead of `&self` if you don't want to bother
    //! with interior mutability in single threaded contexts.

    use crate::internal::*;
    use futures::stream;
    use std::{future::Future, pin::Pin};

    pub use super::PagesStream;
    pub use crate::{Limit, RequestAhead, TurnedPage};
    #[doc = include_str!("../doc/prelude")]
    pub mod prelude {
        pub use super::{
            Limit, PageTurner, PagesStream, RequestAhead, TurnedPage, TurnedPageResult,
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
        Pin<Box<dyn 'a + Future<Output = TurnedPageResult<P, R>>>>;

    /// The least constrained page turner that allows an implementor to mutate during request
    /// execution and, therefore, doesn't provide the `pages_ahead` family of methods as it's
    /// invalid to hold multiple `&mut self` references concurrently. For uses in single threaded
    /// contexts when you don't want to bother with interior mutability of the implementor.
    ///
    #[doc = include_str!("../doc/PageTurner")]
    pub trait PageTurner<R>: Sized {
        type PageItems;
        type PageError;

        #[doc = include_str!("../doc/PageTurner__turn_page")]
        fn turn_page(&mut self, request: R) -> impl Future<Output = TurnedPageResult<Self, R>>;

        #[doc = include_str!("../doc/PageTurner__pages")]
        fn pages<'s>(
            &'s mut self,
            request: R,
        ) -> impl PagesStream<'s, PageItems<Self, R>, PageError<Self, R>>
        where
            R: 's,
        {
            stream::try_unfold(PagesState::new(self, request), request_next_page)
        }

        #[doc = include_str!("../doc/PageTurner__into_pages")]
        fn into_pages<'s>(
            self,
            request: R,
        ) -> impl PagesStream<'s, Self::PageItems, Self::PageError>
        where
            Self: 's,
            R: 's,
        {
            stream::try_unfold(PagesState::new(self, request), request_next_page)
        }
    }

    impl<P, R> PageTurner<R> for &mut P
    where
        P: PageTurner<R>,
    {
        type PageItems = PageItems<P, R>;
        type PageError = PageError<P, R>;

        fn turn_page(&mut self, request: R) -> impl Future<Output = TurnedPageResult<Self, R>> {
            P::turn_page(self, request)
        }
    }

    impl<P, R> PageTurner<R> for Box<P>
    where
        P: PageTurner<R>,
    {
        type PageItems = PageItems<P, R>;
        type PageError = PageError<P, R>;

        fn turn_page(&mut self, request: R) -> impl Future<Output = TurnedPageResult<Self, R>> {
            P::turn_page(self.as_mut(), request)
        }
    }

    request_next_page_decl!();
}

#[cfg(test)]
mod tests;
