#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum WsReqIdentifier {
    GetNewestSeq = 1001,
    PullMsgByRange = 1002,
    SendMsg = 1003,
    SendSignalMsg = 1004,
    PullMsgBySeqList = 1005,
    GetConvMaxReadSeq = 1006,
    PullConvLastMessage = 1007,
    PushMsg = 2001,
    KickOnlineMsg = 2002,
    LogoutMsg = 2003,
    SetBackgroundStatus = 2004,
    SubUserOnlineStatus = 2005,
}

impl WsReqIdentifier {
    pub const fn as_i32(self) -> i32 {
        self as i32
    }
}

pub const SDK_TYPE_JS: &str = "js";
pub const SDK_TYPE_GO: &str = "go";
pub const GZIP_COMPRESSION: &str = "gzip";
pub const PHASE1_SDK_VERSION: &str = "rust-phase1-poc";
