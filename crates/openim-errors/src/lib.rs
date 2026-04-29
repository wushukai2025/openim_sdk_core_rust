use std::fmt;

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ErrorCode(i32);

impl ErrorCode {
    pub const NETWORK: Self = Self(10000);
    pub const NETWORK_TIMEOUT: Self = Self(10001);
    pub const ARGS: Self = Self(10002);
    pub const CTX_DEADLINE_EXCEEDED: Self = Self(10003);
    pub const UNKNOWN: Self = Self(10005);
    pub const SDK_INTERNAL: Self = Self(10006);
    pub const NO_UPDATE: Self = Self(10007);
    pub const SDK_NOT_INIT: Self = Self(10008);
    pub const SDK_NOT_LOGIN: Self = Self(10009);
    pub const USER_ID_NOT_FOUND: Self = Self(10100);
    pub const LOGIN_OUT: Self = Self(10101);
    pub const LOGIN_REPEAT: Self = Self(10102);
    pub const FILE_NOT_FOUND: Self = Self(10200);
    pub const MSG_DECOMPRESSION: Self = Self(10201);
    pub const MSG_DECODE_BINARY_WS: Self = Self(10202);
    pub const MSG_BINARY_TYPE_NOT_SUPPORT: Self = Self(10203);
    pub const MSG_REPEAT: Self = Self(10204);
    pub const MSG_CONTENT_TYPE_NOT_SUPPORT: Self = Self(10205);
    pub const MSG_HAS_NO_SEQ: Self = Self(10206);
    pub const MSG_HAS_DELETED: Self = Self(10207);
    pub const NOT_SUPPORT_OPT: Self = Self(10301);
    pub const NOT_SUPPORT_TYPE: Self = Self(10302);
    pub const UNREAD_COUNT: Self = Self(10303);
    pub const GROUP_ID_NOT_FOUND: Self = Self(10400);
    pub const GROUP_TYPE: Self = Self(10401);

    pub const fn new(code: i32) -> Self {
        Self(code)
    }

    pub const fn as_i32(self) -> i32 {
        self.0
    }

    pub const fn category(self) -> ErrorCategory {
        match self.0 {
            10000..=10099 => ErrorCategory::Common,
            10100..=10199 => ErrorCategory::User,
            10200..=10299 => ErrorCategory::Message,
            10300..=10399 => ErrorCategory::Conversation,
            10400..=10499 => ErrorCategory::Group,
            _ => ErrorCategory::Unknown,
        }
    }
}

impl From<i32> for ErrorCode {
    fn from(value: i32) -> Self {
        Self(value)
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    Common,
    User,
    Message,
    Conversation,
    Group,
    Unknown,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("{code}: {message}")]
pub struct OpenImError {
    code: ErrorCode,
    message: String,
    detail: Option<String>,
}

impl OpenImError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    pub fn category(&self) -> ErrorCategory {
        self.code.category()
    }

    pub fn args(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::ARGS, message)
    }

    pub fn sdk_internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::SDK_INTERNAL, message)
    }

    pub fn msg_decompression(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::MSG_DECOMPRESSION, message)
    }

    pub fn msg_decode_binary_ws(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::MSG_DECODE_BINARY_WS, message)
    }
}

pub type Result<T> = std::result::Result<T, OpenImError>;

pub fn ws_error(err_code: i32, err_msg: impl Into<String>) -> Option<OpenImError> {
    if err_code == 0 {
        None
    } else {
        Some(OpenImError::new(ErrorCode::from(err_code), err_msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_unknown_error_code() {
        let err = OpenImError::new(ErrorCode::new(59999), "custom");

        assert_eq!(err.code().as_i32(), 59999);
        assert_eq!(err.category(), ErrorCategory::Unknown);
        assert_eq!(err.to_string(), "59999: custom");
    }

    #[test]
    fn maps_go_sdk_error_categories() {
        assert_eq!(ErrorCode::NETWORK.category(), ErrorCategory::Common);
        assert_eq!(ErrorCode::LOGIN_OUT.category(), ErrorCategory::User);
        assert_eq!(
            ErrorCode::MSG_DECOMPRESSION.category(),
            ErrorCategory::Message
        );
        assert_eq!(
            ErrorCode::UNREAD_COUNT.category(),
            ErrorCategory::Conversation
        );
        assert_eq!(ErrorCode::GROUP_TYPE.category(), ErrorCategory::Group);
    }

    #[test]
    fn websocket_success_has_no_error() {
        assert!(ws_error(0, "").is_none());
        assert_eq!(
            ws_error(10001, "timeout").unwrap().code(),
            ErrorCode::NETWORK_TIMEOUT
        );
    }
}
