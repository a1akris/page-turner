![Build](https://github.com/a1akris/page-turner/actions/workflows/build.yml/badge.svg)

# page-turner

### A generic abstraction of paginated APIs

Imagine, you need to use the following API to find the most upvoted comment
under a blog post.

```rust
struct GetCommentsRequest {
    blog_post_id: BlogPostId,
    page_number: u32,
}

struct GetCommentsResponse {
    comments: Vec<Comment>,
    more_comments_available: bool,
}

# struct Comment {
#     upvotes: u32,
#     text: String,
# }
#
# type BlogPostId = u32;
```

In order to do that you will need to write a hairy loop that checks the
`more_comments_available` flag, increments `page_number`, and updates a
variable that stores the resulting value. This crate helps to abstract away any
sort of pagination and allows you to work with such APIs uniformly with the
help of async streams. All you need to do is to implement the [`PageTurner`]
trait instead for the client that sends `GetCommentsRequest`.

In [`PageTurner`] you specify what items you query and what errors may occur,
then you implement the `turn_page` method where you describe how to query a
single page and how to prepare a request for the next page.

```rust
use async_trait::async_trait;
use page_turner::prelude::*;

#[async_trait]
impl PageTurner<GetCommentsRequest> for OurBlogClient {
    type PageItem = Comment;
    type PageError = OurClientError;

    async fn turn_page(&self, mut request: GetCommentsRequest) -> PageTurnerOutput<Self, GetCommentsRequest> {
        let response = self.get_comments(request.clone()).await?;

        if response.more_comments_available {
            request.page_number += 1;
            Ok(TurnedPage::next(response.comments, request))
        } else {
            Ok(TurnedPage::last(response.comments))
        }
    }
}

# struct OurBlogClient {}

# impl OurBlogClient {
#    async fn get_comments(&self, req: GetCommentsRequest) -> Result<GetCommentsResponse, OurClientError> {
#        todo!()
#    }
# }

# struct Comment {}
# struct OurClientError {}
# type BlogPostId = u32;

# #[derive(Clone)]
# struct GetCommentsRequest {
#     blog_post_id: BlogPostId,
#     page_number: u32,
# }
#
# struct GetCommentsResponse {
#     comments: Vec<Comment>,
#     more_comments_available: bool,
# }
```

[`PageTurner`] then provides default implementations for [`PageTurner::pages`]
and [`PageTurner::into_pages`] methods that you can use to get a stream of
pages and, optionally, to turn it into a stream of items if you need. Now we
can use our client to find the most upvoted comment like that:

```rust
# type BlogPostId = u32;
#
# #[derive(Clone)]
# struct GetCommentsRequest {
#     blog_post_id: BlogPostId,
#     page_number: u32,
# }
#
# struct GetCommentsResponse {
#     comments: Vec<Comment>,
#     more_comments_available: bool,
# }
#
# struct Comment {
#     upvotes: u32,
#     text: String,
# }
#
# struct OurBlogClient {}
#
# impl OurBlogClient {
#     fn new() -> Self {
#         Self {}
#     }
#
#     async fn get_comments(&self, req: GetCommentsRequest) -> Result<GetCommentsResponse, OurClientError> {
#         Ok(GetCommentsResponse {
#             comments: vec![
#                 Comment {
#                     text: "First".to_owned(),
#                     upvotes: 0,
#                 },
#                 Comment {
#                     text: "Second".to_owned(),
#                     upvotes: 2,
#                 },
#                 Comment {
#                     text: "Yeet".to_owned(),
#                     upvotes: 5,
#                 }
#             ],
#             more_comments_available: false,
#         })
#     }
# }
#
# #[derive(Debug)]
# struct OurClientError {}
#
# use async_trait::async_trait;
# use page_turner::prelude::*;
#
# #[async_trait]
# impl PageTurner<GetCommentsRequest> for OurBlogClient {
#     type PageItem = Comment;
#     type PageError = OurClientError;
#
#     async fn turn_page(&self, mut request: GetCommentsRequest) -> PageTurnerOutput<Self, GetCommentsRequest> {
#         let response = self.get_comments(request.clone()).await?;
#
#         if response.more_comments_available {
#             request.page_number += 1;
#             Ok(TurnedPage::next(response.comments, request))
#         } else {
#             Ok(TurnedPage::last(response.comments))
#         }
#     }
# }
#
#
# use futures::TryStreamExt;
#
# #[tokio::main(flavor = "current_thread")]
# async fn main() -> Result<(), OurClientError> {
# let blog_post_id = 1337;
let client = OurBlogClient::new();

let most_upvoted_comment = client
    .pages(GetCommentsRequest { blog_post_id, page_number: 1 })
    .items()
    .try_fold(None::<Comment>, |most_upvoted, next_comment| async move {
        match most_upvoted {
            Some(comment) if next_comment.upvotes > comment.upvotes => Ok(Some(next_comment)),
            current @ Some(_) => Ok(current),
            None => Ok(Some(next_comment)),
        }
    })
    .await?
    .unwrap();

assert_eq!(most_upvoted_comment.text, "Yeet");
assert_eq!(most_upvoted_comment.upvotes, 5);

// Or we can process the whole pages if needed

let mut comment_pages = client.pages(GetCommentsRequest { blog_post_id, page_number: 1 });

while let Some(comment_page) = comment_pages.try_next().await? {
    detect_spam(comment_page);
}

#   Ok(())
# }
#
# fn detect_spam(page: Vec<Comment>) -> bool {
#    false
# }
```

Notice, that with this API we don't actually need any info from the response to
construct the next valid request. We can take an advantage on such kind of
requests by implementing the [`RequestAhead`] trait for them. For requests that
implement [`RequestAhead`] [`PageTurner`] provides additional methods -
[`PageTurner::pages_ahead`] and [`PageTurner::pages_ahead_unordered`]. These
methods allow to query multiple pages concurrently.

```rust
# type BlogPostId = u32;
#
# #[derive(Clone)]
# struct GetCommentsRequest {
#     blog_post_id: BlogPostId,
#     page_number: u32,
# }
#
# struct GetCommentsResponse {
#     comments: Vec<Comment>,
#     more_comments_available: bool,
# }
#
# struct Comment {
#     upvotes: u32,
#     text: String,
# }
#
# struct OurBlogClient {}
#
# impl OurBlogClient {
#     fn new() -> Self {
#         Self {}
#     }
#
#     async fn get_comments(&self, req: GetCommentsRequest) -> Result<GetCommentsResponse, OurClientError> {
#         Ok(GetCommentsResponse {
#             comments: vec![
#                 Comment {
#                     text: "First".to_owned(),
#                     upvotes: 0,
#                 },
#                 Comment {
#                     text: "Second".to_owned(),
#                     upvotes: 2,
#                 },
#                 Comment {
#                     text: "Yeet".to_owned(),
#                     upvotes: 5,
#                 }
#             ],
#             more_comments_available: false,
#         })
#     }
# }
#
# #[derive(Debug)]
# struct OurClientError {}
#
# use async_trait::async_trait;
# use page_turner::prelude::*;
#
# #[async_trait]
# impl PageTurner<GetCommentsRequest> for OurBlogClient {
#     type PageItem = Comment;
#     type PageError = OurClientError;
#
#     async fn turn_page(&self, mut request: GetCommentsRequest) -> PageTurnerOutput<Self, GetCommentsRequest> {
#         let response = self.get_comments(request.clone()).await?;
#
#         if response.more_comments_available {
#             request.page_number += 1;
#             Ok(TurnedPage::next(response.comments, request))
#         } else {
#             Ok(TurnedPage::last(response.comments))
#         }
#     }
# }
#
#
# use futures::TryStreamExt;
#
# #[tokio::main(flavor = "current_thread")]
# async fn main() -> Result<(), OurClientError> {
# let blog_post_id = 1337;
impl RequestAhead for GetCommentsRequest {
    fn next_request(&self) -> Self {
        Self {
            blog_post_id: self.blog_post_id,
            page_number: self.page_number + 1,
        }
    }
}

let client = OurBlogClient::new();

// Now instead of querying pages one by one we make up to 4 concurrent requests
// for multiple pages under the hood but besides using a different PageTurner
// method nothing changes in the user code.
let most_upvoted_comment = client
    .pages_ahead(4, Limit::None, GetCommentsRequest { blog_post_id, page_number: 1 })
    .items()
    .try_fold(None::<Comment>, |most_upvoted, next_comment| async move {
        match most_upvoted {
            Some(comment) if next_comment.upvotes > comment.upvotes => Ok(Some(next_comment)),
            current @ Some(_) => Ok(current),
            None => Ok(Some(next_comment)),
        }
    })
    .await?
    .unwrap();

assert_eq!(most_upvoted_comment.text, "Yeet");
assert_eq!(most_upvoted_comment.upvotes, 5);

// In the example above the order of pages being returned corresponds to the order
// of requests which means the stream is blocked until the first page is ready
// even if the second and the third pages are already received. For this use case
// we don't really care about the order of the comments so we can use
// pages_ahead_unordered to unblock the stream as soon as we receive a response to
// any of the concurrent requests.
let most_upvoted_comment = client
    .pages_ahead_unordered(4, Limit::None, GetCommentsRequest { blog_post_id, page_number: 1 })
    .items()
    .try_fold(None::<Comment>, |most_upvoted, next_comment| async move {
        match most_upvoted {
            Some(comment) if next_comment.upvotes > comment.upvotes => Ok(Some(next_comment)),
            current @ Some(_) => Ok(current),
            None => Ok(Some(next_comment)),
        }
    })
    .await?
    .unwrap();

assert_eq!(most_upvoted_comment.text, "Yeet");
assert_eq!(most_upvoted_comment.upvotes, 5);
#   Ok(())
# }
```


### Changelog

- **0.8.1:** Bugfix in internal chunking iterator that yilded empty chunks for
  `chunk_size = 1` in previous version. (0.8.0 yanked)
- **0.8.0:** Introduce [`RequestAhead`] and [`PageTurner::pages_ahead`],
  [`PageTurner::pages_ahead_unordered`] for concurrent page querying
- **0.7.0:** Hotfix lifetime bounds in [`PagesStream`] for `T` and `E`.
- **0.6.0:** Redesign, initial public release

#### License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT
license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
