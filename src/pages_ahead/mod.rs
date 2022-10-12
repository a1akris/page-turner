//! Helpers to implement `pages_ahead` funtionality
use super::{Limit, PageTurner, RequestAhead, TurnedPage};

// TODO: The code in these submodules is copy-pasted a lot. Reorginize it to extract repetetive
// parts or use macros to get rid of duplication.
pub mod ordered;
pub mod unordered;

type RequestChunks<R> = Chunks<RequestIter<R>>;

struct RequestIter<R> {
    cur_request: Option<R>,
    limit: Limit,
    counter: usize,
}

impl<R> RequestIter<R> {
    fn new(req: R, limit: Limit) -> Self {
        Self {
            cur_request: Some(req),
            limit,
            counter: 0,
        }
    }
}

impl<R> Iterator for RequestIter<R>
where
    R: RequestAhead,
{
    type Item = R;

    fn next(&mut self) -> Option<Self::Item> {
        if let Limit::Pages(pages) = self.limit {
            if self.counter >= pages {
                return None;
            }
        }

        let next_request = self
            .cur_request
            .as_ref()
            .map(<R as RequestAhead>::next_request);

        let request_to_ret = self.cur_request.take();

        self.cur_request = next_request;
        self.counter += 1;

        request_to_ret
    }
}

trait ChunksExt: Sized {
    fn chunks(self, chunk_size: usize) -> Chunks<Self>;
}

impl<I: Iterator + Sized> ChunksExt for I {
    fn chunks(self, chunk_size: usize) -> Chunks<Self> {
        Chunks::new(self, chunk_size)
    }
}

struct Chunks<I> {
    iter: I,
    chunk_size: usize,
}

impl<I: Iterator> Chunks<I> {
    fn next_chunk(&mut self) -> Option<Chunk<'_, I>> {
        if self.chunk_size == 0 {
            None
        } else {
            self.iter.next().map(|first| Chunk::new(self, first))
        }
    }
}

impl<I> Chunks<I> {
    fn new(iter: I, chunk_size: usize) -> Self {
        Self { iter, chunk_size }
    }
}

struct Chunk<'c, I: Iterator> {
    chunks: &'c mut Chunks<I>,
    first: Option<I::Item>,
    yielded_count: usize,
}

impl<'c, I: Iterator> Chunk<'c, I> {
    fn new(chunks: &'c mut Chunks<I>, first: I::Item) -> Self {
        Self {
            chunks,
            first: Some(first),
            yielded_count: 1,
        }
    }
}

impl<'c, I: Iterator> Iterator for Chunk<'c, I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if self.yielded_count < self.chunks.chunk_size {
            match self.first.take() {
                first @ Some(_) => first,
                None => {
                    self.yielded_count += 1;
                    self.chunks.iter.next()
                }
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DumbRequest {
        page: usize,
    }

    impl Default for DumbRequest {
        fn default() -> Self {
            Self { page: 1 }
        }
    }

    impl RequestAhead for DumbRequest {
        fn next_request(&self) -> Self {
            Self {
                page: self.page + 1,
            }
        }
    }

    #[test]
    fn request_iter() {
        let last = RequestIter::new(DumbRequest::default(), Limit::None)
            .take(20)
            .last();

        assert_eq!(last.map(|req| req.page), Some(20));

        let last = RequestIter::new(DumbRequest::default(), Limit::Pages(8))
            .take(20)
            .last();

        assert_eq!(last.map(|req| req.page), Some(8));
    }

    #[test]
    fn chunks_ext() {
        let mut chunks = RequestIter::new(DumbRequest::default(), Limit::Pages(20)).chunks(4);
        let mut chunks_count = 0;
        while let Some(chunk) = chunks.next_chunk() {
            let requests: Vec<_> = chunk.collect();

            assert_eq!(requests.len(), 4);
            assert_eq!(requests.last().unwrap().page % 4, 0);

            chunks_count += 1;
        }

        assert_eq!(chunks_count, 5);

        let mut chunks = RequestIter::new(DumbRequest::default(), Limit::Pages(13)).chunks(4);

        let mut chunks_count = 0;
        while let Some(chunk) = chunks.next_chunk() {
            let requests: Vec<_> = chunk.collect();

            if chunks_count != 3 {
                assert_eq!(requests.len(), 4);
                assert_eq!(requests.last().unwrap().page % 4, 0);
            } else {
                assert_eq!(requests.len(), 1);
                assert_eq!(requests.last().unwrap().page, 13);
            }

            chunks_count += 1;
        }

        assert_eq!(chunks_count, 4);

        let mut chunks = RequestIter::new(DumbRequest::default(), Limit::None).chunks(0);
        assert!(chunks.next_chunk().is_none())
    }
}
