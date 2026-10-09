use super::*;
use std::os::unix::fs::{PermissionsExt, symlink};

struct Fixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
    source: PathBuf,
    files: Vec<Artifact>,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("managed");
        let source = directory.path().join("source");
        fs::create_dir(&source).unwrap();
        let files = ["system_core.dic", "LEGAL", "LICENSE-2.0.txt"]
            .into_iter()
            .map(|name| {
                let data = format!("synthetic {name}");
                fs::write(source.join(name), &data).unwrap();
                Artifact {
                    name: name.into(),
                    bytes: data.len() as u64,
                    sha256: format!("{:x}", Sha256::digest(data.as_bytes())),
                }
            })
            .collect();
        Self {
            _directory: directory,
            root,
            source,
            files,
        }
    }

    fn import(&self) -> String {
        import_with(&self.root, &self.source, &self.files, |_| Ok(())).unwrap()
    }

    fn current(&self) -> String {
        read_manifest::<Current>(&self.root.join("current"))
            .unwrap()
            .generation
    }

    fn verify_generation(&self, generation: &str) {
        let bundle = self.root.join("bundles").join(generation);
        for pin in &self.files {
            let mut file = File::open(bundle.join(&pin.name)).unwrap();
            verify_file(&mut file, pin).unwrap();
        }
    }
}

#[test]
fn failed_copy_sync_and_publication_preserve_the_previous_complete_generation() {
    for boundary in [
        ImportStep::SyncAncestors,
        ImportStep::Copy,
        ImportStep::SyncFile,
        ImportStep::PublishBundle,
        ImportStep::PublishCurrent,
    ] {
        let fixture = Fixture::new();
        let old = fixture.import();
        let pointer = fs::read(fixture.root.join("current")).unwrap();
        let result = import_with(&fixture.root, &fixture.source, &fixture.files, |step| {
            if step == boundary {
                Err(io::Error::other("injected pre-publication failure"))
            } else {
                Ok(())
            }
        });
        assert!(result.is_err());
        assert_eq!(fs::read(fixture.root.join("current")).unwrap(), pointer);
        fixture.verify_generation(&old);
        assert_ne!(fixture.import(), old);
    }
}

#[test]
fn invalid_copied_bytes_or_missing_notices_preserve_current() {
    for name in ["system_core.dic", "LEGAL", "LICENSE-2.0.txt"] {
        let fixture = Fixture::new();
        fixture.import();
        let pointer = fs::read(fixture.root.join("current")).unwrap();
        let path = fixture.source.join(name);
        let length = fs::metadata(&path).unwrap().len() as usize;
        fs::write(&path, vec![b'x'; length]).unwrap();
        assert!(matches!(
            import_with(&fixture.root, &fixture.source, &fixture.files, |_| Ok(())),
            Err(InstallationError::Mismatch(_))
        ));
        fs::remove_file(path).unwrap();
        assert!(import_with(&fixture.root, &fixture.source, &fixture.files, |_| Ok(())).is_err());
        assert_eq!(fs::read(fixture.root.join("current")).unwrap(), pointer);
    }
}

#[test]
fn final_sync_failure_reports_uncertain_durability_with_a_complete_new_generation() {
    let fixture = Fixture::new();
    let old = fixture.import();
    let error = import_with(&fixture.root, &fixture.source, &fixture.files, |step| {
        if step == ImportStep::SyncRoot {
            Err(io::Error::other("injected final sync failure"))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert!(
        error.to_string().contains("durability is uncertain"),
        "{error}"
    );
    let new = fixture.current();
    assert_ne!(new, old);
    for generation in [old, new] {
        fixture.verify_generation(&generation);
    }
}

#[test]
fn another_importer_holding_the_lock_prevents_publication() {
    let fixture = Fixture::new();
    fixture.import();
    let pointer = fs::read(fixture.root.join("current")).unwrap();
    let lock = File::create(fixture.root.join("installation.lock")).unwrap();
    lock.try_lock().unwrap();
    let result = import_with(&fixture.root, &fixture.source, &fixture.files, |_| Ok(()));
    assert!(
        result.is_err(),
        "a second importer published while the lock was held"
    );
    assert_eq!(fs::read(fixture.root.join("current")).unwrap(), pointer);
    lock.unlock().unwrap();
    fixture.import();
}

// A sparse candidate exercises cheap-check rejection only. It is never mapped
// or treated as checksum-verified analyzer input.
fn candidate() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("managed");
    let generation = format!("core-20260723-v0-{DICTIONARY_SHA256}-metadata-test");
    let bundle = root.join("bundles").join(&generation);
    fs::create_dir_all(&bundle).unwrap();
    for path in [&root, &root.join("bundles"), &bundle] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    fs::write(
        root.join("current"),
        serde_json::to_vec(&Current {
            version: 1,
            generation,
        })
        .unwrap(),
    )
    .unwrap();
    fs::set_permissions(root.join("current"), fs::Permissions::from_mode(0o600)).unwrap();
    let mut header = [0; 272];
    header[..8].copy_from_slice(&0xce9f011a92394434_u64.to_le_bytes());
    header[16..24].copy_from_slice(b"20260723");
    let mut file = File::create(bundle.join("system_core.dic")).unwrap();
    file.set_len(DICTIONARY_BYTES).unwrap();
    file.write_all(&header).unwrap();
    file.set_permissions(fs::Permissions::from_mode(0o400))
        .unwrap();
    for pin in pins()
        .into_iter()
        .filter(|pin| pin.name != "system_core.dic")
    {
        let file = File::create(bundle.join(&pin.name)).unwrap();
        file.set_len(pin.bytes).unwrap();
        file.set_permissions(fs::Permissions::from_mode(0o400))
            .unwrap();
    }
    fs::write(
        bundle.join("installation.json"),
        serde_json::to_vec(&Receipt {
            version: RECEIPT_VERSION,
            dictionary_version: DICTIONARY_VERSION.into(),
            files: pins(),
            dictionary_fingerprint: FileFingerprint::from(&file.metadata().unwrap()),
        })
        .unwrap(),
    )
    .unwrap();
    fs::set_permissions(
        bundle.join("installation.json"),
        fs::Permissions::from_mode(0o400),
    )
    .unwrap();
    (directory, root, bundle)
}

fn revise_receipt(bundle: &Path, revise: impl FnOnce(&mut Receipt)) {
    let path = bundle.join("installation.json");
    let mut receipt: Receipt = read_manifest_file(&path, true).unwrap();
    revise(&mut receipt);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(&path, serde_json::to_vec(&receipt).unwrap()).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
}

#[test]
fn startup_rejects_subsecond_metadata_mismatches_without_relying_on_clock_advancement() {
    for mtime in [true, false] {
        let (_directory, root, bundle) = candidate();
        drop(ManagedInstallation::open(&root).unwrap());
        revise_receipt(&bundle, |receipt| {
            let nanoseconds = if mtime {
                &mut receipt.dictionary_fingerprint.mtime_nanoseconds
            } else {
                &mut receipt.dictionary_fingerprint.ctime_nanoseconds
            };
            *nanoseconds ^= 1;
        });
        assert!(matches!(
            ManagedInstallation::open(&root),
            Err(InstallationError::Changed)
        ));
    }
}

#[test]
fn replacement_truncation_and_touch_are_rejected_before_header_parsing() {
    for change in ["replacement", "truncation", "touch"] {
        let (directory, root, bundle) = candidate();
        drop(ManagedInstallation::open(&root).unwrap());
        let path = bundle.join("system_core.dic");
        match change {
            "replacement" => {
                let replacement = directory.path().join("replacement.dic");
                let mut header = [0; Header::STORAGE_SIZE];
                File::open(&path).unwrap().read_exact(&mut header).unwrap();
                let mut file = File::create(&replacement).unwrap();
                file.set_len(DICTIONARY_BYTES).unwrap();
                file.write_all(&header).unwrap();
                file.set_permissions(fs::Permissions::from_mode(0o400))
                    .unwrap();
                fs::rename(replacement, &path).unwrap();
            }
            "truncation" => {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
                fs::OpenOptions::new()
                    .write(true)
                    .open(&path)
                    .unwrap()
                    .set_len(8)
                    .unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
            }
            _ => File::open(&path)
                .unwrap()
                .set_times(fs::FileTimes::new().set_modified(std::time::SystemTime::UNIX_EPOCH))
                .unwrap(),
        }
        assert!(
            matches!(
                ManagedInstallation::open(&root),
                Err(InstallationError::Changed)
            ),
            "accepted {change}"
        );
    }
}

#[test]
fn incorrect_header_format_or_release_description_is_rejected_at_startup() {
    use std::io::{Seek, SeekFrom};
    for (offset, bytes) in [
        (0, 0x7366d3f18bd111e7_u64.to_le_bytes()),
        (16, *b"20260101"),
    ] {
        let (_directory, root, bundle) = candidate();
        fs::set_permissions(
            bundle.join("system_core.dic"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(bundle.join("system_core.dic"))
            .unwrap();
        file.seek(SeekFrom::Start(offset)).unwrap();
        file.write_all(&bytes).unwrap();
        fs::set_permissions(
            bundle.join("system_core.dic"),
            fs::Permissions::from_mode(0o400),
        )
        .unwrap();
        revise_receipt(&bundle, |receipt| {
            receipt.dictionary_fingerprint = FileFingerprint::from(&file.metadata().unwrap());
        });
        assert!(
            matches!(
                ManagedInstallation::open(&root),
                Err(InstallationError::Invalid)
            ),
            "startup accepted a wrong header"
        );
    }
}

#[test]
fn managed_loading_rejects_symlinked_directories_and_files() {
    for relative in [
        "",
        "bundles",
        "current",
        "bundle",
        "installation.json",
        "system_core.dic",
    ] {
        let (directory, root, bundle) = candidate();
        let path = match relative {
            "" => root.clone(),
            "bundles" | "current" => root.join(relative),
            "bundle" => bundle.clone(),
            name => bundle.join(name),
        };
        let relocated = directory.path().join("relocated");
        fs::rename(&path, &relocated).unwrap();
        symlink(&relocated, &path).unwrap();
        let error = ManagedInstallation::open(&root)
            .err()
            .unwrap_or_else(|| panic!("accepted symlink at {relative:?}"));
        if relative == "system_core.dic" {
            assert!(matches!(error, InstallationError::Io(_)), "{error}");
        }
    }
}

#[test]
fn writable_or_hardlinked_published_files_and_shared_roots_are_rejected() {
    for variant in ["root", "dictionary", "hardlink"] {
        let (directory, root, bundle) = candidate();
        match variant {
            "root" => fs::set_permissions(&root, fs::Permissions::from_mode(0o777)).unwrap(),
            "dictionary" => fs::set_permissions(
                bundle.join("system_core.dic"),
                fs::Permissions::from_mode(0o600),
            )
            .unwrap(),
            _ => fs::hard_link(
                bundle.join("system_core.dic"),
                directory.path().join("alias.dic"),
            )
            .unwrap(),
        }
        let error = ManagedInstallation::open(&root)
            .err()
            .expect("unsafe installation accepted");
        if variant == "root" {
            assert!(
                matches!(error, InstallationError::UnsafeStorage { .. }),
                "{error}"
            );
        } else {
            assert!(
                matches!(error, InstallationError::Invalid),
                "wrong rejection for {variant}: {error}"
            );
        }
    }
}

#[test]
fn unsafe_import_storage_identifies_the_path_and_preserves_current() {
    for change in ["root", "permissions", "hardlink"] {
        let fixture = Fixture::new();
        let old = fixture.import();
        let pointer = fs::read(fixture.root.join("current")).unwrap();
        let path = if change == "root" {
            fixture.root.clone()
        } else {
            fixture.root.join("installation.lock")
        };
        match change {
            "root" => fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap(),
            "permissions" => fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap(),
            _ => fs::hard_link(&path, fixture._directory.path().join("extra-link")).unwrap(),
        }
        let error =
            import_with(&fixture.root, &fixture.source, &fixture.files, |_| Ok(())).unwrap_err();
        let message = error.to_string();
        assert!(message.contains(path.to_str().unwrap()), "{message}");
        assert!(
            message.contains("ownership")
                && message.contains("permissions")
                && message.contains("links"),
            "{message}"
        );
        assert!(message.contains("new --dictionary-dir"), "{message}");
        assert_eq!(fs::read(fixture.root.join("current")).unwrap(), pointer);
        fixture.verify_generation(&old);
    }
}

#[test]
fn installation_record_write_errors_retain_the_io_cause_without_blind_retry_advice() {
    let fixture = Fixture::new();
    let mut read_only = File::open(fixture.source.join("LEGAL")).unwrap();
    let cause = serde_json::to_writer(
        &mut read_only,
        &Current {
            version: RECEIPT_VERSION,
            generation: "unused".into(),
        },
    )
    .unwrap_err();
    assert!(cause.is_io());
    let cause_text = cause.to_string();
    let error: InstallationError = cause.into();
    let message = error.to_string();
    assert!(
        message.starts_with("Dictionary installation record error:"),
        "{message}"
    );
    assert!(message.contains(&cause_text), "{message}");
    assert!(!message.contains("reimport"), "{message}");
}

#[test]
fn import_refuses_a_shared_or_symlinked_root_without_changing_it() {
    for symlinked in [false, true] {
        let fixture = Fixture::new();
        fs::create_dir(&fixture.root).unwrap();
        if symlinked {
            let target = fixture._directory.path().join("actual");
            fs::rename(&fixture.root, &target).unwrap();
            symlink(target, &fixture.root).unwrap();
        } else {
            fs::set_permissions(&fixture.root, fs::Permissions::from_mode(0o777)).unwrap();
        }
        assert!(import_with(&fixture.root, &fixture.source, &fixture.files, |_| Ok(())).is_err());
        assert_eq!(fs::read_dir(&fixture.root).unwrap().count(), 0);
    }
}

#[test]
fn missing_or_symlinked_notices_are_rejected_by_startup_checks() {
    for name in ["LEGAL", "LICENSE-2.0.txt"] {
        let (directory, root, bundle) = candidate();
        let path = bundle.join(name);
        fs::remove_file(&path).unwrap();
        assert!(
            ManagedInstallation::open(&root).is_err(),
            "missing {name} was accepted"
        );
        let external = directory.path().join("notice");
        File::create(&external)
            .unwrap()
            .set_len(
                pins()
                    .into_iter()
                    .find(|pin| pin.name == name)
                    .unwrap()
                    .bytes,
            )
            .unwrap();
        symlink(&external, &path).unwrap();
        assert!(
            ManagedInstallation::open(&root).is_err(),
            "symlinked {name} was accepted"
        );
    }
}

#[test]
fn malformed_unsupported_and_oversized_installation_records_are_rejected() {
    for bytes in [
        b"{broken".to_vec(),
        br#"{"version":2,"generation":"core-20260723-v0-unpinned"}"#.to_vec(),
        br#"{"version":1,"generation":"../external"}"#.to_vec(),
        vec![b' '; MANIFEST_LIMIT as usize + 1],
    ] {
        let (_directory, root, _bundle) = candidate();
        fs::write(root.join("current"), bytes).unwrap();
        assert!(ManagedInstallation::open(root).is_err());
    }
    let (_directory, root, bundle) = candidate();
    fs::set_permissions(
        bundle.join("installation.json"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let mut receipt: Receipt =
        read_manifest_file(&bundle.join("installation.json"), false).unwrap();
    receipt.files[0].sha256 = "wrong pin".into();
    fs::write(
        bundle.join("installation.json"),
        serde_json::to_vec(&receipt).unwrap(),
    )
    .unwrap();
    fs::set_permissions(
        bundle.join("installation.json"),
        fs::Permissions::from_mode(0o400),
    )
    .unwrap();
    assert!(ManagedInstallation::open(root).is_err());
}

#[test]
fn interrupted_import_child() {
    let Some(root) = std::env::var_os("YOMIBU_IMPORT_CHILD_ROOT") else {
        return;
    };
    let source = PathBuf::from(std::env::var_os("YOMIBU_IMPORT_CHILD_SOURCE").unwrap());
    let files: Vec<Artifact> =
        serde_json::from_str(&std::env::var("YOMIBU_IMPORT_CHILD_PINS").unwrap()).unwrap();
    let selected = std::env::var("YOMIBU_IMPORT_CHILD_STEP").unwrap();
    import_with(Path::new(&root), &source, &files, |step| {
        if format!("{step:?}") == selected {
            fs::write(source.join("ready"), b"ready").unwrap();
            loop {
                std::thread::park();
            }
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn process_interruption_preserves_complete_generations_and_releases_writer_lock() {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    for step in [
        ImportStep::Copy,
        ImportStep::SyncFile,
        ImportStep::PublishBundle,
        ImportStep::PublishCurrent,
        ImportStep::SyncRoot,
    ] {
        let fixture = Fixture::new();
        let old = fixture.import();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "sudachi_dictionary::installation::tests::interrupted_import_child",
                "--nocapture",
            ])
            .env("YOMIBU_IMPORT_CHILD_ROOT", &fixture.root)
            .env("YOMIBU_IMPORT_CHILD_SOURCE", &fixture.source)
            .env(
                "YOMIBU_IMPORT_CHILD_PINS",
                serde_json::to_string(&fixture.files).unwrap(),
            )
            .env("YOMIBU_IMPORT_CHILD_STEP", format!("{step:?}"))
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !fixture.source.join("ready").exists() {
            if Instant::now() >= deadline || child.try_wait().unwrap().is_some() {
                let _ = child.kill();
                let _ = child.wait();
                panic!("child did not reach {step:?}");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        child.kill().unwrap();
        child.wait().unwrap();
        let visible = fixture.current();
        assert_eq!(visible == old, step != ImportStep::SyncRoot);
        for generation in [old, visible] {
            fixture.verify_generation(&generation);
        }
        fixture.import();
    }
}

#[test]
fn failure_to_sync_new_ancestor_entries_prevents_first_publication() {
    let fixture = Fixture::new();
    let root = fixture._directory.path().join("new/parents/managed");
    let result = import_with(&root, &fixture.source, &fixture.files, |step| {
        if step == ImportStep::SyncAncestors {
            Err(io::Error::other("injected ancestor sync failure"))
        } else {
            Ok(())
        }
    });
    assert!(
        result.is_err(),
        "published without synchronizing new ancestor entries"
    );
    assert!(!root.join("current").exists());
}
