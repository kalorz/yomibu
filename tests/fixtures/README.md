# Synthetic cache and API fixtures

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

## API fixtures (1b)

`wanikani/{user,assignments,review_statistics,subjects}.json` are synthetic v2
response shapes checked against the same official reference on 2026-09-27. They
represent the observations in `mixed.json`: source fields use `data_updated_at`,
`subject_type`, subscription/reading `type`, sentence `ja`/`en`, and
`spaced_repetition_system_id`. The source fixtures also contain radical progress,
a level-4 subject excluded by the level-3 access limit, and ignored preferences,
mnemonic, and audio fields. None is retained in normalized output.

Synthetic credentials exist only in test request setup, never in these fixtures.
Tests mutate copies for pagination, duplication, invalid content, and resets.
Requests go exclusively to isolated loopback mock servers, exercising the actual
HTTP adapter and cache writer. The cross-component test runs the offline status
binary with a cleared child environment after synchronization.
