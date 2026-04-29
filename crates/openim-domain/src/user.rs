use std::collections::HashMap;

use openim_errors::{OpenImError, Result};
use openim_types::UserId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    pub user_id: UserId,
    pub nickname: String,
    pub face_url: String,
    pub ex: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserProfilePatch {
    pub nickname: Option<String>,
    pub face_url: Option<String>,
    pub ex: Option<String>,
    pub updated_at: Option<i64>,
}

pub trait UserProfileRepository {
    fn save_profile(&mut self, profile: UserProfile) -> Result<()>;
    fn load_profile(&self, user_id: &str) -> Result<Option<UserProfile>>;
    fn load_profiles(&self, user_ids: &[UserId]) -> Result<Vec<UserProfile>>;
}

#[derive(Debug, Default)]
pub struct UserService {
    profiles: HashMap<UserId, UserProfile>,
}

impl UserService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert_profile(&mut self, profile: UserProfile) -> Result<()> {
        ensure_user_id(&profile.user_id)?;
        self.profiles.insert(profile.user_id.clone(), profile);
        Ok(())
    }

    pub fn update_profile(&mut self, user_id: &str, patch: UserProfilePatch) -> Result<()> {
        ensure_user_id(user_id)?;
        let profile = self
            .profiles
            .get_mut(user_id)
            .ok_or_else(|| OpenImError::args(format!("user profile not found: {user_id}")))?;

        if let Some(nickname) = patch.nickname {
            profile.nickname = nickname;
        }
        if let Some(face_url) = patch.face_url {
            profile.face_url = face_url;
        }
        if let Some(ex) = patch.ex {
            profile.ex = ex;
        }
        if let Some(updated_at) = patch.updated_at {
            profile.updated_at = updated_at;
        }

        Ok(())
    }

    pub fn get_profile(&self, user_id: &str) -> Result<Option<UserProfile>> {
        ensure_user_id(user_id)?;
        Ok(self.profiles.get(user_id).cloned())
    }

    pub fn get_profiles(&self, user_ids: &[UserId]) -> Result<Vec<UserProfile>> {
        let mut profiles = Vec::new();
        for user_id in user_ids {
            ensure_user_id(user_id)?;
            if let Some(profile) = self.profiles.get(user_id) {
                profiles.push(profile.clone());
            }
        }
        Ok(profiles)
    }

    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }
}

impl UserProfileRepository for UserService {
    fn save_profile(&mut self, profile: UserProfile) -> Result<()> {
        UserService::upsert_profile(self, profile)
    }

    fn load_profile(&self, user_id: &str) -> Result<Option<UserProfile>> {
        UserService::get_profile(self, user_id)
    }

    fn load_profiles(&self, user_ids: &[UserId]) -> Result<Vec<UserProfile>> {
        UserService::get_profiles(self, user_ids)
    }
}

fn ensure_user_id(user_id: &str) -> Result<()> {
    if user_id.is_empty() {
        Err(OpenImError::args("user_id is empty"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_profile_round_trips_and_updates() {
        let mut service = UserService::new();
        service
            .upsert_profile(UserProfile {
                user_id: "u1".to_string(),
                nickname: "Alice".to_string(),
                face_url: "face-a".to_string(),
                ex: String::new(),
                updated_at: 1,
            })
            .unwrap();

        service
            .update_profile(
                "u1",
                UserProfilePatch {
                    nickname: Some("Alice B".to_string()),
                    updated_at: Some(2),
                    ..Default::default()
                },
            )
            .unwrap();

        let profile = service.get_profile("u1").unwrap().unwrap();
        assert_eq!(profile.nickname, "Alice B");
        assert_eq!(profile.face_url, "face-a");
        assert_eq!(profile.updated_at, 2);
    }

    #[test]
    fn get_profiles_preserves_requested_order_and_skips_missing() {
        let mut service = UserService::new();
        for user_id in ["u1", "u2"] {
            service
                .upsert_profile(UserProfile {
                    user_id: user_id.to_string(),
                    nickname: user_id.to_string(),
                    face_url: String::new(),
                    ex: String::new(),
                    updated_at: 0,
                })
                .unwrap();
        }

        let profiles = service
            .get_profiles(&["u2".to_string(), "missing".to_string(), "u1".to_string()])
            .unwrap();

        assert_eq!(
            profiles
                .iter()
                .map(|profile| profile.user_id.as_str())
                .collect::<Vec<_>>(),
            vec!["u2", "u1"]
        );
    }

    #[test]
    fn repository_trait_delegates_to_user_service() {
        let mut repository = UserService::new();
        repository
            .save_profile(UserProfile {
                user_id: "u1".to_string(),
                nickname: "Alice".to_string(),
                face_url: String::new(),
                ex: String::new(),
                updated_at: 1,
            })
            .unwrap();

        assert_eq!(
            repository.load_profile("u1").unwrap().unwrap().nickname,
            "Alice"
        );
    }
}
