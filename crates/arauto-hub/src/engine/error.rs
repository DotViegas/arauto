//! What can go wrong with the engine. No variant carries a text of the engine, a value that
//! was refused, an address or a secret: only fixed words and numbers.

use arauto_protocol::errors::ErrorCode;

/// Why the engine counts as unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// No engine is configured, or it has no key.
    NotConfigured,
    /// The connection did not open.
    Connect,
    /// The answer did not come in time.
    Timeout,
    /// The engine answered with an error of its own (5xx), or an unexpected status.
    Status(u16),
    /// The engine refused the key of its API.
    KeyRefused,
    /// The answer is not what the engine documents: not JSON, not of the expected shape, or
    /// with a value outside its format.
    Malformed,
    /// The answer is larger than the ceiling of that call.
    TooLarge,
    /// The answer has another type of content than the call expects.
    ContentType,
}

impl Cause {
    /// A fixed word for the logs and the screen.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Cause::NotConfigured => "not configured",
            Cause::Connect => "no connection",
            Cause::Timeout => "no answer in time",
            Cause::Status(_) => "an error status",
            Cause::KeyRefused => "the key was refused",
            Cause::Malformed => "a malformed answer",
            Cause::TooLarge => "an answer above the ceiling",
            Cause::ContentType => "an answer of another type",
        }
    }
}

/// What a call to the engine returned instead of an answer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EngineError {
    /// The engine is down, slow, or answered something unusable (AR-703).
    #[error("the WhatsApp engine is unavailable: {}", .0.as_str())]
    Unavailable(Cause),
    /// The number is not connected to WhatsApp (AR-702).
    #[error("the number is not connected to WhatsApp")]
    NumberDisconnected,
    /// The WhatsApp imposed a limit (AR-704).
    #[error("the WhatsApp limited this number")]
    WhatsappLimit,
    /// What was asked for does not exist in the engine (AR-705).
    #[error("not found in the engine")]
    NotFound,
    /// The engine refused the request (AR-711).
    #[error("the engine refused the request")]
    Refused,
    /// An argument was refused by the hub before anything went to the engine (AR-711). The
    /// word names the kind of argument, never its value.
    #[error("invalid {0}")]
    Invalid(&'static str),
}

impl EngineError {
    pub(crate) const fn malformed() -> Self {
        EngineError::Unavailable(Cause::Malformed)
    }

    /// The code of the catalog the Hermes sees for this error
    /// (`docs/08-permissoes.md`, section 3).
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            EngineError::Unavailable(_) => ErrorCode::Ar703,
            EngineError::NumberDisconnected => ErrorCode::Ar702,
            EngineError::WhatsappLimit => ErrorCode::Ar704,
            EngineError::NotFound => ErrorCode::Ar705,
            EngineError::Refused | EngineError::Invalid(_) => ErrorCode::Ar711,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_error_has_its_code() {
        assert_eq!(
            EngineError::Unavailable(Cause::Timeout).code(),
            ErrorCode::Ar703
        );
        assert_eq!(EngineError::NumberDisconnected.code(), ErrorCode::Ar702);
        assert_eq!(EngineError::WhatsappLimit.code(), ErrorCode::Ar704);
        assert_eq!(EngineError::NotFound.code(), ErrorCode::Ar705);
        assert_eq!(EngineError::Refused.code(), ErrorCode::Ar711);
        assert_eq!(
            EngineError::Invalid("chat identifier").code(),
            ErrorCode::Ar711
        );
    }
}
