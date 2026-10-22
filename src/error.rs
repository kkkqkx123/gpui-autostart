/// The result type returned by gpui-autostart operations.
pub type Result<T> = std::result::Result<T, Error>;

/// An error produced while registering or querying an auto-start entry.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The current target does not have an auto-start backend.
    #[error("auto-start is unsupported on this platform")]
    UnsupportedPlatform,

    /// An operation was attempted from a thread other than the creating thread.
    #[error("auto-start operation must run on the GPUI thread")]
    WrongThread,

    /// The builder settings cannot describe a valid registration.
    #[error("invalid auto-start configuration: {0}")]
    InvalidConfig(String),

    /// A native platform operation failed.
    #[error("native auto-start error: {0}")]
    Native(#[source] Box<dyn std::error::Error + Send + Sync>),

    /// A native platform operation failed without exposing a typed source.
    #[error("native auto-start error: {0}")]
    NativeMessage(String),
}

impl Error {
    pub(crate) fn native(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Native(Box::new(error))
    }

    pub(crate) fn native_message(message: impl Into<String>) -> Self {
        Self::NativeMessage(message.into())
    }
}
