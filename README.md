![Build](https://github.com/a1akris/page-turner/actions/workflows/build.yml/badge.svg)

### Changelog

- **0.8.2:** Fix typo in docs.
- **0.8.1:** Bugfix in internal chunking iterator that yilded empty chunks for
  `chunk_size = 1` in previous version. (0.8.0 yanked)
- **0.8.0:** Introduce [`RequestAhead`] and [`PageTurner::pages_ahead`],
  [`PageTurner::pages_ahead_unordered`] for concurrent page querying
- **0.7.0:** Hotfix lifetime bounds in [`PagesStream`] for `T` and `E`. (0.6.0 yanked)
- **0.6.0:** Redesign, initial public release

#### License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT
license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
