# Historical retrieval probe

Retrieval is retired. Reproduce this probe with the [pinned code](https://github.com/kalorz/yomibu/blob/7239de7517ab5be23193030b6d12f64a1f57fac3/scripts/compare_story_retrieval.py).

The public synthetic [cases](https://github.com/kalorz/yomibu/blob/7239de7517ab5be23193030b6d12f64a1f57fac3/tests/fixtures/story/retrieval-cases.json) have
10 vocabulary documents and six topics, with labeled relevant IDs and no forced
targets. Use a checkout of commit `7239de7517ab5be23193030b6d12f64a1f57fac3`
and follow its [reproduction instructions](https://github.com/kalorz/yomibu/blob/7239de7517ab5be23193030b6d12f64a1f57fac3/docs/RETRIEVAL.md).

The script fails if the requested encoder is unavailable; it never substitutes
another one. It uses a fresh temporary cache per run, reusing documents between
topics. Reported preparation times include CLI startup/cache reads and writes;
the first case prepares documents and query, later cases prepare only queries.
They are exploratory timings, not controlled embedding-only latency measurements.

Observed on 2026-10-06, Linux/x86_64, debug executable:

| Brief | Lexical baseline top 3 | Recall@3 |
| --- | --- | --- |
| A cat sleeping | cat, dog, eat | 0.50 |
| Meeting someone at the train station | station, cat, dog | 0.50 |
| Watering flowers after the rain | rain, cat, dog | 0.33 |
| Reading while eating | cat, dog, eat | 0.50 |
| 猫が寝る | cat, dog, eat | 0.50 |
| A canine having a nap | cat, dog, eat | 0.50 |

Mean recall@3: **0.4722**, with preparation taking about 11–24 ms per case in
this single run. Identity: `local-baseline / lexical-hash / 1 / 512 / tokens-v1`.
This exposes lack of stemming, synonyms and Japanese segmentation; many results
come from zero-score ID ties, not meaningful similarity. The labels/corpus are
small engineering examples, not a validated Japanese retrieval benchmark.

**Local dense and hosted comparisons are not run.** The configured local endpoint
had no service and `OPENAI_API_KEY` was absent. HTTP adapter tests use loopback
mock vector responses to verify protocol/error behavior; they provide no semantic
quality evidence. Real dense-model quality, latency and hosted cost remain an
explicit follow-up before selecting a default. No live generation call was made.
