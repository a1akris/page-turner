//! Module with page turner methods implementation.
//!
//! Most of the logic is put inside macro_rules! because the crate provides different types of page
//! turners and all of them have slightly different signatures despite function bodies and trait
//! impls are same for them.
//!
//! WARNING: All macros here are magical! They implicitly expect some defined/imported types and
//! type aliases in the calling contexts and you must read their bodies in order to understand what
//! arguments they accept. The idea is by calling a macro in a context with specific imports/type
//! aliases you generate specific for those types code. This was done as a compromise between
//! public API readbility and a code verbosity, there are just too many types involved to hide them
//! inside macros or to bother with some macro syntax to enumerate them all.
//!
//! It turned out that every page turner requires everything from this module to be fully
//! implemented so it's ok to abuse glob imports(`use internal::*;`) in page turner modules.

pub mod itertools;
pub mod pages;
pub mod pages_ahead;
pub mod pages_ahead_unordered;

pub use itertools::*;
pub use pages::PagesState;

pub(crate) use pages::request_next_page_decl;
pub(crate) use pages_ahead::{pages_ahead_state_def, request_pages_ahead_decl};
pub(crate) use pages_ahead_unordered::{
    pages_ahead_unordered_state_def, request_pages_ahead_unordered_decl,
};
