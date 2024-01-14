# A generic abstraction of paginated APIs

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
paginated API and allows you to work with such APIs uniformly with the help of
async streams. Both cursor and non-cursor pagniations are supported. All you
need to do is to implement the [`PageTurner`] trait for the client that sends
`GetCommentsRequest`.

In [`PageTurner`] you specify what data you fetch and what errors may occur for
a particular request, then you implement the `turn_page` method where you
describe how to query a single page and how to prepare a request for the next
page.

```rust
use page_turner::prelude::*;

impl PageTurner<GetCommentsRequest> for BlogClient {
    type PageItems = Vec<Comment>;
    type PageError = BlogClientError;

    async fn turn_page(&self, mut request: GetCommentsRequest) -> TurnedPageResult<Self, GetCommentsRequest> {
        let response = self.get_comments(request.clone()).await?;

        if response.more_comments_available {
            request.page_number += 1;
            Ok(TurnedPage::next(response.comments, request))
        } else {
            Ok(TurnedPage::last(response.comments))
        }
    }
}

# struct BlogClient {}
#
# impl BlogClient {
#    async fn get_comments(&self, req: GetCommentsRequest) -> Result<GetCommentsResponse, BlogClientError> {
#        todo!()
#    }
# }
#
# struct Comment {}
# struct BlogClientError {}
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
```

[`PageTurner`] then provides default implementations for [`PageTurner::pages`]
and [`PageTurner::into_pages`] methods that you can use to get a stream of
pages and, optionally, to turn it into a stream of page items if you need. Now
we can use our client to find the most upvoted comment like that:

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
# struct BlogClient {}
#
# impl BlogClient {
#     fn new() -> Self {
#         Self {}
#     }
#
#     async fn get_comments(&self, req: GetCommentsRequest) -> Result<GetCommentsResponse, BlogClientError> {
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
# struct BlogClientError {}
#
# use page_turner::prelude::*;
#
# impl PageTurner<GetCommentsRequest> for BlogClient {
#     type PageItems = Vec<Comment>;
#     type PageError = BlogClientError;
#
#     async fn turn_page(&self, mut request: GetCommentsRequest) -> TurnedPageResult<Self, GetCommentsRequest> {
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
# async fn main() -> Result<(), BlogClientError> {
# let blog_post_id = 1337;
let client = BlogClient::new();

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

let mut comment_pages = std::pin::pin!(client.pages(GetCommentsRequest { blog_post_id, page_number: 1 }));

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

Notice, that with this kind of the API we don't require any data from response
to construct the next valid request. We can take an advantage on such APIs by
implementing the [`RequestAhead`] trait on a request type. For requests that
implement [`RequestAhead`] [`PageTurner`] provides additional methods -
[`PageTurner::pages_ahead`] and [`PageTurner::pages_ahead_unordered`]. These
methods allow to query multiple pages concurrently using an optimal [sliding
window](PageTurner::pages_ahead) request scheduling.

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
# struct BlogClient {}
#
# impl BlogClient {
#     fn new() -> Self {
#         Self {}
#     }
#
#     async fn get_comments(&self, req: GetCommentsRequest) -> Result<GetCommentsResponse, BlogClientError> {
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
# struct BlogClientError {}
#
# use page_turner::prelude::*;
#
# impl PageTurner<GetCommentsRequest> for BlogClient {
#     type PageItems = Vec<Comment>;
#     type PageError = BlogClientError;
#
#     async fn turn_page(&self, mut request: GetCommentsRequest) -> TurnedPageResult<Self, GetCommentsRequest> {
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
# async fn main() -> Result<(), BlogClientError> {
# let blog_post_id = 1337;
impl RequestAhead for GetCommentsRequest {
    fn next_request(&self) -> Self {
        Self {
            blog_post_id: self.blog_post_id,
            page_number: self.page_number + 1,
        }
    }
}

let client = BlogClient::new();

// Now instead of querying pages one by one we make 4 concurrent requests
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

## Page turner flavors and crate features

There are multiple falvors of page turners suitable for different contexts and
each flavor is available behind a feature flag. By default the `mt` feature is
enabled which provides you [`crate::mt::PageTurner`] that works with
multithreaded executors. The crate root `prelude` just reexports the
`crate::mt::prelude::*`, and types at the crate root are reexported from
`crate::mt`. Overall there are the following page turner flavors and
corresponding features available:

- [local](crate::local): Provides a less constrained [`crate::local::PageTurner`] suitable
  for singlethreaded executors. Use `page_turner::local::prelude::*` to work
  with it.
- [mutable](crate::mutable): A [`crate::mutable::PageTurner`] is like `local` but even allows your
  client to mutate during the request execution. Use
  `page_turner::mutable::prelude::*` to work with it.
- [mt](crate::mt): A [`crate::mt::PageTurner`] for multithreaded executors. It's
  reexported by default but if you enable additional features in the same
  project it is recommended to use `page_turner::mt::prelude::*` to distinguish
  between different flavors of page turners.
- [dynamic](crate::dynamic): An object safe [`crate::dynamic::PageTurner`] that requires
  `async_trait` to be implemented and can be used as an object with dynamic
  dispatch.

