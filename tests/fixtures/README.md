# Cache and API test data

These fixtures contain invented learner data. They include no account exports or
credentials. Cache files use Yomibu's schema 1 and its
[source data types](../../crates/yomibu-core/src/domain/knowledge/source.rs).
The API structures and dates that can be null follow the
[WaniKani v2 revision 20170710 reference](https://docs.api.wanikani.com/20170710/),
checked on 2026-09-27.

## Cache fixtures

| File | Purpose |
| --- | --- |
| `empty.json` | A valid account with no subjects or progress. |
| `mixed.json` | Kanji and vocabulary with varied progress, hidden or unavailable subjects, and missing optional data. |
| `preparation.json` | A copy of `mixed.json` with assignment 102 made visible, allowing the practice target `一つ:ひとつ:one thing`. |
| `mixed-status.txt` | The expected sync summary. Reading accuracy is 9/11 and meaning accuracy is 10/20, calculated from totals rather than average item rates. |

Tests copy the cache fixtures into separate temporary directories as `wanikani.json`
and use the production cache reader. Tests change their own copies to create
invalid inputs. The application never uses these fixtures as fallback data.

## API fixtures

The `wanikani/` JSON files represent API responses for the observations in
`mixed.json`. They also include data that the importer excludes: radical progress,
a level-4 subject beyond the account's level-3 access limit, and unused fields.
Source observations collected during sync can have different hidden flags.

Tests modify copies to cover pagination, duplicates, invalid content and account
resets. Requests go only to local mock servers. Tests create their own fake
credentials, and CLI sync tests set the child process's environment explicitly.

Failure tests simulate malformed or slow HTTP responses, inject storage errors,
and stop child processes at known write stages. Files and process controls are
isolated per test. These controls are not part of the CLI or saved cache format.
