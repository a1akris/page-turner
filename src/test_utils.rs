#[derive(Debug)]
pub struct NumbersClient {
    pub numbers: Vec<usize>,
    pub page_size: usize,
}

#[derive(Default, Clone)]
pub struct GetNumbersQuery {
    pub key: usize,
}

impl NumbersClient {
    pub fn new(last_number: usize, page_size: usize) -> Self {
        NumbersClient {
            numbers: (1..=last_number).collect(),
            page_size,
        }
    }
}

pub struct BlogClient {
    content: Vec<Result<BlogRecord, String>>,
}

impl Clone for BlogClient {
    fn clone(&self) -> Self {
        panic!("BlogClient Clone MUST NOT BE TRIGGERED");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlogRecord(pub usize);

#[derive(Debug, Clone)]
pub struct GetContentRequest {
    pub page: usize,
}

impl RequestAhead for GetContentRequest {
    fn next_request(&self) -> Self {
        Self {
            page: self.page + 1,
        }
    }
}

pub struct GetContentResponse {
    pub record: BlogRecord,
    pub next_page: Option<usize>,
}

impl BlogClient {
    pub fn new(amount: usize) -> Self {
        Self {
            content: (0..amount).map(BlogRecord).map(Ok).collect(),
        }
    }

    pub async fn get_content(&self, req: GetContentRequest) -> Result<GetContentResponse, String> {
        let record = self
            .content
            .get(req.page)
            .ok_or("The page is out of bound")?
            .clone()?;

        let next_page = (req.page + 1 < self.content.len()).then_some(req.page + 1);
        Ok(GetContentResponse { record, next_page })
    }

    pub fn set_error(&mut self, pos: usize) {
        self.set_error_with_msg(pos, "Custom error");
    }

    pub fn set_error_with_msg(&mut self, pos: usize, msg: &'static str) {
        self.content[pos] = Err(msg.into())
    }
}

macro_rules! numbers_client_page_turner_impl {
    (@types) => {
        type PageItems = Vec<usize>;
        type PageError = ();
    };
    (@body, $client:ident, $query:ident) => {{
        let index = $query.key;

        let response: Vec<_> = $client.numbers[index..]
            .iter()
            .copied()
            .take($client.page_size)
            .collect();

        if index + $client.page_size < $client.numbers.len() {
            Ok(TurnedPage::next(
                response,
                GetNumbersQuery {
                    key: index + $client.page_size,
                },
            ))
        } else {
            Ok(TurnedPage::last(response))
        }
    }};
    (async_trait) => {
        #[async_trait]
        impl PageTurner<GetNumbersQuery> for NumbersClient {
            numbers_client_page_turner_impl!(@types);

            async fn turn_page(
                &self,
                request: GetNumbersQuery,
            ) -> TurnedPageResult<Self, GetNumbersQuery> {
                numbers_client_page_turner_impl!(@body, self, request)
            }
        }
    };
    ($($mutability:tt)*) => {
        impl PageTurner<GetNumbersQuery> for NumbersClient {
            numbers_client_page_turner_impl!(@types);

            async fn turn_page(
                &$($mutability)* self,
                request: GetNumbersQuery,
            ) -> TurnedPageResult<Self, GetNumbersQuery> {
                numbers_client_page_turner_impl!(@body, self, request)
            }
        }
    };
}

macro_rules! numbers_client_pages_base_test {
    ($($mutability:tt)*) => {
        async {
            let $($mutability)* client = NumbersClient::new(30, 30);
            let expected: Vec<_> = (1..=30).collect();

            let pages: Vec<_> = client
                .pages(GetNumbersQuery::default())
                .try_collect()
                .await
                .unwrap();

            assert_eq!(pages.len(), 1, "There should be only one page");
            assert_eq!(pages[0].len(), 30, "The page must contain 30 items");

            let output: Vec<_> = client
                .into_pages(GetNumbersQuery::default())
                .items()
                .try_collect()
                .await
                .unwrap();

            assert_eq!(
                output, expected,
                "After paginated query with page_size = 30"
            );

            let $($mutability)* client = NumbersClient::new(30, 10);

            let pages: Vec<_> = client
                .pages(GetNumbersQuery::default())
                .try_collect()
                .await
                .unwrap();

            assert_eq!(pages.len(), 3, "There should be 3 pages");

            for page in pages {
                assert_eq!(page.len(), 10, "Each page must contain 10 elements")
            }

            let output: Vec<_> = client
                .into_pages(GetNumbersQuery::default())
                .items()
                .try_collect()
                .await
                .unwrap();

            assert_eq!(
                output, expected,
                "After paginated query with page_size = 10"
            );

            let $($mutability)* client = NumbersClient::new(30, 19);

            let pages: Vec<_> = client
                .pages(GetNumbersQuery::default())
                .try_collect()
                .await
                .unwrap();

            assert_eq!(pages.len(), 2, "There should be 2 pages");

            assert_eq!(pages[0].len(), 19, "The first page must contain 19 items");
            assert_eq!(pages[1].len(), 11, "The second page must contain 11 items");

            let output: Vec<_> = client
                .into_pages(GetNumbersQuery::default())
                .items()
                .try_collect()
                .await
                .unwrap();

            assert_eq!(
                output, expected,
                "After paginated query with page_size = 19"
            );

            fn consumed_numbers_client() -> impl futures::stream::Stream<Item = Result<Vec<usize>, ()>> {
                let client = NumbersClient::new(48, 13);
                client.into_pages(GetNumbersQuery::default())
            }

            let pages: Vec<_> = consumed_numbers_client().try_collect().await.unwrap();
            assert_eq!(pages.len(), 4, "Consumed numbers client must produce 4 pages");
        }
    };
}

macro_rules! blogs_client_page_turner_impl {
    (@types) => {
        type PageItems = Vec<BlogRecord>;
        type PageError = String;
    };
    (@body, $self:ident, $query:ident) => {{
        let response = $self.get_content($query).await?;

        match response.next_page {
            Some(page) => Ok(TurnedPage::next(
                vec![response.record],
                GetContentRequest { page },
            )),
            None => Ok(TurnedPage::last(vec![response.record])),
        }
    }};
    (async_trait) => {
        #[async_trait]
        impl PageTurner<GetContentRequest> for BlogClient {
            blogs_client_page_turner_impl!(@types);

            async fn turn_page(
                &self,
                req: GetContentRequest,
            ) -> TurnedPageResult<Self, GetContentRequest> {
                blogs_client_page_turner_impl!(@body, self, req)
            }
        }
    };
    ($($mutability:tt)*) => {
        impl PageTurner<GetContentRequest> for BlogClient {
            blogs_client_page_turner_impl!(@types);

            async fn turn_page(
                &$($mutability)* self,
                req: GetContentRequest,
            ) -> TurnedPageResult<Self, GetContentRequest> {
                blogs_client_page_turner_impl!(@body, self, req)
            }
        }
    };
}

macro_rules! blogs_client_pages_base_test {
    ($($modifier:tt)*) => {
        async {
            let mut blog = BlogClient::new(41);
            blog.set_error(0);

            let mut stream = std::pin::pin!(blog.pages(GetContentRequest { page: 0 }).items());

            let item = stream.try_next().await;
            assert_eq!(item, Err("Custom error".to_owned()));

            let item = stream.try_next().await;
            assert_eq!(item, Ok(None), "pages stream must end after an error");
        }
    };
}

macro_rules! blogs_client_pages_ahead_base_test {
    ($($modifier:tt)*) => {
        async {
            let blog = BlogClient::new(33);

            // Basic case
            let results: Vec<_> = blog
                .pages_ahead(5, Limit::None, GetContentRequest { page: 0 })
                .items()
                .try_collect()
                .await
                .unwrap();

            assert_eq!(results.len(), 33);

            for (ix, res) in results.into_iter().enumerate() {
                assert_eq!(res.0, ix);
            }

            let blog = std::sync::Arc::new(blog);

            // Pages limiting
            let results: Vec<_> = blog
                .clone()
                .into_pages_ahead(11, Limit::Pages(22), GetContentRequest { page: 0 })
                .items()
                .try_collect()
                .await
                .unwrap();

            assert_eq!(results.len(), 22);
            assert_eq!(results.last().unwrap(), &BlogRecord(21));

            // Zero corner case
            let results: Vec<_> = blog
                .pages_ahead(0, Limit::None, GetContentRequest { page: 0 })
                .items()
                .try_collect()
                .await
                .unwrap();

            assert_eq!(results.len(), 0);

            let results: Vec<_> = blog
                .clone()
                .into_pages_ahead(5, Limit::Pages(0), GetContentRequest { page: 0 })
                .items()
                .try_collect()
                .await
                .unwrap();

            assert_eq!(results.len(), 0);

            let mut blog = std::sync::Arc::into_inner(blog).unwrap();
            blog.set_error(1);

            let mut stream = std::pin::pin!(blog
                .pages_ahead(4, Limit::None, GetContentRequest { page: 0 })
                .items());

            let item = stream.try_next().await;
            assert_eq!(item.unwrap().unwrap(), BlogRecord(0));

            let item = stream.try_next().await;
            assert_eq!(item, Err("Custom error".to_owned()));

            let item = stream.try_next().await;
            assert_eq!(item, Ok(None), "pages_ahead stream must end after an error");
        }
    };
}

macro_rules! blogs_client_pages_ahead_unordered_base_test {
    ($($modifier:tt)*) => {
        async {
            // Because in tests our futures resolve immediately `page_ahead_unordered` yields results in the
            // scheduling order and is equivalent to the `page_ahead` in all aspects except of error
            // handling.
            let blog = BlogClient::new(33);

            // Basic case
            let results: Vec<_> = blog
                .pages_ahead_unordered(5, Limit::None, GetContentRequest { page: 0 })
                .items()
                .try_collect()
                .await
                .unwrap();

            assert_eq!(results.len(), 33);

            for (ix, res) in results.into_iter().enumerate() {
                assert_eq!(res.0, ix);
            }

            let blog = std::sync::Arc::new(blog);
            // Pages limiting
            let results: Vec<_> = blog
                .clone()
                .into_pages_ahead_unordered(11, Limit::Pages(22), GetContentRequest { page: 0 })
                .items()
                .try_collect()
                .await
                .unwrap();

            assert_eq!(results.len(), 22);
            assert_eq!(results.last().unwrap(), &BlogRecord(21));

            // Zero corner case
            let results: Vec<_> = blog
                .pages_ahead_unordered(0, Limit::None, GetContentRequest { page: 0 })
                .items()
                .try_collect()
                .await
                .unwrap();

            assert_eq!(results.len(), 0);

            let results: Vec<_> = blog
                .clone()
                .into_pages_ahead_unordered(5, Limit::Pages(0), GetContentRequest { page: 0 })
                .items()
                .try_collect()
                .await
                .unwrap();

            assert_eq!(results.len(), 0);

            let mut blog = std::sync::Arc::into_inner(blog).unwrap();
            // Error case
            blog.set_error_with_msg(1, "1");
            blog.set_error_with_msg(2, "2");
            blog.set_error_with_msg(3, "3");

            let mut stream = std::pin::pin!(blog
                .pages_ahead_unordered(5, Limit::None, GetContentRequest { page: 0 })
                .items());

            let item = stream.try_next().await;
            assert_eq!(item.unwrap().unwrap(), BlogRecord(0));

            let item = stream.try_next().await;
            assert_eq!(item.unwrap().unwrap(), BlogRecord(4));

            // This comes from a sliding window shift!
            let item = stream.try_next().await;
            assert_eq!(item.unwrap().unwrap(), BlogRecord(5));

            let item = stream.try_next().await;
            assert_eq!(item, Err("1".to_owned()));

            let item = stream.try_next().await;
            assert_eq!(
                item,
                Ok(None),
                "pages_ahead_unordered stream must end after an error"
            );
        }
    };
}

macro_rules! page_turner_impls {
    ($($modifier:tt)*) => {
        numbers_client_page_turner_impl!($($modifier)*);
        blogs_client_page_turner_impl!($($modifier)*);
    }
}

macro_rules! pages_base_test {
    ($($modifier:tt)*) => { async {
        numbers_client_pages_base_test!($($modifier)*).await;
        blogs_client_pages_base_test!($($modifier)*).await;
    }};
}

macro_rules! pages_ahead_base_test {
    ($($modifier:tt)*) => { async {
        blogs_client_pages_ahead_base_test!($($modifier)*).await;
    }};
}

macro_rules! pages_ahead_unordered_base_test {
    ($($modifier:tt)*) => { async {
        blogs_client_pages_ahead_unordered_base_test!($($modifier)*).await;
    }};
}

pub(crate) use blogs_client_page_turner_impl;
pub(crate) use blogs_client_pages_ahead_base_test;
pub(crate) use blogs_client_pages_ahead_unordered_base_test;
pub(crate) use blogs_client_pages_base_test;
pub(crate) use numbers_client_page_turner_impl;
pub(crate) use numbers_client_pages_base_test;
pub(crate) use page_turner_impls;
pub(crate) use pages_ahead_base_test;
pub(crate) use pages_ahead_unordered_base_test;
pub(crate) use pages_base_test;

use super::RequestAhead;
