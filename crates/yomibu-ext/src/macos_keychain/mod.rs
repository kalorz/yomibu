use yomibu_core::ports::infrastructure::credentials::{CredentialError, Secret, SecretStore};

pub struct MacosKeychain;

impl SecretStore for MacosKeychain {
    fn read(&self, service: &str, account: &str) -> Result<Option<Secret>, CredentialError> {
        #[cfg(target_os = "macos")]
        {
            let entry = entry(service, account).map_err(|_| unavailable())?;
            match entry.get_password() {
                Ok(value) => Ok(Some(value.into())),
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

    fn write(&self, service: &str, account: &str, value: &Secret) -> Result<(), CredentialError> {
        #[cfg(target_os = "macos")]
        {
            let entry = entry(service, account).map_err(|_| unavailable())?;
            entry
                .set_password(value.expose()?)
                .map_err(|_| unavailable())
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (service, account, value);
            Err(unavailable())
        }
    }
}

fn unavailable() -> CredentialError {
    CredentialError::StoreUnavailable
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
        assert!(MacosKeychain.read(&service, account).unwrap().is_none());
        MacosKeychain
            .write(&service, account, &"synthetic-first".into())
            .unwrap();
        let first = MacosKeychain.read(&service, account);
        let replace = MacosKeychain.write(&service, account, &"synthetic-second".into());
        let second = MacosKeychain.read(&service, account);
        entry.delete_credential().unwrap();
        assert_eq!(first.unwrap().unwrap().expose().unwrap(), "synthetic-first");
        replace.unwrap();
        assert_eq!(
            second.unwrap().unwrap().expose().unwrap(),
            "synthetic-second"
        );
        assert!(MacosKeychain.read(&service, account).unwrap().is_none());
    }
}
