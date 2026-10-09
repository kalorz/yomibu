use anyhow::Result;

pub(super) fn read(service: &str, account: &str) -> Result<Option<String>> {
    #[cfg(target_os = "macos")]
    {
        let entry = entry(service, account).map_err(|_| unavailable())?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(_) => Err(unavailable()),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (service, account);
        Err(unavailable())
    }
}

pub(super) fn write(service: &str, account: &str, value: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let entry = entry(service, account).map_err(|_| unavailable())?;
        entry.set_password(value).map_err(|_| unavailable())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (service, account, value);
        Err(unavailable())
    }
}

fn unavailable() -> anyhow::Error {
    anyhow::anyhow!("Cannot access macOS Keychain; unlock it and check access permissions.")
}

#[cfg(target_os = "macos")]
fn entry(service: &str, account: &str) -> keyring_core::Result<keyring_core::Entry> {
    use apple_native_keyring_store::keychain::{Cred, MacKeychainDomain};
    Cred::build(MacKeychainDomain::User, service, account)
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn native_keychain_saves_reads_and_replaces_an_isolated_test_credential() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let service = format!("yomibu:auth-test-{}-{suffix}", std::process::id());
        let account = "api-key";
        let entry = entry(&service, account).unwrap();
        assert!(read(&service, account).unwrap().is_none());
        write(&service, account, "synthetic-first").unwrap();
        let first = read(&service, account);
        let replace = write(&service, account, "synthetic-second");
        let second = read(&service, account);
        entry.delete_credential().unwrap();
        assert_eq!(first.unwrap().unwrap(), "synthetic-first");
        replace.unwrap();
        assert_eq!(second.unwrap().unwrap(), "synthetic-second");
        assert!(read(&service, account).unwrap().is_none());
    }
}
