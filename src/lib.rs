#![doc = include_str!("../README.md")]

pub mod prelude;

use async_trait::async_trait;
use futures::{
    stream::{self, BoxStream},
    Stream, StreamExt, TryStreamExt,
};

/// The trait is supposed to be implemented on API clients. The implementor needs to specify the
/// [`PageTurner::PageItem`]s to return and [`PageTurner::PageError`]s that may occur. Then it must
/// implement the [`PageTurner::turn_page`] method to describe how to query a single page and how
/// to prepare a request to query the next page. After that default [`PageTurner::pages`] and
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

    /// Returns a borrowed [`PagesStream`] that cannot outlive `self`. This may dissappoint the
    /// borrow checker in certain situations so you can use [`PageTurner::into_pages`] if you need an owned
    /// stream.
    fn pages(&self, request: R) -> PagesStream<'_, Self::PageItem, Self::PageError> {
        stream::try_unfold(StreamState::NextPage { request }, move |state| {
            request_next_page(self, state)
        })
        .boxed()
        .into()
    }

    /// Returns an owned [`PagesStream`]. In certain situations you can't use borrowed streams. For
    /// example, you can't use a borrowed stream if a stream should outlive a client in APIs like
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
    /// Now the client is consumed into a stream and is used internally. However, this comes at a
    /// cost. The current implementation is based on the `FnMut` closure which clones the client
    /// every time it requests a page. If your client is not cheaply clonable or not clonable at
    /// all just wrap it into [`std::sync::Arc`] like that:
    ///
    /// ```ignore
    /// fn get_stuff(params: Params) -> impl Stream<Item = Stuff> {
    ///     let client = StuffClient::new(params);
    ///     Arc::new(client).into_pages(GetStuff::from_params(params))
    /// }
    /// ```
    fn into_pages(self, request: R) -> OwnedPagesStream<Self::PageItem, Self::PageError>
    where
        Self: 'static + Sized + Clone,
    {
        stream::try_unfold(StreamState::NextPage { request }, move |state| {
            request_next_page(self.clone(), state)
        })
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

/// A stream of `Result<Vec<PageTurner::PageItem>, PageTurner::PageError>`
pub struct PagesStream<'a, T, E>(BoxStream<'a, Result<Vec<T>, E>>);
pub type OwnedPagesStream<T, E> = PagesStream<'static, T, E>;

impl<'a, T, E> PagesStream<'a, T, E>
where
    T: 'static + Send,
    E: 'static + Send,
{
    /// Gets items of the page. This effectively performs pages flattening turning
    /// `Result<Vec<T>, E>` into `Result<T, E>`. The `flatten()` name is not used
    /// to not interfere with [`futures::StreamExt::flatten`] that you may want to call
    /// on the resulting stream.
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

enum StreamState<R> {
    NextPage { request: R },
    End,
}

async fn request_next_page<P, R>(
    page_turner: P,
    state: StreamState<R>,
) -> Result<
    Option<(Vec<<P as PageTurner<R>>::PageItem>, StreamState<R>)>,
    <P as PageTurner<R>>::PageError,
>
where
    P: PageTurner<R>,
    R: 'static + Send,
{
    let request = match state {
        StreamState::NextPage { request } => request,
        StreamState::End => return Ok(None),
    };

    let TurnedPage {
        items,
        next_request,
    } = page_turner.turn_page(request).await?;

    let next_state = match next_request {
        Some(request) => StreamState::NextPage { request },
        None => StreamState::End,
    };

    Ok(Some((items, next_state)))
}

// It's an artificial test to verify that paginated streams work as expected.
// It's not a good example of how the PageTurner should be implemented for real use cases.
#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use super::*;

    struct GetNumbersQuery {
        key: usize,
    }

    impl GetNumbersQuery {
        fn new() -> Self {
            Self { key: 0 }
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

    #[tokio::test]
    async fn page_turner_smoke_test() {
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
    }
}
