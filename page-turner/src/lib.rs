//! A generic abstraction of paginated APIs.
//!
//! Imagine, you need to use the following API to find
//! the most upvoted comment under a blog post.
//!
//! ```ignore
//! struct GetCommentsRequest {
//!     blog_post_id: BlogPostId,
//!     page_number: u32,
//! }
//!
//! struct GetCommentsResponse {
//!     comments: Vec<Comment>,
//!     more_comments_available: bool,
//! }
//! ```
//!
//! In order to do that you will need to write a hairy loop that checks the `more_comments_available`
//! flag, increments `page_number`, and updates a variable that stores the resulting value.
//! This crate helps to abstract away any sort of pagination and allows you to work with such
//! APIs uniformly with the help of async streams.
//!
//! First, lets implement the [`PageQuery`] trait for our request. The purpose of this trait
//! is to specify a type of the field that determines which page is queried and provide
//! a setter for it. In our case we need to implement it for the `page_number` field of `GetCommentsRequest`.
//!
//! ```ignore
//! impl PageQuery for GetCommentsRequest {
//!     type PageKey = u32;
//!
//!     fn set_page_key(&mut self, page_number: Self::PageKey) {
//!         self.page_number = page_number;
//!     }
//! }
//! ```
//!
//! Next, we need to implement the [`PageTurner`] trait for the client that sends our `GetCommentsRequest`.
//! The purpose of the [`PageTurner`] trait is to specify the queried item type, a query's error type,
//! how to query a single page with provided `PageQuery`, and how to retrieve a key for the next page.
//!
//! ```ignore
//! #[async_trait]
//! impl PageTurner<GetCommentsRequest> for OurBlogClient {
//!     type PageItem = Comment;
//!     type PageError = OurClientError;
//!
//!     async fn turn_page(&self, request: GetCommentsRequest) -> PageTurnerOutput<Self, GetCommentsRequest> {
//!         let current_page_number = request.page_number;
//!
//!         let response = self.get_comments(request).await?;
//!         let next_page_number = response.more_comments_available.then(|| current_page_number + 1);
//!
//!         (response.comments, next_page_number)
//!     }
//! }
//! ```
//!
//! With the [`PageTurner`] trait [`GetPagesStream`] and [`IntoPagesStream`] traits are auto-implemented
//! and now we can use our client to find the most upvoted comment:
//!
//! ```ignore
//! let client = OurBlogClient::new();
//!
//! let most_upvoted_comment = client
//!     .page_items(GetCommentsRequest { blog_post_id, page_number: 1 })
//!     .try_fold(None::<Comment>, |most_upvoted, next_comment| async move {
//!         match most_upvoted {
//!             Some(comment) if next_comment.upvotes > comment.upvotes => Ok(Some(next_comment)),
//!             current @ Some(_) => Ok(current),
//!             None => Ok(Some(next_comment)),
//!         }
//!     })
//!     .await?;
//!
//! ```
//!
//! Or we can process the whole pages if needed
//!
//! ```ignore
//! let comment_pages = client.pages(GetCommentsRequest { blog_post_id, page_number: 1 });
//!
//! while let Some(comment_page) = comment_pages.try_next().await? {
//!     detect_spam(comment_page);
//! }
//! ```
//!
//! The [`PageQuery`] trait implementation is usually trivial, so you can simply derive it.
//! `#[page_key]` attribute determines the [`PageQuery::PageKey`]. `T` and `Option<T>` types are supported.
//!
//! ```ignore
//! #[derive(PageQuery, Clone)]
//! struct GetCommentsRequest {
//!     blog_post_id: BlogPostId,
//!
//!     #[page_key]
//!     page_number: u32,
//! }
//! ```

use async_trait::async_trait;
use futures::{stream, Stream, TryStreamExt};
pub use page_turner_macros::PageQuery;
use std::{ops::Deref, pin::Pin};

/// A handy shortcut that deduces the return type of [`PageTurner::turn_page`] for you.
pub type PageTurnerOutput<P, Q> = TurnedPage<
    <P as PageTurner<Q>>::PageItem,
    <P as PageTurner<Q>>::PageError,
    <Q as PageQuery>::PageKey,
>;

/// An output of the [`PageTurner::turn_page`] method. The Ok part of `Result` consists of `Vec` of
/// page items and an optional next page key. If the key is `None` then pagination is terminated.
pub type TurnedPage<T, E, NextPageKey> = Result<(Vec<T>, Option<NextPageKey>), E>;

/// A stream of page items returned by [`GetPagesStream::page_items`] and
/// [`IntoPagesStream::into_page_items`] methods.
pub type PageItemsStream<'a, T, E> = Pin<Box<dyn Stream<Item = Result<T, E>> + Send + 'a>>;

/// A stream of pages returned by [`GetPagesStream::pages`] and [`IntoPagesStream::into_pages`]
/// methods.
pub type PagesStream<'a, T, E> = Pin<Box<dyn Stream<Item = Result<Vec<T>, E>> + Send + 'a>>;

/// The trait for requests that support pagination. Requires to set a type of the field that
/// determines which page is queried and requires to provide a setter for it.
///
/// There is a procedural macro `#[derive(PageQuery)]` that you can simply derive on a struct to
/// implement this trait. Use `#[page_key]` to specify the page key field.
pub trait PageQuery: Send + Sync + Clone + 'static {
    type PageKey;

    /// Implementation should simply update the page key
    fn set_page_key(&mut self, next_key: Self::PageKey);
}

/// The trait that defines how to query a single page, the type of the page items and possible
/// errors during the query execution. Should be implemented by the clients that send [`PageQuery`]
/// requests. All types that implement this trait automatically implement [`GetPagesStream`] and
/// [`IntoPagesStream`] traits that can be used to get streams of [`PageTurner::PageItem`].
#[async_trait]
pub trait PageTurner<Q: PageQuery>: Send + Sync + 'static {
    type PageItem: Send;
    type PageError: Send;

    /// Returns `Result<(Vec<PageItem>, Option<NextPageKey>), PageError>`. `Option<NextPageKey> ==
    /// None` signals to terminate the pagination. Beware that if you return the same page key that
    /// you get in the `query` you can trigger an endless loop.
    async fn turn_page(&self, query: Q) -> PageTurnerOutput<Self, Q>;
}

/// A trait that is auto-implemented for all types that implement the [`PageTurner`] trait.
/// Its methods return streams that handle all pagination for you.
pub trait GetPagesStream<Q> {
    type PageItem: Send;
    type PageError: Send;

    /// Returns the stream of page items. You can use [`futures::TryStreamExt`] combinators
    /// on it. Beware that if you need only parital results you must limit this stream using
    /// [`futures::StreamExt::take`] or [`futures::TryStreamExt::try_take_while`] or simillar
    /// combinators. Otherwise all pages will be queried.
    fn page_items(&self, query: Q) -> PageItemsStream<'_, Self::PageItem, Self::PageError>;

    /// Returns the stream of pages. The page is a `Vec<Self::PageItem>`. This is useful when you
    /// need to process data in chunks, count pages, etc...
    fn pages(&self, query: Q) -> PagesStream<'_, Self::PageItem, Self::PageError>;
}

/// The same as [`GetPagesStream`] but consumes the client to return a stream bounded
/// by the `'static` lifetime.  May be useful if you face some lifetime issues with
/// [`GetPagesStream`]. However, this comes at a cost. The implementation clones the client
/// with each [`PageTurner::turn_page`] call, so you need to ensure that cloning a client is possible
/// and cheap. The best way to do that is to simply wrap the client into [`std::sync::Arc`].
///
/// ```text
/// Arc::new(client).into_page_items(...)
/// ```
pub trait IntoPagesStream<Q> {
    type PageItem: Send;
    type PageError: Send;

    /// The same stream as [`GetPagesStream::page_items`] but bounded by a `'static` lifetime
    fn into_page_items(self, query: Q)
        -> PageItemsStream<'static, Self::PageItem, Self::PageError>;

    /// The same stream as [`GetPagesStream::pages`] but bounded by a `'static` lifetime
    fn into_pages(self, query: Q) -> PagesStream<'static, Self::PageItem, Self::PageError>;
}

#[async_trait]
impl<D, P, Q> PageTurner<Q> for D
where
    D: Deref<Target = P> + Send + Sync + 'static,
    P: PageTurner<Q>,
    Q: PageQuery,
{
    type PageItem = P::PageItem;
    type PageError = P::PageError;

    async fn turn_page(&self, query: Q) -> PageTurnerOutput<Self, Q> {
        self.deref().turn_page(query).await
    }
}

impl<P, Q> IntoPagesStream<Q> for P
where
    P: PageTurner<Q> + Clone,
    Q: PageQuery,
{
    type PageItem = P::PageItem;
    type PageError = P::PageError;

    fn into_page_items(
        self,
        query: Q,
    ) -> PageItemsStream<'static, Self::PageItem, Self::PageError> {
        let stream = owned_base_stream(self, query)
            .map_ok(|items| stream::iter(items.into_iter().map(Ok)))
            .try_flatten();

        Box::pin(stream)
    }

    fn into_pages(self, query: Q) -> PagesStream<'static, Self::PageItem, Self::PageError> {
        Box::pin(owned_base_stream(self, query))
    }
}

impl<P, Q> GetPagesStream<Q> for P
where
    P: PageTurner<Q>,
    Q: PageQuery,
{
    type PageItem = P::PageItem;
    type PageError = P::PageError;

    fn page_items(&self, query: Q) -> PageItemsStream<'_, Self::PageItem, Self::PageError> {
        let stream = bounded_base_stream(self, query)
            .map_ok(|items| stream::iter(items.into_iter().map(Ok)))
            .try_flatten();

        Box::pin(stream)
    }

    fn pages(&self, query: Q) -> PagesStream<'_, Self::PageItem, Self::PageError> {
        Box::pin(bounded_base_stream(self, query))
    }
}

enum StreamState<Q> {
    NextPage { query: Q },
    End,
}

type Page<P, Q> = Result<Vec<<P as PageTurner<Q>>::PageItem>, <P as PageTurner<Q>>::PageError>;

/// Construct a stream bounded by the 'page_turner lifetime
fn bounded_base_stream<P, Q>(page_turner: &P, query: Q) -> impl Stream<Item = Page<P, Q>> + '_
where
    P: PageTurner<Q>,
    Q: PageQuery,
{
    stream::try_unfold(StreamState::NextPage { query }, move |state| async move {
        let mut query = match state {
            StreamState::NextPage { query } => query,
            StreamState::End => return Ok(None),
        };

        let (items, next_key) = page_turner.turn_page(query.clone()).await?;

        let next_state = match next_key {
            Some(key) => {
                query.set_page_key(key);
                StreamState::NextPage { query }
            }
            None => StreamState::End,
        };

        Ok(Some((items, next_state)))
    })
}

/// Construct a stream bounded by a 'static lifetime
fn owned_base_stream<P, Q>(page_turner: P, query: Q) -> impl Stream<Item = Page<P, Q>> + 'static
where
    P: PageTurner<Q> + Clone,
    Q: PageQuery,
{
    stream::try_unfold(StreamState::NextPage { query }, move |state| {
        let page_turner = page_turner.clone();

        async move {
            let mut query = match state {
                StreamState::NextPage { query } => query,
                StreamState::End => return Ok(None),
            };

            let (items, next_key) = page_turner.turn_page(query.clone()).await?;

            let next_state = match next_key {
                Some(key) => {
                    query.set_page_key(key);
                    StreamState::NextPage { query }
                }
                None => StreamState::End,
            };

            Ok(Some((items, next_state)))
        }
    })
}

// It's an artificial test to verify that paginated streams work as expected.
// It's not how the PageTurner will be actually used in real code.
#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use super::*;

    #[derive(Copy, Clone)]
    struct GetNumbersQuery {
        key: usize,
    }

    impl GetNumbersQuery {
        fn new() -> Self {
            Self { key: 0 }
        }
    }

    impl PageQuery for GetNumbersQuery {
        type PageKey = usize;

        fn set_page_key(&mut self, key: Self::PageKey) {
            self.key = key;
        }
    }

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

            let response: Vec<_> = self.numbers[self.index.load(Ordering::Acquire)..]
                .iter()
                .copied()
                .take(self.page_size)
                .collect();

            let index = self.index.load(Ordering::Acquire);
            let next_key =
                (index + self.page_size < self.numbers.len()).then(|| index + self.page_size);

            Ok((response, next_key))
        }
    }

    #[tokio::test]
    async fn page_turner_smoke_test() {
        // This line tests that auto impls for Arc<PageTurner>
        // and IntoPagesStream<PageTurner + Clone> work
        let client = Arc::new(NumbersClient::new(30, 30));
        let expected: Vec<usize> = (1..=30).collect();

        let (output, _) = client.turn_page(GetNumbersQuery::new()).await.unwrap();
        assert_eq!(output, expected, "Without stream");

        let pages: Vec<_> = client
            .pages(GetNumbersQuery::new())
            .try_collect()
            .await
            .unwrap();
        assert_eq!(pages.len(), 1, "There should be only one page");

        assert_eq!(pages[0].len(), 30, "The page must contain 30 items");

        let output: Vec<_> = client
            .into_page_items(GetNumbersQuery::new())
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
            .page_items(GetNumbersQuery::new())
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
            .page_items(GetNumbersQuery::new())
            .try_collect()
            .await
            .unwrap();

        assert_eq!(
            output, expected,
            "After paginated query with page_size = 19"
        );
    }
}
