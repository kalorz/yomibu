#[derive(Clone)]
pub struct Secret(Option<String>);
impl Secret {
    pub fn new(value: String) -> Self {
        Self(Some(value))
    }
    pub fn invalid_encoding() -> Self {
        Self(None)
    }
    pub fn expose(&self) -> Result<&str, CredentialError> {
        self.0.as_deref().ok_or(CredentialError::InvalidEncoding)
    }
}
impl From<String> for Secret {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}
impl From<&str> for Secret {
    fn from(value: &str) -> Self {
        Self::new(value.into())
    }
}
#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum CredentialError {
    #[error("Invalid credential input; supply a UTF-8 credential.")]
    InvalidEncoding,
    #[error(
        "Cannot access the credential store; unlock it or supply a CLI/environment credential."
    )]
    StoreUnavailable,
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}
