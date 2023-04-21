//! # page-turner
//!
//! ### A generic abstraction of paginated APIs
//!
//! Imagine, you need to use the following API to find the most upvoted comment
//! under a blog post.
//!
//! ```rust
//! struct GetCommentsRequest {
//!     blog_post_id: BlogPostId,
//!     page_number: u32,
//! }
//!
//! struct GetCommentsResponse {
//!     comments: Vec<Comment>,
//!     more_comments_available: bool,
//! }
//!
//! # struct Comment {
//! #     upvotes: u32,
//! #     text: String,
//! # }
//! #
//! # type BlogPostId = u32;
//! ```
//!
//! In order to do that you will need to write a hairy loop that checks the
//! `more_comments_available` flag, increments `page_number`, and updates a
//! variable that stores the resulting value. This crate helps to abstract away any
//! sort of pagination and allows you to work with such APIs uniformly with the
//! help of async streams. All you need to do is to implement the [`PageTurner`]
//! trait instead for the client that sends `GetCommentsRequest`.
//!
//! In [`PageTurner`] you specify what items you query and what errors may occur,
//! then you implement the `turn_page` method where you describe how to query a
//! single page and how to prepare a request for the next page.
//!
//! ```rust
//! use async_trait::async_trait;
//! use page_turner::prelude::*;
//!
//! #[async_trait]
//! impl PageTurner<GetCommentsRequest> for OurBlogClient {
//!     type PageItem = Comment;
//!     type PageError = OurClientError;
//!
//!     async fn turn_page(&self, mut request: GetCommentsRequest) -> PageTurnerOutput<Self, GetCommentsRequest> {
//!         let response = self.get_comments(request.clone()).await?;
//!
//!         if response.more_comments_available {
//!             request.page_number += 1;
//!             Ok(TurnedPage::next(response.comments, request))
//!         } else {
//!             Ok(TurnedPage::last(response.comments))
//!         }
//!     }
//! }
//!
//! # struct OurBlogClient {}
//!
//! # impl OurBlogClient {
//! #    async fn get_comments(&self, req: GetCommentsRequest) -> Result<GetCommentsResponse, OurClientError> {
//! #        todo!()
//! #    }
//! # }
//!
//! # struct Comment {}
//! # struct OurClientError {}
//! # type BlogPostId = u32;
//!
//! # #[derive(Clone)]
//! # struct GetCommentsRequest {
//! #     blog_post_id: BlogPostId,
//! #     page_number: u32,
//! # }
//! #
//! # struct GetCommentsResponse {
//! #     comments: Vec<Comment>,
//! #     more_comments_available: bool,
//! # }
//! ```
//!
//! [`PageTurner`] then provides default implementations for [`PageTurner::pages`]
//! and [`PageTurner::into_pages`] methods that you can use to get a stream of
//! pages and, optionally, to turn it into a stream of items if you need. Now we
//! can use our client to find the most upvoted comment like that:
//!
//! ```rust
//! # type BlogPostId = u32;
//! #
//! # #[derive(Clone)]
//! # struct GetCommentsRequest {
//! #     blog_post_id: BlogPostId,
//! #     page_number: u32,
//! # }
//! #
//! # struct GetCommentsResponse {
//! #     comments: Vec<Comment>,
//! #     more_comments_available: bool,
//! # }
//! #
//! # struct Comment {
//! #     upvotes: u32,
//! #     text: String,
//! # }
//! #
//! # struct OurBlogClient {}
//! #
//! # impl OurBlogClient {
//! #     fn new() -> Self {
//! #         Self {}
//! #     }
//! #
//! #     async fn get_comments(&self, req: GetCommentsRequest) -> Result<GetCommentsResponse, OurClientError> {
//! #         Ok(GetCommentsResponse {
//! #             comments: vec![
//! #                 Comment {
//! #                     text: "First".to_owned(),
//! #                     upvotes: 0,
//! #                 },
//! #                 Comment {
//! #                     text: "Second".to_owned(),
//! #                     upvotes: 2,
//! #                 },
//! #                 Comment {
//! #                     text: "Yeet".to_owned(),
//! #                     upvotes: 5,
//! #                 }
//! #             ],
//! #             more_comments_available: false,
//! #         })
//! #     }
//! # }
//! #
//! # #[derive(Debug)]
//! # struct OurClientError {}
//! #
//! # use async_trait::async_trait;
//! # use page_turner::prelude::*;
//! #
//! # #[async_trait]
//! # impl PageTurner<GetCommentsRequest> for OurBlogClient {
//! #     type PageItem = Comment;
//! #     type PageError = OurClientError;
//! #
//! #     async fn turn_page(&self, mut request: GetCommentsRequest) -> PageTurnerOutput<Self, GetCommentsRequest> {
//! #         let response = self.get_comments(request.clone()).await?;
//! #
//! #         if response.more_comments_available {
//! #             request.page_number += 1;
//! #             Ok(TurnedPage::next(response.comments, request))
//! #         } else {
//! #             Ok(TurnedPage::last(response.comments))
//! #         }
//! #     }
//! # }
//! #
//! #
//! # use futures::TryStreamExt;
//! #
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() -> Result<(), OurClientError> {
//! # let blog_post_id = 1337;
//! let client = OurBlogClient::new();
//!
//! let most_upvoted_comment = client
//!     .pages(GetCommentsRequest { blog_post_id, page_number: 1 })
//!     .items()
//!     .try_fold(None::<Comment>, |most_upvoted, next_comment| async move {
//!         match most_upvoted {
//!             Some(comment) if next_comment.upvotes > comment.upvotes => Ok(Some(next_comment)),
//!             current @ Some(_) => Ok(current),
//!             None => Ok(Some(next_comment)),
//!         }
//!     })
//!     .await?
//!     .unwrap();
//!
//! assert_eq!(most_upvoted_comment.text, "Yeet");
//! assert_eq!(most_upvoted_comment.upvotes, 5);
//!
//! // Or we can process the whole pages if needed
//!
//! let mut comment_pages = client.pages(GetCommentsRequest { blog_post_id, page_number: 1 });
//!
//! while let Some(comment_page) = comment_pages.try_next().await? {
//!     detect_spam(comment_page);
//! }
//!
//! #   Ok(())
//! # }
//! #
//! # fn detect_spam(page: Vec<Comment>) -> bool {
//! #    false
//! # }
//! ```
//!
//! Notice, that with this API we don't actually need any info from the response to
//! construct the next valid request. We can take an advantage on such kind of
//! requests by implementing the [`RequestAhead`] trait for them. For requests that
//! implement [`RequestAhead`] [`PageTurner`] provides additional methods -
//! [`PageTurner::pages_ahead`] and [`PageTurner::pages_ahead_unordered`]. These
//! methods allow to query multiple pages concurrently.
//!
//! ```rust
//! # type BlogPostId = u32;
//! #
//! # #[derive(Clone)]
//! # struct GetCommentsRequest {
//! #     blog_post_id: BlogPostId,
//! #     page_number: u32,
//! # }
//! #
//! # struct GetCommentsResponse {
//! #     comments: Vec<Comment>,
//! #     more_comments_available: bool,
//! # }
//! #
//! # struct Comment {
//! #     upvotes: u32,
//! #     text: String,
//! # }
//! #
//! # struct OurBlogClient {}
//! #
//! # impl OurBlogClient {
//! #     fn new() -> Self {
//! #         Self {}
//! #     }
//! #
//! #     async fn get_comments(&self, req: GetCommentsRequest) -> Result<GetCommentsResponse, OurClientError> {
//! #         Ok(GetCommentsResponse {
//! #             comments: vec![
//! #                 Comment {
//! #                     text: "First".to_owned(),
//! #                     upvotes: 0,
//! #                 },
//! #                 Comment {
//! #                     text: "Second".to_owned(),
//! #                     upvotes: 2,
//! #                 },
//! #                 Comment {
//! #                     text: "Yeet".to_owned(),
//! #                     upvotes: 5,
//! #                 }
//! #             ],
//! #             more_comments_available: false,
//! #         })
//! #     }
//! # }
//! #
//! # #[derive(Debug)]
//! # struct OurClientError {}
//! #
//! # use async_trait::async_trait;
//! # use page_turner::prelude::*;
//! #
//! # #[async_trait]
//! # impl PageTurner<GetCommentsRequest> for OurBlogClient {
//! #     type PageItem = Comment;
//! #     type PageError = OurClientError;
//! #
//! #     async fn turn_page(&self, mut request: GetCommentsRequest) -> PageTurnerOutput<Self, GetCommentsRequest> {
//! #         let response = self.get_comments(request.clone()).await?;
//! #
//! #         if response.more_comments_available {
//! #             request.page_number += 1;
//! #             Ok(TurnedPage::next(response.comments, request))
//! #         } else {
//! #             Ok(TurnedPage::last(response.comments))
//! #         }
//! #     }
//! # }
//! #
//! #
//! # use futures::TryStreamExt;
//! #
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() -> Result<(), OurClientError> {
//! # let blog_post_id = 1337;
//! impl RequestAhead for GetCommentsRequest {
//!     fn next_request(&self) -> Self {
//!         Self {
//!             blog_post_id: self.blog_post_id,
//!             page_number: self.page_number + 1,
//!         }
//!     }
//! }
//!
//! let client = OurBlogClient::new();
//!
//! // Now instead of querying pages one by one we make up to 4 concurrent requests
//! // for multiple pages under the hood but besides using a different PageTurner
//! // method nothing changes in the user code.
//! let most_upvoted_comment = client
//!     .pages_ahead(4, Limit::None, GetCommentsRequest { blog_post_id, page_number: 1 })
//!     .items()
//!     .try_fold(None::<Comment>, |most_upvoted, next_comment| async move {
//!         match most_upvoted {
//!             Some(comment) if next_comment.upvotes > comment.upvotes => Ok(Some(next_comment)),
//!             current @ Some(_) => Ok(current),
//!             None => Ok(Some(next_comment)),
//!         }
//!     })
//!     .await?
//!     .unwrap();
//!
//! assert_eq!(most_upvoted_comment.text, "Yeet");
//! assert_eq!(most_upvoted_comment.upvotes, 5);
//!
//! // In the example above the order of pages being returned corresponds to the order
//! // of requests which means the stream is blocked until the first page is ready
//! // even if the second and the third pages are already received. For this use case
//! // we don't really care about the order of the comments so we can use
//! // pages_ahead_unordered to unblock the stream as soon as we receive a response to
//! // any of the concurrent requests.
//! let most_upvoted_comment = client
//!     .pages_ahead_unordered(4, Limit::None, GetCommentsRequest { blog_post_id, page_number: 1 })
//!     .items()
//!     .try_fold(None::<Comment>, |most_upvoted, next_comment| async move {
//!         match most_upvoted {
//!             Some(comment) if next_comment.upvotes > comment.upvotes => Ok(Some(next_comment)),
//!             current @ Some(_) => Ok(current),
//!             None => Ok(Some(next_comment)),
//!         }
//!     })
//!     .await?
//!     .unwrap();
//!
//! assert_eq!(most_upvoted_comment.text, "Yeet");
//! assert_eq!(most_upvoted_comment.upvotes, 5);
//! #   Ok(())
//! # }
//! ```

mod pages;
mod pages_ahead;
pub mod prelude;

use async_trait::async_trait;
use futures::{
    stream::{self, BoxStream},
    Stream, StreamExt, TryStreamExt,
};

/// The trait is supposed to be implemented on API clients. You need to specify the
/// [`PageTurner::PageItem`]s to return and [`PageTurner::PageError`]s that may occur. Then you
/// should implement the [`PageTurner::turn_page`] method to describe how to query a single page
/// and how to prepare a request for the next page. After that default [`PageTurner::pages`] and
/// [`PageTurner::into_pages`] methods become available to provide a stream based querying API.
#[async_trait]
pub trait PageTurner<R>: Send + Sync
where
    R: 'static + Send,
{
    type PageItem: Send;
    type PageError: Send;

    /// Note: You need to return Ok([`TurnedPage`]) or an error. [`PageTurnerOutput`] is just a type
    /// alias to save you from typing a somewhat complex return type.
    async fn turn_page(&self, request: R) -> PageTurnerOutput<Self, R>;

    /// Returns a stream that queries pages lineary. The stream borrows the client internally and
    /// this may dissappoint the borrow checker in certain situations so you can use
    /// [`PageTurner::into_pages`] if you need an owned stream.
    fn pages(&self, request: R) -> PagesStream<'_, Self::PageItem, Self::PageError> {
        stream::try_unfold(
            pages::StreamState::new(self, request),
            pages::request_next_page,
        )
        .boxed()
        .into()
    }

    /// Returns an owned [`PagesStream`]. In certain situations you can't use streams with borrows. For
    /// example, you can't use a stream with a borrow if the stream should outlive a client in APIs like
    /// this one:
    ///
    /// ```ignore
    /// fn get_stuff(params: Params) -> impl Stream<Item = Stuff> {
    ///     // This client is not needed anywhere else and is cheap to create.
    ///     let client = StuffClient::new();
    ///     client.pages(GetStuff::from_params(params))
    /// }
    /// ```
    ///
    /// The client gets dropped after the `.pages` call but the stream we're returning needs an
    /// internal reference to the client in order to perform the querying. This situation can be
    /// fixed by simply using this method instead:
    ///
    ///
    /// ```ignore
    /// fn get_stuff(params: Params) -> impl Stream<Item = Stuff> {
    ///     let client = StuffClient::new(params);
    ///     client.into_pages(GetStuff::from_params(params))
    /// }
    /// ```
    ///
    /// Now the client is consumed into a stream to be used internally. If you want to keep the
    /// client in a scope but it is not cheaply clonable or not clonable at all you can wrap it into
    /// [`std::sync::Arc`] like that:
    ///
    /// ```ignore
    /// fn get_stuff(params: Params) -> impl Stream<Item = Stuff> {
    ///     let client = Arc::new(StuffClient::new(params));
    ///     Arc::clone(&client).into_pages(GetStuff::from_params(params))
    /// }
    /// ```
    fn into_pages(self, request: R) -> OwnedPagesStream<Self::PageItem, Self::PageError>
    where
        Self: 'static + Sized,
    {
        stream::try_unfold(
            pages::StreamState::new(self, request),
            pages::request_next_page,
        )
        .boxed()
        .into()
    }

    /// Executes `requests_ahead_count` requests concurrently using a sliding window to query
    /// multiple pages at once. Returns pages in the requests order(first page corresponds to the
    /// first request and so on). Because of that the stream will be blocked until the first page
    /// becomes available even if the second page is already received. Use
    /// [`PageTurner::pages_ahead_unordered`] if the order is not important to unblock the stream
    /// as soon as any page becomes available.
    ///
    /// It's quite likely that some redundant queries past the last existing page will be executed
    /// just to return the "not found" error. For example we choose `requests_ahead_count` to be 4
    /// but there are 6 pages on the backend:
    ///
    /// ```text
    /// Pages:    [1,2,3,4,5,6]
    /// Requests: [[1,2,3,4], [5,6,7*,8*]]
    /// ```
    ///
    /// It's possible that requests `[7,8]` will start executing before the request 6, but when we
    /// receive the 6th page or an error, futures to query `[7,8]` are discarded in any case. To
    /// prevent them from being scheduled at all use [`Limit::Pages`] if you know how many pages
    /// you need to query in advance.
    ///
    /// The implementation uses [`TurnedPage::next_request`] just to identify the stream end but it
    /// only checks the availability of the [`TurnedPage::next_request`] without using the returned
    /// request itself, thus it's possible to get different results in the [`PageTurner::pages`]
    /// stream and in the [`PageTurner::pages_ahead`] stream if you construct next requests inside
    /// [`RequestAhead::next_request`] and inside [`PageTurner::turn_page`] differently. This is
    /// considered to be a bug, by the contract streams must be identical. You can use
    /// [`RequestAhead::next_request`] in the [`PageTurner::turn_page`] for [`RequestAhead`]
    /// requests to prevent this bug but don't forget to return [`TurnedPage::last`] for the last
    /// existing page.
    fn pages_ahead(
        &self,
        requests_ahead_count: usize,
        limit: Limit,
        request: R,
    ) -> PagesStream<'_, Self::PageItem, Self::PageError>
    where
        R: RequestAhead,
    {
        stream::try_unfold(
            pages_ahead::ordered::StreamState::new(self, request, requests_ahead_count, limit),
            pages_ahead::ordered::request_next_page,
        )
        .boxed()
        .into()
    }

    /// This method exists for the same reason described in [`PageTurner::into_pages`].
    fn into_pages_ahead(
        self,
        requests_ahead_count: usize,
        limit: Limit,
        request: R,
    ) -> OwnedPagesStream<Self::PageItem, Self::PageError>
    where
        Self: 'static + Sized,
        R: RequestAhead,
    {
        stream::try_unfold(
            pages_ahead::ordered::StreamStateOwned::new(self, request, requests_ahead_count, limit),
            pages_ahead::ordered::request_next_page_owned,
        )
        .boxed()
        .into()
    }

    /// Behaves mostly like [`PageTurner::pages_ahead`] with the difference that pages are returned
    /// as soon as they become available in an arbitrary order. This has an important consequence
    /// though. Unlike all other methods provided by [`PageTurner`] this method postpones errors
    /// till all requests in a chunk are processed. This is required to handle errors reading past
    /// the end of available data correctly.
    ///
    /// For example if there are 6 pages and we choose to execute 4 requests concurrently the following
    /// situation can occur.
    ///
    /// ```text
    /// Pages:    [1,2,3,4,5,6]
    /// Requests: [[1,2,3,4], [5,6,7,8]]
    /// Responses order: [[3, 1, 2, 4], [6!, 8*, 7*, 5]]
    /// ```
    ///
    /// In the last chunk we receive the last 6th page before all other pages, then 2 "Not found"
    /// errors, and the valid 5th page comes last. That's why when we receive the last page or an
    /// error we must let other futures in the chunk to complete. The current behavior is to
    /// remember the last error till all futures in the chunk are ready, then if the end of the
    /// stream was detected during the chunk processing we discard the error but otherwise we
    /// return it.
    ///
    /// That's what the resulting stream yields for the example above.
    ///
    /// ```text
    /// Pages in stream: [3, 1, 2, 4, 6, 5]
    /// ```
    ///
    /// **TL;DR**: In practice this means you should choose small numbers for the
    /// `requests_ahead_count` unless you provide [`Limit::Pages`] to get rid of the read past the
    /// end errors, or if you expect that some errors can occur quite frequently. If unsure just
    /// use [`PageTurner::pages_ahead`] instead.
    fn pages_ahead_unordered(
        &self,
        requests_ahead_count: usize,
        limit: Limit,
        request: R,
    ) -> PagesStream<'_, Self::PageItem, Self::PageError>
    where
        R: RequestAhead,
    {
        stream::try_unfold(
            pages_ahead::unordered::StreamState::new(self, request, requests_ahead_count, limit),
            pages_ahead::unordered::request_next_page,
        )
        .boxed()
        .into()
    }

    /// This method exists for the same reason described in [`PageTurner::into_pages`].
    fn into_pages_ahead_unordered(
        self,
        requests_ahead_count: usize,
        limit: Limit,
        request: R,
    ) -> OwnedPagesStream<Self::PageItem, Self::PageError>
    where
        Self: 'static + Sized,
        R: RequestAhead,
    {
        stream::try_unfold(
            pages_ahead::unordered::StreamStateOwned::new(
                self,
                request,
                requests_ahead_count,
                limit,
            ),
            pages_ahead::unordered::request_next_page_owned,
        )
        .boxed()
        .into()
    }
}

#[async_trait]
impl<D, P, R> PageTurner<R> for D
where
    D: Send + Sync + std::ops::Deref<Target = P>,
    P: ?Sized + PageTurner<R>,
    R: 'static + Send,
{
    type PageItem = P::PageItem;
    type PageError = P::PageError;

    async fn turn_page(&self, request: R) -> PageTurnerOutput<Self, R> {
        self.deref().turn_page(request).await
    }
}

/// A handy shortcut to deduce [`PageTurner::turn_page`] return type.
pub type PageTurnerOutput<P, R> =
    Result<TurnedPage<<P as PageTurner<R>>::PageItem, R>, <P as PageTurner<R>>::PageError>;

/// A struct that combines items queried for the current page and an optional request to query the
/// next page. If `next_request` is `None` [`PageTurner`] stops querying pages.
///
/// [`TurnedPage::next`] and [`TurnedPage::last`] constructors can be used for convenience.
pub struct TurnedPage<T, R> {
    pub items: Vec<T>,
    pub next_request: Option<R>,
}

impl<T, R> TurnedPage<T, R> {
    pub fn new(items: Vec<T>, next_request: Option<R>) -> Self {
        Self {
            items,
            next_request,
        }
    }

    pub fn next(items: Vec<T>, next_request: R) -> Self {
        Self {
            items,
            next_request: Some(next_request),
        }
    }

    pub fn last(items: Vec<T>) -> Self {
        Self {
            items,
            next_request: None,
        }
    }
}

/// A stream of `Result<Vec<PageTurner::PageItem>, PageTurner::PageError>` produced by the
/// [`PageTurner`] trait
pub struct PagesStream<'a, T, E>(BoxStream<'a, Result<Vec<T>, E>>);

/// An owned [`PagesStream`] for the cases when the borrow checker is not happy with your usage of
/// streams that borrow.
pub type OwnedPagesStream<T, E> = PagesStream<'static, T, E>;

impl<'a, T, E> PagesStream<'a, T, E>
where
    T: 'a + Send,
    E: 'a + Send,
{
    /// Gets items of the page. This effectively performs pages flattening turning
    /// `Result<Vec<T>, E>` into `Result<T, E>`. The `flatten()` name is not used
    /// to not interfere with [`futures::StreamExt::flatten`] that you may want to call
    /// on other streams.
    pub fn items(self) -> impl 'a + Send + Stream<Item = Result<T, E>> {
        self.0
            .map_ok(|items| stream::iter(items.into_iter().map(Ok)))
            .try_flatten()
    }
}

impl<'a, T, E> Stream for PagesStream<'a, T, E> {
    type Item = Result<Vec<T>, E>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0.poll_next_unpin(cx)
    }
}

impl<'a, T, E> From<BoxStream<'a, Result<Vec<T>, E>>> for PagesStream<'a, T, E> {
    fn from(stream: BoxStream<'a, Result<Vec<T>, E>>) -> Self {
        Self(stream)
    }
}

/// If you use [`PageTurner::pages_ahead`] or [`PageTurner::pages_ahead_unordered`] and you know
/// in advance how many pages you need to query, specify [`Limit::Pages`] to prevent redundant
/// querying past the last existing page from being executed.
#[allow(dead_code)]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    #[default]
    None,
    Pages(usize),
}

/// If a request for the next page doesn't require any data from the response and can be made out
/// of the request for the current page implement this trait to enable [`PageTurner::pages_ahead`]
/// and [`PageTurner::pages_ahead_unordered`] methods for concurrent querying.
pub trait RequestAhead {
    fn next_request(&self) -> Self;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[tokio::test]
    async fn pages() {
        let client = NumbersClient::new(30, 30);
        let expected: Vec<usize> = (1..=30).collect();

        let TurnedPage { items: output, .. } =
            client.turn_page(GetNumbersQuery::new()).await.unwrap();
        assert_eq!(output, expected, "Without stream");

        let pages: Vec<_> = client
            .pages(GetNumbersQuery::new())
            .try_collect()
            .await
            .unwrap();

        assert_eq!(pages.len(), 1, "There should be only one page");
        assert_eq!(pages[0].len(), 30, "The page must contain 30 items");

        // Testing that `into_pages` stream will be available if we wrap a non-clonable client in
        // Arc
        let output: Vec<_> = Arc::new(client)
            .into_pages(GetNumbersQuery::new())
            .items()
            .try_collect()
            .await
            .unwrap();

        assert_eq!(
            output, expected,
            "After paginated query with page_size = 30"
        );

        let client = NumbersClient::new(30, 10);

        let pages: Vec<_> = client
            .pages(GetNumbersQuery::new())
            .try_collect()
            .await
            .unwrap();

        assert_eq!(pages.len(), 3, "There should be 3 pages");

        for page in pages {
            assert_eq!(page.len(), 10, "Each page must contain 10 elements")
        }

        let output: Vec<_> = client
            .pages(GetNumbersQuery::new())
            .items()
            .try_collect()
            .await
            .unwrap();

        assert_eq!(
            output, expected,
            "After paginated query with page_size = 10"
        );

        let client = NumbersClient::new(30, 19);

        let pages: Vec<_> = client
            .pages(GetNumbersQuery::new())
            .try_collect()
            .await
            .unwrap();

        assert_eq!(pages.len(), 2, "There should be 2 pages");

        assert_eq!(pages[0].len(), 19, "The first page must contain 19 items");
        assert_eq!(pages[1].len(), 11, "The second page must contain 11 items");

        let output: Vec<_> = client
            .pages(GetNumbersQuery::new())
            .items()
            .try_collect()
            .await
            .unwrap();

        assert_eq!(
            output, expected,
            "After paginated query with page_size = 19"
        );

        let mut blog = BlogClient::new(41);
        blog.set_error(0);

        let mut stream = blog.pages(GetContentRequest { page: 0 }).items();

        let item = stream.try_next().await;
        assert_eq!(item, Err("Custom error".to_owned()));

        let item = stream.try_next().await;
        assert_eq!(item, Ok(None), "pages stream must end after an error");
    }

    #[tokio::test]
    async fn pages_ahead() {
        std::env::set_var("RUST_LOG", "trace");
        env_logger::init();

        let mut blog = BlogClient::new(33);

        let results: Vec<_> = blog
            .pages_ahead(5, Limit::None, GetContentRequest { page: 0 })
            .items()
            .try_collect()
            .await
            .unwrap();

        assert_eq!(results.len(), 33);
        assert_eq!(*results.last().unwrap(), BlogRecord(32));

        let results: Vec<_> = blog
            .clone()
            .into_pages_ahead(11, Limit::Pages(22), GetContentRequest { page: 0 })
            .try_collect()
            .await
            .unwrap();

        assert_eq!(results.len(), 22);
        assert_eq!(*results.last().unwrap().first().unwrap(), BlogRecord(21));

        let results: Vec<_> = blog
            .pages_ahead(0, Limit::None, GetContentRequest { page: 0 })
            .items()
            .try_collect()
            .await
            .unwrap();

        assert_eq!(results.len(), 0);

        blog.set_error(0);
        let mut stream = blog.pages_ahead(4, Limit::Pages(1), GetContentRequest { page: 0 });

        let item = stream.try_next().await;
        assert_eq!(item, Err("Custom error".to_owned()));

        let item = stream.try_next().await;
        assert_eq!(item, Ok(None), "pages_ahead stream must end after an error");
    }

    #[tokio::test]
    async fn pages_ahead_unordered() {
        let mut blog = BlogClient::new(33);

        let results: Vec<_> = blog
            .pages_ahead_unordered(6, Limit::None, GetContentRequest { page: 0 })
            .items()
            .try_collect()
            .await
            .unwrap();

        assert_eq!(results.len(), 33);
        assert_eq!(*results.last().unwrap(), BlogRecord(32));

        let results: Vec<_> = blog
            .clone()
            .into_pages_ahead_unordered(10, Limit::Pages(22), GetContentRequest { page: 0 })
            .try_collect()
            .await
            .unwrap();

        assert_eq!(results.len(), 22);
        assert_eq!(*results.last().unwrap().first().unwrap(), BlogRecord(21));

        let results: Vec<_> = blog
            .pages_ahead_unordered(0, Limit::None, GetContentRequest { page: 0 })
            .items()
            .try_collect()
            .await
            .unwrap();

        assert_eq!(results.len(), 0);

        blog.set_error(0);
        let stream = blog.pages_ahead_unordered(4, Limit::None, GetContentRequest { page: 0 });

        let items: Result<Vec<_>, _> = stream.try_collect().await;
        assert_eq!(items, Err("Custom error".to_owned()));
    }

    struct GetNumbersQuery {
        key: usize,
    }

    impl GetNumbersQuery {
        fn new() -> Self {
            Self { key: 0 }
        }
    }

    // It's a very artificial client to verify that paginated streams work as expected.
    // It doesn't serve as a good example of how the PageTurner can be implemented for real use cases.
    #[derive(Debug)]
    struct NumbersClient {
        numbers: Vec<usize>,
        page_size: usize,
        index: AtomicUsize,
    }

    impl NumbersClient {
        fn new(last_number: usize, page_size: usize) -> Self {
            NumbersClient {
                numbers: (1..=last_number).collect(),
                page_size,
                index: Default::default(),
            }
        }
    }

    #[async_trait]
    impl PageTurner<GetNumbersQuery> for NumbersClient {
        type PageItem = usize;
        type PageError = ();

        async fn turn_page(
            &self,
            query: GetNumbersQuery,
        ) -> PageTurnerOutput<Self, GetNumbersQuery> {
            self.index.store(query.key, Ordering::Release);

            let index = self.index.load(Ordering::Acquire);
            let response: Vec<_> = self.numbers[index..]
                .iter()
                .copied()
                .take(self.page_size)
                .collect();

            if index + self.page_size < self.numbers.len() {
                Ok(TurnedPage::next(
                    response,
                    GetNumbersQuery {
                        key: index + self.page_size,
                    },
                ))
            } else {
                Ok(TurnedPage::last(response))
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct BlogRecord(usize);

    #[derive(Clone)]
    struct BlogClient {
        content: Vec<Result<BlogRecord, String>>,
    }

    struct GetContentRequest {
        page: usize,
    }

    struct GetContentResponse {
        record: BlogRecord,
        next_page: Option<usize>,
    }

    impl BlogClient {
        fn new(amount: usize) -> Self {
            Self {
                content: (0..amount).map(BlogRecord).map(Ok).collect(),
            }
        }

        async fn get_content(&self, req: GetContentRequest) -> Result<GetContentResponse, String> {
            let record = self
                .content
                .get(req.page)
                .ok_or("The page is out of bound")?
                .clone()?;

            let next_page = (req.page + 1 < self.content.len()).then_some(req.page + 1);
            Ok(GetContentResponse { record, next_page })
        }

        fn set_error(&mut self, pos: usize) {
            self.content[pos] = Err("Custom error".into());
        }
    }

    #[async_trait]
    impl PageTurner<GetContentRequest> for BlogClient {
        type PageItem = BlogRecord;
        type PageError = String;

        async fn turn_page(
            &self,
            req: GetContentRequest,
        ) -> PageTurnerOutput<Self, GetContentRequest> {
            let response = self.get_content(req).await?;

            match response.next_page {
                Some(page) => Ok(TurnedPage::next(
                    vec![response.record],
                    GetContentRequest { page },
                )),
                None => Ok(TurnedPage::last(vec![response.record])),
            }
        }
    }

    impl RequestAhead for GetContentRequest {
        fn next_request(&self) -> GetContentRequest {
            GetContentRequest {
                page: self.page + 1,
            }
        }
    }
}
