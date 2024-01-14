- **1.0.0:**
    - Replace `#[async_trait]` with partially stabilized `async trait` using
      RPITIT. Set MSRV to 1.75.
    - `PageTurner` doesn't require to return `Vec<PageItem>` anymore. `type
      PageItem` is renamed into `type PageItems` and a user can specify a full
      return type with it like `type PageItems = HashMap<String, Vec<Object>>`;
    - `PagesStream` is now a `Stream` extension trait and not a separate type.
    - Add extra `PageItems`, `PageError`, `PageTurnerFuture` type aliases,
      rename `PageTurnerOutput` into `TurnedPageResult`.
    - Implement more optimal sliding window request scheduling strategy for
      `pages_ahead` and `pages_ahead_unordered` streams. Details:
      <https://github.com/a1akris/page-turner/issues/2>. `into_pages_ahead` and
      `into_pages_ahead_unordered` now require `Clone` explicitly and don't use
      `Arc` under the hood.
    - Internal refactorings, module restructurings, and a huge simplification
      of internal streams implementation. All copy-paste is gone!
    - Introduce different page turner flavors with relaxed constraints behind
      feature flags for use in singlethreaded environments. `local` doesn't
      require anything to be `Send` and `mutable` allows to mutate client
      during querying. Bring back `async_trait` version of page turner behind
      the `dynamic` feature flag.
    - Extra tests that check that everything has correct constraints and is
      send/object safe where required.
    - README, CHANGELOG and documentation overhauls.

- **0.8.2:**
    - Fix typo in docs.

- **0.8.1:**
    - Bugfix in internal chunking iterator that yilded empty chunks for
      `chunk_size = 1` in previous version. (0.8.0 yanked)

- **0.8.0:**
    - Introduce [`RequestAhead`] and [`PageTurner::pages_ahead`],
      [`PageTurner::pages_ahead_unordered`] for concurrent page querying

- **0.7.0:**
    - Hotfix lifetime bounds in [`PagesStream`] for `T` and `E`. (0.6.0 yanked)

- **0.6.0:**
    - Initial public release

