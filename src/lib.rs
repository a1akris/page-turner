#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("doc/Main.md")]

#[cfg(feature = "local")]
#[cfg_attr(docsrs, doc(cfg(feature = "local")))]
pub mod local;

#[cfg(feature = "mt")]
pub mod mt;

#[cfg(feature = "mutable")]
#[cfg_attr(docsrs, doc(cfg(feature = "mutable")))]
pub use local::mutable;

#[cfg(feature = "dynamic")]
#[cfg_attr(docsrs, doc(cfg(feature = "dynamic")))]
pub use mt::dynamic;

// `mt` is enabled by default so prelude reexports the mt::prelude. Users will need to specify a
// prelude module manually like `page_turner::local::prelude*` if they want to use other flavours
// of page turner.
#[cfg(feature = "mt")]
#[doc = include_str!("doc/prelude")]
pub mod prelude {
    pub use crate::mt::prelude::*;
}

// `mt` is enabled by default so it's reexported into the root.
#[cfg(feature = "mt")]
pub use crate::mt::*;

/// A struct that combines items queried for the current page and an optional request to query the
/// next page. If `next_request` is `None` `PageTurner` stops querying pages.
///
/// [`TurnedPage::next`] and [`TurnedPage::last`] constructors can be used for convenience.
pub struct TurnedPage<I, R> {
    pub items: I,
    pub next_request: Option<R>,
}

impl<I, R> TurnedPage<I, R> {
    pub fn new(items: I, next_request: Option<R>) -> Self {
        Self {
            items,
            next_request,
        }
    }

    pub fn next(items: I, next_request: R) -> Self {
        Self {
            items,
            next_request: Some(next_request),
        }
    }

    pub fn last(items: I) -> Self {
        Self {
            items,
            next_request: None,
        }
    }
}

/// If a request for the next page doesn't require any data from the response and can be made out
/// of the request for the current page implement this trait to enable `pages_ahead`,
/// `pages_ahead_unordered` families of methods that query pages concurrently.
///
/// # Caveats
///
/// - Ensure that page turner's `turn_page` returns [`TurnedPage::last`] at some point or that you
/// always use [`Limit::Pages`] in `*pages_ahead*` methods, otherwise `*pages_ahead*` streams will
/// always end with errors.
///
/// - Ensure that page turner's `turn_page` produces equivalent next requests that query the same
/// data so that `*pages_ahead*` streams and `pages` stream yield the same results.
pub trait RequestAhead {
    fn next_request(&self) -> Self;
}

/// If you use `pages_ahead` or `pages_ahead_unordered` families of methods and you know in advance
/// how many pages you need to query, specify [`Limit::Pages`] to prevent redundant querying past
/// the last existing page from being executed.
#[allow(dead_code)]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    #[default]
    None,
    Pages(usize),
}

mod internal;

#[cfg(test)]
mod test_utils;
