![Build](https://github.com/a1akris/page-turner/actions/workflows/build.yml/badge.svg)

# page-turner

A generic abstraction of paginated APIs

## In a nutshell

If you have a paginated request implement a page turner trait for this request
on your client:

```rust
use page_turner::prelude::*;

impl PageTurner<GetReviewsRequest> for ApiClient {
    type PageItems = Vec<Review>;
    type PageError = ApiError;

    async fn turn_page(&self, request: GetReviewsRequest) -> TurnedPageResult<Self, GetReviewsRequest> {
        let response = self.execute(request).await?;

        let turned_page = match response.next_page_token {
            Some(token) => TurnedPage::next(response.reviews, GetReviewsRequest::from(token)),
            None => TurnedPage::last(response.reviews),
        };

        Ok(turned_page)
    }
}
```

The page turner then provides stream-based APIs that allow data to be queried
as if pagination does not exist:

```rust
use page_turner::prelude::*;
use futures::{StreamExt, TryStreamExt};

async fn first_n_positive_reviews(client: &ApiClient, request: GetReviewsRequest, count: usize) -> ApiResult<Vec<Review>> {
    client
        .pages(request)
        .items()
        .try_filter(|review| std::future::ready(review.is_positive()))
        .take(count)
        .await
}

```

Both cursor and non-cursor pagination patterns are supported and for the later
one you can enable a concurrent querying by implementing the `RequestAhead`
trait:

```rust
pub struct GetCommentsRequest {
    pub page_id: PageId,
}

impl RequestAhead for GetCommentsRequest {
    fn next_request(&self) -> Self {
        Self {
            page_id: self.page_id + 1,
        }
    }
}
```

Now you can use `pages_ahead`/`pages_ahead_unordered` family of methods to
request multiple pages concurrently using a quite optimal [sliding
window](https://docs.rs/page-turner/1.0.0/page_turner/mt/trait.PageTurner.html#method.pages_ahead)
request scheduling under the hood:

```rust
use page_turner::prelude::*;
use futures::TryStreamExt;

async fn retrieve_user_comments(username: &str) -> ResponseResult<Vec<Comment>> {
    let client = ForumClient::new();

    client.pages_ahead(4, Limit::None, GetCommentsRequest { page_id: 1 })
        .items()
        .try_filter(|comment| std::future::ready(comment.author == username))
        .try_collect()
        .await

}
```

The example above schedules requests for 4 pages simultaneously and then issues
a request as soon as you receive a response concurrently awaiting for 4
responses all the time while you're processing results.


## v1.0.0 release

The `v1.0.0` uses features like RPITIT stabilized in Rust 1.75, so MSRV for
`v1.0.0` is `1.75.0`. If you can't afford to upgrade to Rust `1.75` use `0.8.2`
version of the crate. It's quite similar and supports Rust versions the
`async_trait` crate supports.

See [docs](https://docs.rs/page-turner) for details about new supported
features.

See [CHANGELOG.md](CHANGELOG.md) for full changes history.


### Migration to v1.0.0 from older versions

There are several major breaking changes in v1.0.0, here are instructions how
to quickly adopt them:

1. No more `#[async_trait]` by default. Remove `#[async_trait]` from your page
   turner impls and everything should work. If you for some reason rely on `dyn
   PageTurner` then enable feature `dynamic` and use
   `page_turner::dynamic::prelude::*` instead of the `page_turner::prelude::*`.

1. New page turners don't enforce you to return `Vec<PageItem>` anymore, now
   you can return whatever you like(`HashMap` is a one example of a popular
   alternative). To quickly make your code compile retaining the old behavior
   replace `type PageItem = YourItem;` with `type PageItems = Vec<YourItem>;`.
   Note that `s` in `PageItems` :)

1. `PageTurnerOutput` was renamed into `TurnedPageResult` but it is the same
   type alias so a simple global search&replace should do the trick.

1. `into_pages_ahead` and `into_pages_ahead_unordered` methods now require
   implementors to be clonable. Previously, they used `Arc` under the hood, but
   now it's up to you. Most likely your clients are already cheaply clonable
   but if not then the quickest way to fix `doesn't implement Clone` errors is
   to wrap your clients into `Arc` like
   `Arc::new(client).into_pages_ahead(..)`.


##### License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT
license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
