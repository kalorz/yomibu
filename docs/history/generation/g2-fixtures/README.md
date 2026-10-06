# G2 fixtures

Archived engineering evidence for the retired implementation; see
[generation history](../../../GENERATION_HISTORY.md).

`pet-rest.json` is original synthetic permission data, dedicated to the public
domain under CC0-1.0. It grants only explicit tuples/bindings; it is not learner
progress or verified language evidence. The request snapshot records compiled
prompt/settings plus selected tuples and complete grammar. No secret, private
corpus, source learner record or held-out material is present.

These fixtures and generated synthetic inventories test engineering contracts,
not linguistic quality, reading comprehension or A1 reference accuracy.

`pet-rest-request.json` is the canonical `g2-focused-sentence-v2` request, exactly
2,312 UTF-8 bytes (no trailing newline), SHA-256
`a3cbfc09ec6cb2b1264737a0ba97e90367644e91b151a0689f6276d2c4b43422`.
`pet-rest-request-v1.json` preserves the original `g2-focused-sentence-v1` bytes:
1,882 bytes, SHA-256
`81415d76fb7f43ba4cc435bc7d98afbe29abdff6cd33d7104b62ad14c63cb503`.
Both lengths/hashes were independently checked with Python and `wc`/`sha256sum`.
Passing snapshots and scripted responses establish request/report contracts only.
The public synthetic comparison inputs and requests live in `comparison/`; see
[the comparison recipe](../../../G2_COMPARISON.md).
