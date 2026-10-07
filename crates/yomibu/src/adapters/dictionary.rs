//! Explicit, offline installation of pinned dictionary bundles.
//!
//! Installation verifies copied bytes. A receipt records that operation; it is
//! not a signature or a substitute for controlling subsequent file writes.

use std::{
    fs::{self, File},
    io::{self, Read, Seek, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use sudachi::dic::header::{Header, HeaderVersion, SystemDictVersion};

use super::sudachi::{DICTIONARY_BYTES, DICTIONARY_SHA256, DICTIONARY_VERSION};

const MANIFEST_LIMIT: u64 = 16 * 1024;
const RECEIPT_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    name: String,
    bytes: u64,
    sha256: String,
}

fn pins() -> Vec<Artifact> {
    [
        ("system_core.dic", DICTIONARY_BYTES, DICTIONARY_SHA256),
        (
            "LEGAL",
            6037,
            "725a8776b38e058b185e905594bc9a2437dbf3787df022fffeefedb9a84e4665",
        ),
        (
            "LICENSE-2.0.txt",
            11358,
            "cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30",
        ),
    ]
    .into_iter()
    .map(|(name, bytes, sha256)| Artifact {
        name: name.into(),
        bytes,
        sha256: sha256.into(),
    })
    .collect()
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    version: u32,
    dictionary_version: String,
    files: Vec<Artifact>,
    dictionary_fingerprint: FileFingerprint,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileFingerprint {
    device: u64,
    inode: u64,
    bytes: u64,
    mtime_seconds: i64,
    mtime_nanoseconds: i64,
    ctime_seconds: i64,
    ctime_nanoseconds: i64,
}

impl From<&fs::Metadata> for FileFingerprint {
    fn from(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            bytes: metadata.len(),
            mtime_seconds: metadata.mtime(),
            mtime_nanoseconds: metadata.mtime_nsec(),
            ctime_seconds: metadata.ctime(),
            ctime_nanoseconds: metadata.ctime_nsec(),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Current {
    version: u32,
    generation: String,
}

#[derive(Debug, thiserror::Error)]
pub enum InstallationError {
    #[error(
        "Cannot access dictionary installation; use yomibu dictionary import --bundle PATH: {0}"
    )]
    Io(#[from] io::Error),
    #[error("Dictionary installation record error: {0}")]
    Manifest(#[from] serde_json::Error),
    #[error(
        "Unsafe dictionary storage at {path}; check ownership, private permissions and links, or import into a new --dictionary-dir PATH."
    )]
    UnsafeStorage { path: PathBuf },
    #[error(
        "Invalid or unsafe dictionary installation; reimport with yomibu dictionary import --bundle PATH."
    )]
    Invalid,
    #[error(
        "Dictionary metadata changed since installation; run yomibu dictionary verify --dictionary-dir PATH, then reimport with yomibu dictionary import --bundle PATH --dictionary-dir PATH."
    )]
    Changed,
    #[error("Dictionary bundle file {0} does not match its pinned length/SHA-256.")]
    Mismatch(String),
    #[error("Another dictionary import holds the installation lock; wait for it to finish.")]
    Locked,
    #[error("Dictionary installation was published, but its durability is uncertain: {0}")]
    DurabilityUncertain(#[source] io::Error),
}

/// One selected generation. Opening checks installation records, not a full hash.
/// Published files must be controlled by the installation/update protocol.
pub struct ManagedInstallation {
    pub(crate) dictionary: File,
    pub(crate) generation: String,
    bundle: PathBuf,
}

impl ManagedInstallation {
    /// Select one completed generation without installing, downloading or writing.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, InstallationError> {
        let (mut installation, fingerprint) = Self::select(root.as_ref())?;
        if FileFingerprint::from(&installation.dictionary.metadata()?) != fingerprint {
            return Err(InstallationError::Changed);
        }
        let mut bytes = [0; Header::STORAGE_SIZE];
        installation.dictionary.read_exact(&mut bytes)?;
        let header = Header::parse(&bytes).map_err(|_| InstallationError::Invalid)?;
        if header.version != HeaderVersion::SystemDict(SystemDictVersion::Version2)
            || header.description != "20260723"
        {
            return Err(InstallationError::Invalid);
        }
        Ok(installation)
    }

    fn select(root: &Path) -> Result<(Self, FileFingerprint), InstallationError> {
        private_directory(root)?;
        private_directory(&root.join("bundles"))?;
        let current: Current = read_manifest(&root.join("current"))?;
        if current.version != 1 || !valid_generation(&current.generation) {
            return Err(InstallationError::Invalid);
        }
        let bundle = root.join("bundles").join(&current.generation);
        private_directory(&bundle)?;
        let receipt: Receipt = read_manifest_file(&bundle.join("installation.json"), true)?;
        if receipt.version != RECEIPT_VERSION
            || receipt.dictionary_version != DICTIONARY_VERSION
            || receipt.files != pins()
            || receipt.dictionary_fingerprint.bytes != DICTIONARY_BYTES
        {
            return Err(InstallationError::Invalid);
        }
        let dictionary = open_regular(&bundle.join("system_core.dic"), true)?;
        for pin in pins()
            .into_iter()
            .filter(|pin| pin.name != "system_core.dic")
        {
            let notice = open_regular(&bundle.join(&pin.name), true)?;
            if notice.metadata()?.len() != pin.bytes {
                return Err(InstallationError::Invalid);
            }
        }
        Ok((
            Self {
                dictionary,
                generation: current.generation,
                bundle,
            },
            receipt.dictionary_fingerprint,
        ))
    }

    pub fn generation(&self) -> &str {
        &self.generation
    }
}

fn valid_generation(name: &str) -> bool {
    name.starts_with("core-20260723-v0-")
        && name.len() <= 160
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn read_manifest<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, InstallationError> {
    read_manifest_file(path, false)
}

fn read_manifest_file<T: serde::de::DeserializeOwned>(
    path: &Path,
    read_only: bool,
) -> Result<T, InstallationError> {
    let mut bytes = Vec::new();
    open_regular(path, read_only)?
        .take(MANIFEST_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MANIFEST_LIMIT {
        return Err(InstallationError::Invalid);
    }
    Ok(serde_json::from_slice(&bytes)?)
}

fn owned_private(metadata: &fs::Metadata) -> bool {
    // geteuid has no pointer arguments or failure case. Environment variables
    // cannot establish the OS identity that controls this installation.
    metadata.uid() == unsafe { libc::geteuid() } && metadata.mode() & 0o077 == 0
}

fn private_directory(path: &Path) -> Result<(), InstallationError> {
    let directory = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY)
        .open(path)?;
    if !owned_private(&directory.metadata()?) {
        return Err(InstallationError::UnsafeStorage { path: path.into() });
    }
    Ok(())
}

fn open_regular(path: &Path, read_only: bool) -> Result<File, InstallationError> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || !owned_private(&metadata)
        || metadata.nlink() != 1
        || (read_only && metadata.mode() & 0o222 != 0)
    {
        return Err(InstallationError::Invalid);
    }
    Ok(file)
}

fn verify_file(file: &mut File, pin: &Artifact) -> Result<(), InstallationError> {
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() != pin.bytes {
        return Err(InstallationError::Mismatch(pin.name.clone()));
    }
    file.rewind()?;
    let mut input = file.take(pin.bytes + 1);
    let mut hash = Sha256::new();
    let mut length = 0;
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        length += count as u64;
        hash.update(&buffer[..count]);
    }
    if length != pin.bytes || format!("{:x}", hash.finalize()) != pin.sha256 {
        return Err(InstallationError::Mismatch(pin.name.clone()));
    }
    Ok(())
}

/// Full byte verification results; metadata equality does not establish file stability.
#[derive(Debug)]
pub struct Verification {
    pub metadata_matches_installation: bool,
}

/// Fully verify the selected dictionary and both notices; never repair implicitly.
pub fn verify(root: impl AsRef<Path>) -> Result<Verification, InstallationError> {
    let (mut installation, fingerprint) = ManagedInstallation::select(root.as_ref())?;
    for pin in pins() {
        if pin.name == "system_core.dic" {
            verify_file(&mut installation.dictionary, &pin)?;
        } else {
            let mut notice = open_regular(&installation.bundle.join(&pin.name), true)?;
            verify_file(&mut notice, &pin)?;
        }
    }
    Ok(Verification {
        metadata_matches_installation: FileFingerprint::from(&installation.dictionary.metadata()?)
            == fingerprint,
    })
}

/// Copy and fully verify a publisher bundle before publishing a new generation.
/// No existing completed generation is edited or removed, including on failure.
pub fn import_bundle(root: &Path, source: &Path) -> Result<String, InstallationError> {
    import_with(root, source, &pins(), |_| Ok(()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportStep {
    SyncAncestors,
    Copy,
    SyncFile,
    PublishBundle,
    PublishCurrent,
    SyncRoot,
}

struct ImportGuard(File);

impl Drop for ImportGuard {
    fn drop(&mut self) {
        // Unlock explicitly so an inherited descriptor cannot extend this guard.
        let _ = self.0.unlock();
    }
}

fn import_with(
    root: &Path,
    source: &Path,
    files: &[Artifact],
    mut before: impl FnMut(ImportStep) -> io::Result<()>,
) -> Result<String, InstallationError> {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(root)?;
    private_directory(root)?;
    fs::DirBuilder::new()
        .mode(0o700)
        .create(root.join("bundles"))
        .or_else(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                Ok(())
            } else {
                Err(error)
            }
        })?;
    private_directory(&root.join("bundles"))?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(root.join("installation.lock"))?;
    let metadata = lock.metadata()?;
    if !metadata.is_file() || !owned_private(&metadata) || metadata.nlink() != 1 {
        return Err(InstallationError::UnsafeStorage {
            path: root.join("installation.lock"),
        });
    }
    match lock.try_lock() {
        Ok(()) => {}
        Err(fs::TryLockError::WouldBlock) => return Err(InstallationError::Locked),
        Err(fs::TryLockError::Error(error)) => return Err(error.into()),
    }
    let _guard = ImportGuard(lock);
    // A previous interrupted import may have created these entries without
    // syncing them. Existing metadata alone cannot establish their durability.
    let parents: Vec<_> = root
        .ancestors()
        .skip(1)
        .map(|path| {
            if path.as_os_str().is_empty() {
                Path::new(".")
            } else {
                path
            }
        })
        .collect();
    for parent in parents.into_iter().rev() {
        before(ImportStep::SyncAncestors)?;
        File::open(parent)?.sync_all()?;
    }
    let staging = tempfile::Builder::new()
        .prefix(".import-")
        .tempdir_in(root)?;
    let bundle = staging.path().join("bundle");
    fs::DirBuilder::new().mode(0o700).create(&bundle)?;
    let mut dictionary_fingerprint = None;
    for pin in files {
        before(ImportStep::Copy)?;
        let input = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(source.join(&pin.name))?;
        let metadata = input.metadata()?;
        if !metadata.is_file() || metadata.len() != pin.bytes {
            return Err(InstallationError::Mismatch(pin.name.clone()));
        }
        let path = bundle.join(&pin.name);
        let mut output = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        io::copy(&mut input.take(pin.bytes + 1), &mut output)?;
        verify_file(&mut output, pin)?;
        output.set_permissions(fs::Permissions::from_mode(0o400))?;
        before(ImportStep::SyncFile)?;
        output.sync_all()?;
        if pin.name == "system_core.dic" {
            // Publishing moves the containing directory, leaving this metadata intact.
            dictionary_fingerprint = Some(FileFingerprint::from(&output.metadata()?));
        }
    }
    let receipt = Receipt {
        version: RECEIPT_VERSION,
        dictionary_version: DICTIONARY_VERSION.into(),
        files: files.to_vec(),
        dictionary_fingerprint: dictionary_fingerprint.ok_or(InstallationError::Invalid)?,
    };
    let mut record = File::create(bundle.join("installation.json"))?;
    serde_json::to_writer(&mut record, &receipt)?;
    record.set_permissions(fs::Permissions::from_mode(0o400))?;
    record.sync_all()?;
    File::open(&bundle)?.sync_all()?;
    let suffix = staging
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(InstallationError::Invalid)?;
    let suffix = suffix
        .strip_prefix(".import-")
        .ok_or(InstallationError::Invalid)?;
    let generation = format!("core-20260723-v0-{DICTIONARY_SHA256}-{suffix}");
    before(ImportStep::PublishBundle)?;
    fs::rename(&bundle, root.join("bundles").join(&generation))?;
    File::open(root.join("bundles"))?.sync_all()?;
    let mut pointer = tempfile::NamedTempFile::new_in(root)?;
    serde_json::to_writer(
        &mut pointer,
        &Current {
            version: 1,
            generation: generation.clone(),
        },
    )?;
    pointer.flush()?;
    pointer.as_file().sync_all()?;
    before(ImportStep::PublishCurrent)?;
    pointer
        .persist(root.join("current"))
        .map_err(|error| error.error)?;
    before(ImportStep::SyncRoot)
        .and_then(|()| File::open(root))
        .and_then(|directory| directory.sync_all())
        .map_err(InstallationError::DurabilityUncertain)?;
    Ok(generation)
}

#[cfg(test)]
mod tests;
