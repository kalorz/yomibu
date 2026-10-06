# Managed Sudachi dictionaries

Yomibu supports two explicit policies with the same analyzer/configuration and
tokenization/evaluation code. Normal commands use an app-managed mapped dictionary;
`--dictionary PATH` always fully verifies the selected file and keeps owned bytes.
Neither policy loads ambient analyzer configuration, user dictionaries, dynamic
plugins or network resources.

## Install and use offline

From a repository checkout, explicitly obtain the pinned publisher bundle with
the existing developer/CI installer. It verifies the downloaded archive, dictionary
and both notices. An existing pinned ZIP can be supplied with `--archive PATH`.

```sh
python3 scripts/setup_a1_dictionary.py
cargo run --release --locked -- dictionary import --bundle target/a1/current
cargo run --release --locked --offline -- analyze --input tests/fixtures/analyze/nominal.json
cargo run --release --locked --offline -- dictionary verify
```

The Rust import operation is entirely offline and needs no Python at runtime.
Its source directory must contain regular `system_core.dic`, `LEGAL` and
`LICENSE-2.0.txt` files. It copies into private staging and fully checks the
destination lengths and SHA-256 pins before publication. It never trusts a
source path or a source receipt instead of verifying copied bytes.

The default root is `$HOME/.yomibu/dictionaries`. Use `--dictionary-dir PATH` on
import, verify and dictionary-backed commands to select another root. Missing or
empty HOME requires that explicit option. `--data-dir` remains learner storage
and does not select a dictionary. Help/version and preview commands do not open
an installation. Generation retains explicit model opt-in and existing preflight;
import/verify/analyze make no provider call.

```text
dictionaries/
  installation.lock
  current                       # bounded version-1 selection manifest
  bundles/
    core-20260723-v0-<hash>-<unique-generation>/
      system_core.dic
      LEGAL
      LICENSE-2.0.txt
      installation.json         # installation verification record
```

Roots and generation directories must be private and owned by the effective OS
user. Published bundle files are read-only, private regular files without
symlinks or additional hardlinks. Yomibu rejects unsafe storage rather than
changing existing permissions. Explicit source bundles can use a directory
symlink such as the developer installer's `current`, but source file symlinks
are rejected. These checks reduce accidental exposure; they are not proof of
immutability.

## Updates, failures and active readers

Reimporting creates a separate fully verified generation, even for the same pin.
Yomibu never truncates, modifies or removes a published generation. The dictionary
and notices are synchronized as one complete bundle before an atomic replacement
of `current`; parent-directory synchronization follows. A persistent advisory
writer lock coordinates importers and is released when its guard/process ends.
Readers do not need this lock.

Failure before changing `current` leaves the previous selection usable. A failed
directory sync after replacement reports `DurabilityUncertain`: the new complete
selection is visible, but durability was not acknowledged. Do not infer rollback
or blindly retry a download. Process interruption can leave staging or unreferenced
complete generations; loading ignores them. Tests exercise interruption and OS
fault boundaries, not power loss.

Existing analyzers retain their mapped generation after publication. New analyzers
select the new one. There is no automatic cleanup/prune command in this milestone;
retaining generations intentionally consumes additional disk space.

## File-stability contract

Full verification happens during import or explicit `dictionary verify`.
Managed startup checks bounded supported records, exact compiled pins, file
ownership/type/permissions, dictionary/notice sizes and the 272-byte dictionary
header. The system-format value is `0xce9f011a92394434`; description is `20260723`.
These are format/release checks, not a partial hash or exact verification of
current contents. The checked dictionary handle is the handle that gets mapped.

The contract requires the verified generation's dictionary bytes to remain
unchanged for the entire analyzer lifetime. Yomibu's installation/update protocol
maintains this condition. Manual editing/truncation and writes by other processes
are unsupported. A same-user or privileged writer can defeat permissions, alter
receipts and write a mapped file. Linux/macOS locks are advisory; locks, metadata
and read-only modes cannot prove immutability. Such writes can cause invalid
analysis, process faults or unsafe behavior in mmap-backed parsing.

This does not preserve the owned snapshot's protection against arbitrary external
writes. Use `--dictionary PATH` where file stability cannot be maintained. That
option remains strict even for paths inside the managed root: full length/SHA-256
verification precedes Sudachi initialization, and verified bytes are retained.
CI's real-adapter tests use this fully verified policy, as did the retired A1
runner and recorded reproducibility experiments. No timestamp hash cache,
analyzer upgrade, dictionary pruning or hash bypass is introduced.

## Provenance and Rust API

All reports preserve the exact analyzer, dictionary and embedded configuration
pins documented in [A1 implementation](A1_IMPLEMENTATION.md). Managed reports add
`analysis.provenance.dictionary_loading` with generation, `memory_mapped` storage,
`full_sha256_at_installation` verification, startup checks and the file-stability
requirement. `dictionary_sha256` identifies the expected pin; it is not a claimed
startup hash. Owned JSON retains its prior serialized shape. Linguistic judgments
and A1's frozen no-go evidence are unaffected.

Library calls use `adapters::dictionary::{import_bundle, verify,
ManagedInstallation::open}` with explicit paths. Environment/output handling stays
in the executable. `SudachiAnalyzer::load` remains safe for arbitrary external
files. `unsafe SudachiAnalyzer::load_managed` consumes the checked installation
handle: callers must ensure actual full installation verification and no writes
or truncation for the analyzer's lifetime. A receipt alone cannot establish these
obligations. Both paths use the embedded-character-definition constructor and
share the existing `analyze` and `evaluate` operations.

Reuse one analyzer per command/session. Server threads can already share one
owned analyzer; mapping can also benefit a server and lets separate processes
share/reclaim file-backed pages. Policy is determined by source trust, not process
lifetime. Peak memory depends on workloads and touched pages; full hashing of a
mapping touches the entire dictionary.

Rust notes for a Ruby developer: the analyzer owns either a `Vec<u8>` snapshot
or the mapping through the same upstream storage enum. Borrowed shared references
reuse it without copying the dictionary. RAII releases the writer lock, and typed
errors distinguish failed publication from uncertain durability. `unsafe` marks
a filesystem promise the borrow checker cannot prove. Direct dependencies on
already-locked `memmap2 0.9.11` and `libc 0.2.189` provide mapping the checked handle,
effective-user ownership and no-follow/nonblocking opens; no package versions or
features changed.
