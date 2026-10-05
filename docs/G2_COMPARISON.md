# G2 v1/v2 comparison — prepared, not run

This is a small descriptive experiment using public synthetic data. No paid call
has run and no improvement in adherence, naturalness or comprehension is claimed.
Passing request snapshots and mocked responses establish engineering contracts only.
The user's task explicitly requires approval before these paid OpenAI calls.
Credentials and usable network access are also prerequisites; never print a key.

The [manifest](../tests/fixtures/focused/comparison/manifest.json) fixes the code,
inputs, exact request files, byte lengths, SHA-256 hashes and execution order:

- Baseline: `818dda5e6897e4d8ab729ed9198e5070d16af50b`, merged PR #9 with
  `562cc11`; prompt `g2-focused-sentence-v1`.
- Current generation code: `6c5e1343ca6583fc76211c6a757ec0070f5e7325`; prompt
  `g2-focused-sentence-v2`. Later comparison/documentation commits do not alter
  this production code. Record the delivery commit alongside this code pin.
- Both use `g2-situations-v1`, the real pinned Sudachi adapter/dictionary and the
  original full vocabulary/grammar for evaluation. No private or held-out evidence.

| Input / focus entry 1 | Transmitted selected entries | Locally permitted, unselected |
| --- | --- | --- |
| `pet-rest.json` / 寝る | 寝る, 猫 | 犬, 歩く |
| `pet-walk.json` / 歩く | 歩く, 猫 | 犬, 寝る |
| `book-reading.json` / 読む | 読む, 学生, 本 | 先生, 猫 |

These are exact tuples, including readings, senses and direct-object metadata,
not spelling-only membership. Each input is byte-identical between versions.
Offline execution of both pinned binaries confirmed ready contexts and identical
user messages, schema and provider settings: only developer prompt text differs.
All six stored requests have no trailing newline; Python and `wc`/`sha256sum`
independently agree on length/hash. The manifest supplies full hashes.

| Situation | v1 bytes | v2 bytes |
| --- | ---: | ---: |
| pet-rest | 1,882 | 2,312 |
| pet-walk | 1,889 | 2,319 |
| book-reading | 2,104 | 2,534 |

## Fixed order and budget

Run each row once, sequentially. Do not reorder, cherry-pick, retry, repair or
replace failed/missing runs. A failure consumes its scheduled slot. An interruption
leaves subsequent rows not run; do not restart the experiment for a better result.

| Attempts | Input | Repetition | Version order |
| --- | --- | ---: | --- |
| 1, 2 | pet-rest | 1 | v1, v2 |
| 3, 4 | pet-walk | 1 | v1, v2 |
| 5, 6 | book-reading | 1 | v1, v2 |
| 7, 8 | pet-rest | 2 | v1, v2 |
| 9, 10 | pet-walk | 2 | v1, v2 |
| 11, 12 | book-reading | 2 | v1, v2 |

Maximum **12 provider attempts**, up to **24 candidates**, **1,024 output tokens
per attempt / 12,288 total**. Keep Responses `gpt-6-luna`, Standard/default tier,
reasoning `none`, exact strict two-string schema, tools/caching/storage/deadline/
byte limits and all other provider settings unchanged. No seed/temperature override,
SDK, production prompt switch, critic, ranking or acceptance logic is introduced.

Refreshed 2026-10-05 from official [GPT-6 Luna pricing](https://developers.openai.com/api/docs/models/gpt-6-luna)
and [API pricing](https://developers.openai.com/api/docs/pricing): Standard input
US$0.10/million and output US$0.50/million tokens. Using the conservative planning
allowance of 32,768 input plus the 1,024 output limit gives US$0.0037888 per
attempt, **US$0.0454656 total before tax**; propose a **US$0.05 allowance before
tax**. Input tokens are estimated, not tokenizer-enforced; these small fixed
bodies do not prove actual billing. No cache discount is assumed. Timeout or
cancellation can leave completion/billing uncertain; coding-agent usage is separate.
Recheck official pricing if execution is deferred. This document authorizes no call.

## Repeat offline preparation

Run from the delivery checkout with Git history available. These commands build
two isolated pinned checkouts using their Rust 1.98.1 toolchains. They preserve
existing worktrees and ignored artifacts. Keep the same shell for all blocks.
Results, sanitized previews/requests and provenance will stay locally under the
ignored `target/g2-comparison/2026-10-05-v1-v2/`; nothing is published automatically.
The dictionary setup is the existing documented checksum-verified operation.

```sh
export YOMIBU_COMPARISON_REPO="$PWD"
export YOMIBU_COMPARISON_WORK="$(mktemp -d "${TMPDIR:-/tmp}/yomibu-g2-comparison.XXXXXX")"
export YOMIBU_COMPARISON_RESULTS="$PWD/target/g2-comparison/2026-10-05-v1-v2"
export YOMIBU_COMPARISON_DICTIONARY="$PWD/target/a1/current/system_core.dic"
python3 scripts/setup_a1_dictionary.py

git worktree add --detach "$YOMIBU_COMPARISON_WORK/v1" 818dda5e6897e4d8ab729ed9198e5070d16af50b
git worktree add --detach "$YOMIBU_COMPARISON_WORK/v2" 6c5e1343ca6583fc76211c6a757ec0070f5e7325
(cd "$YOMIBU_COMPARISON_WORK/v1" && CARGO_TARGET_DIR="$YOMIBU_COMPARISON_WORK/build-v1" cargo build --locked --release --bin yomibu)
(cd "$YOMIBU_COMPARISON_WORK/v2" && CARGO_TARGET_DIR="$YOMIBU_COMPARISON_WORK/build-v2" cargo build --locked --release --bin yomibu)
```

Verify every request without sending anything. This also preserves binary hashes,
delivery/code commits, fixed inputs and exact requests beside the future results.
It refuses to alter an experiment that has already started live execution.

```sh
python3 - <<'PY'
import hashlib, json, os, shutil, subprocess
from pathlib import Path
repo = Path(os.environ['YOMIBU_COMPARISON_REPO'])
work = Path(os.environ['YOMIBU_COMPARISON_WORK'])
results = Path(os.environ['YOMIBU_COMPARISON_RESULTS'])
fixtures = repo / 'tests/fixtures/focused/comparison'
manifest = json.loads((fixtures / 'manifest.json').read_bytes())
assert not (results / 'live-started').exists()
results.mkdir(parents=True, exist_ok=True)
digest = lambda data: hashlib.sha256(data).hexdigest()
binaries = {}
for version, info in manifest['versions'].items():
    checkout = work / version
    assert subprocess.check_output(['git', '-C', str(checkout), 'rev-parse', 'HEAD'], text=True).strip() == info['code_commit']
    assert not subprocess.check_output(['git', '-C', str(checkout), 'status', '--porcelain'])
    binary = work / ('build-' + version) / 'release/yomibu'
    binaries[version] = {'path': str(binary), 'sha256': digest(binary.read_bytes())}
    for situation, request in info['requests'].items():
        input_info = manifest['inputs'][situation]
        permissions = fixtures / input_info['path']
        assert digest(permissions.read_bytes()) == input_info['sha256']
        preview = subprocess.check_output([str(binary), 'context-preview', '--permissions', str(permissions), '--focus-entry', '1', '--json'])
        report = json.loads(preview)
        body = report['request']['body_utf8'].encode('utf-8')
        assert report['status'] == 'ready' and report['situation']['id'] == situation
        assert body == (fixtures / request['path']).read_bytes()
        assert len(body) == report['request']['bytes'] == request['bytes']
        assert digest(body) == report['request']['sha256'] == request['sha256']
        (results / (situation + '-' + version + '-preview.json')).write_bytes(preview)
for path in fixtures.glob('*.json'):
    shutil.copyfile(path, results / path.name)
provenance = {'delivery_commit': subprocess.check_output(['git', '-C', str(repo), 'rev-parse', 'HEAD'], text=True).strip(), 'binaries': binaries}
(results / 'preparation.json').write_text(json.dumps(provenance, indent=2) + '\n')
print('Offline requests verified; no provider calls made.')
PY
```

## Live execution — explicit authorization required

Only run this block after the user authorizes the bounded comparison and the
approved execution environment supplies `OPENAI_API_KEY` and permitted network
access. Preserve production transport settings; do not bypass environment policy.
Do not install credentials, expose them in commands/output, or enable shell tracing.
The one-time marker prevents restarting this result directory. Spawn failures,
preflight failures and provider failures are retained without replacement calls.

```sh
python3 - <<'PY'
import datetime, hashlib, json, os, subprocess
from pathlib import Path
results = Path(os.environ['YOMIBU_COMPARISON_RESULTS'])
manifest = json.loads((results / 'manifest.json').read_bytes())
preparation = json.loads((results / 'preparation.json').read_bytes())
digest = lambda data: hashlib.sha256(data).hexdigest()
expected = [(v, s, r) for r in (1, 2) for s in ('pet-rest', 'pet-walk', 'book-reading') for v in ('v1', 'v2')]
assert [(row['version'], row['situation'], row['repetition']) for row in manifest['order']] == expected
assert len(expected) == manifest['max_attempts'] == 12
assert os.environ.get('OPENAI_API_KEY', '').strip(), 'Credential unavailable; no experiment started'
with (results / 'live-started').open('x') as marker:
    marker.write(datetime.datetime.now(datetime.timezone.utc).isoformat() + '\n')
for row in manifest['order']:
    stem = results / ('%02d-%s-%s-r%d' % (row['attempt'], row['version'], row['situation'], row['repetition']))
    record = dict(row, started_at=datetime.datetime.now(datetime.timezone.utc).isoformat())
    stem.with_suffix('.execution.json').write_text(json.dumps(record, indent=2) + '\n')
    try:
        binary = preparation['binaries'][row['version']]
        permissions = manifest['inputs'][row['situation']]
        assert digest(Path(binary['path']).read_bytes()) == binary['sha256']
        assert digest((results / permissions['path']).read_bytes()) == permissions['sha256']
        output = subprocess.run([binary['path'], 'generate-focused', '--permissions', str(results / permissions['path']), '--focus-entry', '1', '--dictionary', os.environ['YOMIBU_COMPARISON_DICTIONARY'], '--allow-model-call', '--json'], capture_output=True)
        stem.with_suffix('.stdout.json').write_bytes(output.stdout)
        stem.with_suffix('.stderr.txt').write_bytes(output.stderr)
        record['exit_code'] = output.returncode
    except (OSError, AssertionError) as error:
        record['execution_error'] = type(error).__name__
    record['finished_at'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    stem.with_suffix('.execution.json').write_text(json.dumps(record, indent=2) + '\n')
PY
```

Retain every scheduled row, including missing/interrupted results. The CLI sends
its immutable prepared request once and runs real pinned analysis/full-permission
evaluation. In each available report, check `context.request.body_utf8`, `bytes`
and `sha256` against the manifest and `generation.request_*`; a discrepancy is an
integrity limitation, not a compliant result. Preserve requested/returned model
and tier, prompt revision, response/request IDs, request count and nullable usage
from `generation`, both unchanged texts, analyzer/dictionary/configuration
provenance and all findings/spans. Null or absent provenance stays unknown.
A provider failure may have no report/ID/usage; never invent it or infer no billing.

## Describe results without collapsing dimensions

Write `summary.md` beside the raw result files. Keep one row per returned candidate
with attempt/version/input/repetition/candidate index and exact text. List attempt
and candidate execution errors, interrupted/not-run slots and missing reports
separately from completed assessments; CLI exit 0 does not mean linguistic Pass.
Report denominators (scheduled attempts, returned candidates, completed assessments)
for each version. Preserve all Fail/Inconclusive/NotRun evidence, not just examples.

| Dimension | Evidence and reporting rule |
| --- | --- |
| Selected-vocabulary departures | `context_usage.units`: separately count/report `unselected_permission_evidence` and `no_permission_entry` with original spans and identities. `unresolved` is unknown, not compliant. A permitted unselected word is only a context departure. |
| Full vocabulary permissions | `assessment.evaluation.vocabulary`: Pass/Fail/Inconclusive per completed candidate, every reason/span. A `no_permission_entry` observation does not substitute for the evaluator's judgment. |
| Grammar | Report Inflection, Particles, Nominal and Scope individually as Pass/Fail/Inconclusive (or NotRun), with coverage/findings. Keep book-reading's unresolved object/predicate safeguard visible. No invented combined grammar score. |
| Focus occurrence | `focus_occurrence.status`, `count`, `completeness`, occurrences/spans and uncertainties separately. Observed plus partial completeness stays partial. Contextual reading/sense are unassessed. |
| Exact duplicate candidates | Compare decoded candidate strings for exact equality, without trimming/normalization. Report within-attempt pairs and repeated texts across repetitions separately per version/input; identify occurrences by attempt/index. Duplicates are allowed, not automatic failures. Missing texts cannot be duplicates. |
| Unresolved lexical evidence | Retain `unresolved` units/reasons, focus uncertainties and evaluator lexical Inconclusive findings with spans. Do not count unresolved evidence as compliance or missing results as success. |

Use the existing real reports' whole-unit identities, readings and morphology.
Never use raw substring matching to infer membership. A manual summary may count
these explicit report states; it must not reinterpret them as lexical validation.
Report mixed or unchanged results honestly, including failures, duplicates and
unresolved cases. Twelve attempts are descriptive evidence only; they cannot
prove improved reading comprehension, contextual sense, naturalness or enjoyment.
