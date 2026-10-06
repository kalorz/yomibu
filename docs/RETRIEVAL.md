# Retrieval implementation and evidence

`ports::Embedder` accepts explicit document/query inputs and returns vectors.
`HttpEmbedder` supports an OpenAI-compatible numeric loopback server and the
hosted OpenAI embeddings endpoint. `LexicalEmbedder` is an explicit nonsemantic
comparison baseline. No default dense model is selected.

The cache records provider, model, revision, dimensions and encoding revision.
SHA-256 input keys distinguish document and query purpose and hash exact encoded
text. Changing model identity or input text invalidates reuse. Vectors must have
finite components, correct dimensions and nonzero magnitude; indexed HTTP results
must be complete, unique and from the requested model. Plain encoding sends the
same document/query format to either HTTP backend; models needing instruction
prefixes or another protocol are not implicitly supported. Model revisions are
caller assertions: configure/pin the server accordingly. A hosted alias is not
proof that the provider keeps weights immutable.

Flat cosine ranking uses f64 accumulation and stable ID ties. Every explicit
vocabulary target is retained, irrespective of similarity. No vector database,
plugin registry, server launcher or new dependency is introduced. Only explicit
retrieval preparation performs I/O; library selection/preparation are synchronous.

The CLI batches at most 32 inputs, caches only a complete successful result, and
reuses compatible cached documents. A failed batch leaves the prior file intact.
A temporary file is synchronized before atomic replacement; a directory-sync
failure reports uncertain durability after publication. Concurrent independent
writers may replace each other's additional cached entries; they cannot publish
half a JSON file. A later attempt can explicitly recompute missing entries.

HTTP transport has 5-second connection and 30-second total deadlines, no retries,
redirects or ambient proxies, a 512 KiB request cap and 4 MiB response cap. A batch
allows at most 64 nonblank inputs, each 32 KiB; cache vectors have at most 4,096
dimensions and 20,000 entries. Errors expose status/category, not provider bodies
or credentials. Missing vectors never cause an implicit hosted request.

## Reproducible small probe

The public synthetic [cases](../tests/fixtures/story/retrieval-cases.json) have
10 vocabulary documents and six briefs, with labeled relevant IDs and no forced
targets. Run the same inputs for each backend, independently of generation:

```sh
cargo build --locked
python3 scripts/compare_story_retrieval.py target/debug/yomibu \
  --embedding-provider lexical-baseline

# Substitute an actually installed, pinned OpenAI-compatible local model:
python3 scripts/compare_story_retrieval.py target/debug/yomibu \
  --embedding-provider local --embedding-endpoint http://127.0.0.1:11434/v1/ \
  --embedding-model MODEL --embedding-revision REVISION --embedding-dimensions DIMENSIONS

# Explicit paid/data-transmitting comparison, with a configured environment key:
python3 scripts/compare_story_retrieval.py target/debug/yomibu \
  --embedding-provider openai --embedding-model MODEL \
  --embedding-revision REVISION --embedding-dimensions DIMENSIONS --allow-embedding-call
```

The script fails if the requested encoder is unavailable; it never substitutes
another one. It uses a fresh temporary cache per run, reusing documents between
briefs. Reported preparation times include CLI startup/cache reads and writes;
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
