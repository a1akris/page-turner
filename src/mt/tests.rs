use crate::mt::{prelude::*, PageError, PageItems};
use crate::test_utils::*;
use futures::TryStreamExt;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pages() {
    pages_base_test!().await;
    generic_pages_usage(NumbersClient::new(48, 7), GetNumbersQuery::default()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn pages_ahead() {
    pages_ahead_base_test!().await;
    generic_pages_ahead_usage(BlogClient::new(48), GetContentRequest { page: 0 }).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn pages_ahead_unordered() {
    pages_ahead_unordered_base_test!().await;
    generic_pages_ahead_unordered_usage(BlogClient::new(48), GetContentRequest { page: 0 }).await;
}

page_turner_impls!();

async fn generic_pages_usage<P, R>(p: P, req: R)
where
    P: PageTurner<R>,
    R: Clone + Send,
    PageItems<P, R>: IntoIterator,
    <PageItems<P, R> as IntoIterator>::IntoIter: Send,
    <PageItems<P, R> as IntoIterator>::Item: Send,
    PageError<P, R>: std::fmt::Debug,
{
    is_send(p.turn_page(req.clone()));

    let pages_stream = is_send(p.pages(req.clone()));
    generic_pages_stream_usage(pages_stream).await;
}

async fn generic_pages_ahead_usage<P, R>(p: P, req: R)
where
    P: PageTurner<R>,
    PageItems<P, R>: IntoIterator,
    <PageItems<P, R> as IntoIterator>::IntoIter: Send,
    <PageItems<P, R> as IntoIterator>::Item: Send,
    R: RequestAhead + Clone + Send,
    PageError<P, R>: std::fmt::Debug,
{
    is_send(p.turn_page(req.clone()));

    let pages_stream = is_send(p.pages_ahead(4, Limit::None, req.clone()));
    generic_pages_stream_usage(pages_stream).await;
}

async fn generic_pages_ahead_unordered_usage<'p, P, R>(p: P, req: R)
where
    P: PageTurner<R>,
    R: RequestAhead + Clone + Send,
    PageItems<P, R>: IntoIterator,
    <PageItems<P, R> as IntoIterator>::IntoIter: Send,
    <PageItems<P, R> as IntoIterator>::Item: Send,
    PageError<P, R>: std::fmt::Debug,
{
    is_send(p.turn_page(req.clone()));

    let pages_stream = is_send(p.pages_ahead_unordered(4, Limit::None, req));
    generic_pages_stream_usage(pages_stream).await;
}

async fn generic_pages_stream_usage<'p, T, E>(s: impl 'p + PagesStream<'p, T, E>)
where
    T: Send + IntoIterator,
    E: std::fmt::Debug + Send,
    <T as IntoIterator>::Item: Send,
    <T as IntoIterator>::IntoIter: Send,
{
    std::pin::pin!(is_send(s.items())).try_next().await.unwrap();
}

fn is_send<T: Send>(t: T) -> T {
    t
}

#[cfg(feature = "dynamic")]
mod dynamic {
    use super::is_send;
    use crate::dynamic::prelude::*;
    use crate::test_utils::*;
    use async_trait::async_trait;
    use futures::TryStreamExt;
    use std::sync::Arc;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn pages() {
        pages_base_test!().await;
        dyn_pages_usage(Arc::new(BlogClient::new(42))).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pages_ahead() {
        pages_ahead_base_test!().await;
        dyn_pages_ahead_usage(Arc::new(BlogClient::new(42))).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pages_ahead_unordered() {
        pages_ahead_unordered_base_test!().await;
        dyn_pages_ahead_unordered_usage(Arc::new(BlogClient::new(42))).await;
    }

    page_turner_impls!(async_trait);

    async fn dyn_pages_usage(
        p: Arc<dyn PageTurner<GetContentRequest, PageItems = Vec<BlogRecord>, PageError = String>>,
    ) {
        is_send(p.turn_page(GetContentRequest { page: 0 }));

        let pages_stream = is_send(p.pages(GetContentRequest { page: 0 }));
        generic_pages_stream_usage(pages_stream).await;
    }

    async fn dyn_pages_ahead_usage(
        p: Arc<dyn PageTurner<GetContentRequest, PageItems = Vec<BlogRecord>, PageError = String>>,
    ) {
        is_send(p.turn_page(GetContentRequest { page: 0 }));

        let pages_stream = is_send(p.pages_ahead(3, Limit::None, GetContentRequest { page: 0 }));
        generic_pages_stream_usage(pages_stream).await;
    }

    async fn dyn_pages_ahead_unordered_usage(
        p: Arc<dyn PageTurner<GetContentRequest, PageItems = Vec<BlogRecord>, PageError = String>>,
    ) {
        is_send(p.turn_page(GetContentRequest { page: 0 }));

        let pages_stream =
            is_send(p.pages_ahead_unordered(2, Limit::None, GetContentRequest { page: 0 }));

        generic_pages_stream_usage(pages_stream).await;
    }

    async fn generic_pages_stream_usage<'p, T, E>(s: impl 'p + PagesStream<'p, T, E>)
    where
        T: Send + IntoIterator,
        E: std::fmt::Debug + Send,
        <T as IntoIterator>::Item: Send,
        <T as IntoIterator>::IntoIter: Send,
    {
        std::pin::pin!(is_send(s.items())).try_next().await.unwrap();
    }
}
