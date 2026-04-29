use serde::{Deserialize, Deserializer, Serialize, Serializer};

macro_rules! numeric_i32_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $($variant:ident = $value:expr),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        pub enum $name {
            $($variant = $value),+
        }

        impl $name {
            pub const fn as_i32(self) -> i32 {
                self as i32
            }

            pub const fn from_i32(value: i32) -> Option<Self> {
                match value {
                    $($value => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_i32(self.as_i32())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = i32::deserialize(deserializer)?;
                Self::from_i32(value).ok_or_else(|| {
                    serde::de::Error::custom(format!(
                        "invalid {} value {}",
                        stringify!($name),
                        value
                    ))
                })
            }
        }
    };
}

pub type UserId = String;
pub type GroupId = String;
pub type ConversationId = String;
pub type ClientMsgId = String;
pub type ServerMsgId = String;
pub type OperationId = String;

numeric_i32_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(i32)]
    pub enum Platform {
        Ios = 1,
        Android = 2,
        Windows = 3,
        Macos = 4,
        Web = 5,
        MiniWeb = 6,
        Linux = 7,
        AndroidPad = 8,
        Ipad = 9,
        Admin = 10,
        HarmonyOs = 11,
    }
}

numeric_i32_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(i32)]
    pub enum SessionType {
        Single = 1,
        WriteGroup = 2,
        ReadGroup = 3,
        Notification = 4,
    }
}

numeric_i32_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(i32)]
    pub enum MessageContentType {
        Text = 101,
        Picture = 102,
        Sound = 103,
        Video = 104,
        File = 105,
        AtText = 106,
        Merger = 107,
        Card = 108,
        Location = 109,
        Custom = 110,
        Typing = 113,
        Quote = 114,
        Face = 115,
        AdvancedText = 117,
        MarkdownText = 118,
    }
}

numeric_i32_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(i32)]
    pub enum MessageStatus {
        Sending = 1,
        SendSuccess = 2,
        SendFailed = 3,
        HasDeleted = 4,
        Filtered = 5,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    pub page_number: i32,
    pub show_number: i32,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            page_number: 0,
            show_number: 200,
        }
    }
}

impl Pagination {
    pub fn normalized(mut self) -> Self {
        if self.page_number < 0 {
            self.page_number = 0;
        }
        if self.show_number <= 0 {
            self.show_number = 200;
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionState {
    pub version_id: String,
    pub version: u64,
    pub uid_list: Vec<String>,
}

impl VersionState {
    pub fn empty() -> Self {
        Self {
            version_id: String::new(),
            version: 0,
            uid_list: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn platform_ids_match_go_constants() {
        assert_eq!(Platform::Ios.as_i32(), 1);
        assert_eq!(Platform::Web.as_i32(), 5);
        assert_eq!(Platform::HarmonyOs.as_i32(), 11);
        assert_eq!(Platform::from_i32(7), Some(Platform::Linux));
        assert_eq!(Platform::from_i32(99), None);
    }

    #[test]
    fn pagination_keeps_go_default_semantics() {
        let pagination = Pagination {
            page_number: -1,
            show_number: 0,
        }
        .normalized();

        assert_eq!(pagination.page_number, 0);
        assert_eq!(pagination.show_number, 200);
        assert_eq!(
            serde_json::to_value(&pagination).unwrap(),
            json!({"pageNumber": 0, "showNumber": 200})
        );
    }

    #[test]
    fn numeric_enums_serialize_as_contract_ids() {
        assert_eq!(serde_json::to_value(Platform::Web).unwrap(), json!(5));
        assert_eq!(
            serde_json::from_value::<MessageStatus>(json!(3)).unwrap(),
            MessageStatus::SendFailed
        );
        assert!(serde_json::from_value::<SessionType>(json!(99)).is_err());
    }
}
