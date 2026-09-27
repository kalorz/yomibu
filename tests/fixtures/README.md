# Offline cache fixtures

These are synthetic Yomibu schema-1 caches, created for milestone 1a. They contain
no account export or credentials. Resource shapes and nullable dates are grounded
in the [WaniKani v2 revision 20170710 reference](https://docs.api.wanikani.com/20170710/),
checked on 2026-09-27. Fields are normalized to `src/domain.rs`, not copied API
response envelopes. IDs, learner identity, times, and counters are invented.

- `empty.json`: valid account with no subjects or progress.
- `mixed.json`: two kanji, one vocabulary subject, one kana-only subject, and an
  explicit unavailable vocabulary subject. Includes an unstarted assignment,
  burned assignment, two SRS systems, a review-only subject, absent kana-only
  review statistics, and hidden flags in each source collection. Hidden source
  observations may differ across the synchronization interval.
- `mixed-status.txt`: deterministic CLI output for the mixed cache. Reading
  accuracy is 9/11, meaning accuracy is 10/20; neither averages per-item rates.

The tests write fixtures to isolated temporary directories as `wanikani.json`
and call the production cache reader. Application code never loads these files
as a fallback or fake source. Invalid cache cases mutate the mixed fixture only
inside individual tests.
