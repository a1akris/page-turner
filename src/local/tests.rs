use crate::local::{prelude::*, PageError, PageItems};
use crate::test_utils::*;
use futures::TryStreamExt;

#[tokio::test(flavor = "current_thread")]
async fn pages() {
    pages_base_test!().await;
    generic_pages_usage(NumbersClient::new(17, 3), GetNumbersQuery::default()).await;
}

#[tokio::test(flavor = "current_thread")]
async fn pages_ahead() {
    pages_ahead_base_test!().await;
    generic_pages_ahead_usage(BlogClient::new(49), GetContentRequest { page: 0 }).await;
}

#[tokio::test(flavor = "current_thread")]
async fn pages_ahead_unordered() {
    pages_ahead_unordered_base_test!().await;
    generic_pages_ahead_unordered_usage(BlogClient::new(49), GetContentRequest { page: 0 }).await;
}

page_turner_impls!();

async fn generic_pages_usage<P, R>(p: P, req: R)
where
    P: PageTurner<R>,
    PageItems<P, R>: IntoIterator,
    PageError<P, R>: std::fmt::Debug,
{
    let pages_stream = p.pages(req);
    generic_pages_stream_usage(pages_stream).await;
}

async fn generic_pages_ahead_usage<P, R>(p: P, req: R)
where
    P: PageTurner<R>,
    R: RequestAhead,
    PageItems<P, R>: IntoIterator,
    PageError<P, R>: std::fmt::Debug,
{
    let pages_stream = p.pages_ahead(2, Limit::None, req);
    generic_pages_stream_usage(pages_stream).await;
}

async fn generic_pages_ahead_unordered_usage<P, R>(p: P, req: R)
where
    P: PageTurner<R>,
    R: RequestAhead,
    PageItems<P, R>: IntoIterator,
    PageError<P, R>: std::fmt::Debug,
{
    let pages_stream = p.pages_ahead(2, Limit::None, req);
    generic_pages_stream_usage(pages_stream).await;
}

async fn generic_pages_stream_usage<'p, T, E>(s: impl 'p + PagesStream<'p, T, E>)
where
    T: IntoIterator,
    E: std::fmt::Debug,
{
    std::pin::pin!(s.items()).try_next().await.unwrap();
}

#[cfg(feature = "mutable")]
mod mutable {
    use crate::mutable::{prelude::*, PageError, PageItems};
    use crate::test_utils::*;
    use futures::TryStreamExt;

    #[tokio::test(flavor = "current_thread")]
    async fn pages() {
        pages_base_test!(mut).await;
        generic_pages_usage(NumbersClient::new(19, 5), GetNumbersQuery::default()).await;
    }

    page_turner_impls!(mut);

    async fn generic_pages_usage<P, R>(mut p: P, req: R)
    where
        P: PageTurner<R>,
        PageItems<P, R>: IntoIterator,
        PageError<P, R>: std::fmt::Debug,
    {
        let pages_stream = p.pages(req);
        generic_pages_stream_usage(pages_stream).await;
    }

    async fn generic_pages_stream_usage<'p, T, E>(s: impl 'p + PagesStream<'p, T, E>)
    where
        T: IntoIterator,
        E: std::fmt::Debug,
    {
        std::pin::pin!(s.items()).try_next().await.unwrap();
    }
}
