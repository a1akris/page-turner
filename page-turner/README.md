# page-turner

#### A generic abstraction of paginated APIs

Imagine, you need to use the following API to find
the most upvoted comment under a blog post.

```rust
struct GetCommentsRequest {
    blog_post_id: BlogPostId,
    page_number: u32,
}

struct GetCommentsResponse {
    comments: Vec<Comment>,
    more_comments_available: bool,
}
```

In order to do that you will need to write a hairy loop that checks the `more_comments_available`
flag, increments `page_number`, and updates a variable that stores the resulting value.
This crate helps to abstract away any sort of pagination and allows you to work with such
APIs uniformly with the help of async streams.

First, lets implement the [`PageQuery`] trait for our request. The purpose of this trait
is to specify a type of the field that determines which page is queried and provide
a setter for it. In our case we need to implement it for the `page_number` field of `GetCommentsRequest`.

```rust
impl PageQuery for GetCommentsRequest {
    type PageKey = u32;

    fn set_page_key(&mut self, page_number: Self::PageKey) {
        self.page_number = page_number;
    }
}
```

Next, we need to implement the [`PageTurner`] trait for the client that sends our `GetCommentsRequest`.
The purpose of the [`PageTurner`] trait is to specify the queried item type, a query's error type,
how to query a single page with provided `PageQuery`, and how to retrieve a key for the next page.

```rust
#[async_trait]
impl PageTurner<GetCommentsRequest> for OurBlogClient {
    type PageItem = Comment;
    type PageError = OurClientError;

    async fn turn_page(&self, request: GetCommentsRequest) -> PageTurnerOutput<Self, GetCommentsRequest> {
        let current_page_number = request.page_number;

        let response = self.get_comments(request).await?;
        let next_page_number = response.more_comments_available.then(|| current_page_number + 1);

        (response.comments, next_page_number)
    }
}
```

With the [`PageTurner`] trait [`GetPagesStream`] and [`IntoPagesStream`] traits are auto-implemented
and now we can use our client to find the most upvoted comment:

```rust
let client = OurBlogClient::new();

let most_upvoted_comment = client
    .pages_flat(GetCommentsRequest { blog_post_id, page_number: 1 })
    .try_fold(None::<Comment>, |most_upvoted, next_comment| async move {
        match most_upvoted {
            Some(comment) if next_comment.upvotes > comment.upvotes => Ok(Some(next_comment)),
            current @ Some(_) => Ok(current),
            None => Ok(Some(next_comment)),
        }
    })
    .await?;

```

Or we can process the whole pages if needed

```rust
let comment_pages = client.pages(GetCommentsRequest { blog_post_id, page_number: 1 });

while let Some(comment_page) = comment_pages.try_next().await? {
    detect_spam(comment_page);
}
```

The [`PageQuery`] trait implementation is usually trivial, so you can simply derive it.
`#[page_key]` attribute determines the [`PageQuery::PageKey`]. `T` and `Option<T>` types are supported.

```rust
#[derive(PageQuery, Clone)]
struct GetCommentsRequest {
    blog_post_id: BlogPostId,

    #[page_key]
    page_number: u32,
}
```

