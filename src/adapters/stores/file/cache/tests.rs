use super::*;

fn seeded_cache() -> (tempfile::TempDir, WaniKaniSyncData) {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("wanikani.json"),
        include_str!("../../../../../tests/fixtures/mixed.json"),
    )
    .unwrap();
    let mut next = load(dir.path()).unwrap();
    next.learner.username = "new complete sync data".into();
    (dir, next)
}

fn staging_file(dir: &Path) -> PathBuf {
    fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            !["wanikani.json", "wanikani.lock"]
                .iter()
                .any(|name| p.file_name().unwrap() == *name)
        })
        .expect("expected a staging file")
}

#[test]
fn storage_faults_report_replacement_and_preserve_a_complete_cache() {
    for (failure, remove_staging) in [
        (WriteStep::Create, false),
        (WriteStep::Encode, false),
        (WriteStep::Flush, false),
        (WriteStep::SyncFile, false),
        (WriteStep::Replace, false),
        (WriteStep::SyncDirectory, false),
        (WriteStep::Replace, true),
    ] {
        let (dir, next) = seeded_cache();
        let path = dir.path().join("wanikani.json");
        let old_bytes = fs::read(&path).unwrap();
        let old = load(dir.path()).unwrap();
        let replaced = failure == WriteStep::SyncDirectory;
        let expected = if replaced { &next } else { &old };
        let guard = SyncGuard::acquire(dir.path()).unwrap();
        let error = guard
            .replace_with(&next, |step| {
                if step != failure {
                    return Ok(());
                }
                assert_eq!(&load(dir.path()).unwrap(), expected);
                if remove_staging {
                    // Exercise a real rename failure after writing and syncing.
                    fs::remove_file(staging_file(dir.path())).unwrap();
                    return Ok(());
                }
                if step == WriteStep::Encode {
                    fs::write(staging_file(dir.path()), b"{partial staging data").unwrap();
                }
                Err(io::Error::other("injected storage failure"))
            })
            .unwrap_err();
        if replaced {
            assert!(matches!(error, WriteError::DurabilityUncertain(_)));
            assert!(error.to_string().contains("Cache was replaced"));
        } else {
            assert!(
                matches!(error, WriteError::BeforeReplacement(_)),
                "{failure:?}: {error}"
            );
            assert_eq!(fs::read(&path).unwrap(), old_bytes);
        }
        let cause = std::error::Error::source(&error)
            .and_then(|source| source.downcast_ref::<io::Error>())
            .unwrap_or_else(|| panic!("missing I/O cause at {failure:?}: {error}"));
        assert_eq!(
            cause.kind(),
            if remove_staging {
                io::ErrorKind::NotFound
            } else {
                io::ErrorKind::Other
            }
        );
        assert_eq!(&load(dir.path()).unwrap(), expected);
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            2,
            "temporary file leaked at {failure:?}"
        );
        drop(guard);
        SyncGuard::acquire(dir.path())
            .unwrap()
            .replace(&next)
            .unwrap();
        assert_eq!(load(dir.path()).unwrap(), next);
    }
}

// Re-executed only by the parent test with per-child environment overrides.
#[test]
fn writer_child() {
    use std::io::Write;
    let Some(dir) = std::env::var_os("YOMIBU_TEST_CACHE_DIR") else {
        return;
    };
    let stop = std::env::var("YOMIBU_TEST_WRITE_STEP").unwrap();
    let dir = PathBuf::from(dir);
    let mut next = load(&dir).unwrap();
    next.learner.username = "new complete sync data".into();
    let guard = SyncGuard::acquire(&dir).unwrap();
    guard
        .replace_with(&next, |step| {
            if format!("{step:?}") == stop {
                if step == WriteStep::Encode {
                    fs::write(staging_file(&dir), b"{partial staging data").unwrap();
                }
                println!("WRITE_READY");
                io::stdout().flush().unwrap();
                // Only process termination may release this checkpoint.
                loop {
                    std::thread::park();
                }
            }
            Ok(())
        })
        .unwrap();
}

struct WriterChild(std::process::Child);
impl Drop for WriterChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
impl WriterChild {
    fn at(dir: &Path, step: WriteStep) -> Self {
        use std::{
            io::BufRead,
            process::{Command, Stdio},
            sync::mpsc,
            time::Duration,
        };
        let mut child = Self(
            Command::new(std::env::current_exe().unwrap())
                .env_clear()
                .args([
                    "--exact",
                    "adapters::stores::file::cache::tests::writer_child",
                    "--nocapture",
                ])
                .env("YOMIBU_TEST_CACHE_DIR", dir)
                .env("YOMIBU_TEST_WRITE_STEP", format!("{step:?}"))
                .stdout(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let stdout = child.0.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in io::BufReader::new(stdout).lines() {
                if line.unwrap() == "WRITE_READY" {
                    let _ = tx.send(());
                    return;
                }
            }
        });
        rx.recv_timeout(Duration::from_secs(10))
            .expect("child did not reach write checkpoint");
        child
    }
}

#[test]
fn killed_writer_releases_lock_and_leaves_a_complete_old_or_new_cache() {
    use std::{
        io::Read,
        os::unix::{
            fs::{MetadataExt, PermissionsExt},
            process::ExitStatusExt,
        },
    };
    for step in [
        WriteStep::Create,
        WriteStep::Encode,
        WriteStep::Flush,
        WriteStep::SyncFile,
        WriteStep::Replace,
        WriteStep::SyncDirectory,
    ] {
        let (dir, next) = seeded_cache();
        let old = load(dir.path()).unwrap();
        let path = dir.path().join("wanikani.json");
        let old_bytes = fs::read(&path).unwrap();
        let mut opened_before_replacement = fs::File::open(&path).unwrap();
        let mut child = WriterChild::at(dir.path(), step);
        let lock_inode = fs::metadata(dir.path().join("wanikani.lock"))
            .unwrap()
            .ino();
        assert!(matches!(
            SyncGuard::acquire(dir.path()),
            Err(WriteError::Locked)
        ));
        let expected = if step == WriteStep::SyncDirectory {
            &next
        } else {
            &old
        };
        assert_eq!(&load(dir.path()).unwrap(), expected, "{step:?}");
        if !matches!(step, WriteStep::Create | WriteStep::SyncDirectory) {
            assert_eq!(
                fs::metadata(staging_file(dir.path()))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o077,
                0
            );
        }
        child.0.kill().unwrap();
        assert_eq!(
            child.0.wait().unwrap().signal(),
            Some(9),
            "expected SIGKILL"
        );
        assert_eq!(&load(dir.path()).unwrap(), expected, "{step:?}");
        let mut still_old = Vec::new();
        opened_before_replacement
            .read_to_end(&mut still_old)
            .unwrap();
        assert_eq!(still_old, old_bytes);
        let guard = SyncGuard::acquire(dir.path()).unwrap();
        assert_eq!(
            fs::metadata(dir.path().join("wanikani.lock"))
                .unwrap()
                .ino(),
            lock_inode
        );
        // Abrupt exit may leave a private staging file; it must not become a cache.
        guard.replace(&next).unwrap();
        assert_eq!(load(dir.path()).unwrap(), next);
    }
}

#[test]
fn dropping_guard_unlocks_even_with_a_duplicated_file_description() {
    let (dir, _) = seeded_cache();
    let guard = SyncGuard::acquire(dir.path()).unwrap();
    // A concurrent process spawn can briefly inherit the same open description.
    let inherited = guard.lock.try_clone().unwrap();
    drop(guard);
    let _next = SyncGuard::acquire(dir.path()).unwrap();
    drop(inherited);
    assert!(matches!(
        SyncGuard::acquire(dir.path()),
        Err(WriteError::Locked)
    ));
}
