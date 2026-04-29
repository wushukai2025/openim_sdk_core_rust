use std::collections::HashMap;

use openim_errors::{OpenImError, Result};
use openim_sync::{diff_by, DiffOptions, SyncAction};
use openim_types::UserId;
use serde::{Deserialize, Serialize};

use crate::{summarize_action, DomainSyncSummary};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendInfo {
    pub owner_user_id: UserId,
    pub friend_user_id: UserId,
    pub nickname: String,
    pub remark: String,
    pub face_url: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlacklistInfo {
    pub owner_user_id: UserId,
    pub blocked_user_id: UserId,
    pub nickname: String,
    pub face_url: String,
    pub updated_at: i64,
}

pub trait FriendRepository {
    fn save_friend(&mut self, friend: FriendInfo) -> Result<()>;
    fn remove_friend(&mut self, owner_user_id: &str, friend_user_id: &str) -> Result<()>;
    fn load_friends(&self, owner_user_id: &str) -> Result<Vec<FriendInfo>>;
}

pub trait BlacklistRepository {
    fn save_blacklist(&mut self, black: BlacklistInfo) -> Result<()>;
    fn remove_blacklist(&mut self, owner_user_id: &str, blocked_user_id: &str) -> Result<()>;
    fn load_blacklist(&self, owner_user_id: &str) -> Result<Vec<BlacklistInfo>>;
}

#[derive(Debug, Default)]
pub struct RelationService {
    friends: HashMap<(UserId, UserId), FriendInfo>,
    blacklist: HashMap<(UserId, UserId), BlacklistInfo>,
}

impl RelationService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert_friend(&mut self, friend: FriendInfo) -> Result<()> {
        ensure_pair(&friend.owner_user_id, &friend.friend_user_id)?;
        self.friends.insert(friend_key(&friend), friend);
        Ok(())
    }

    pub fn delete_friend(&mut self, owner_user_id: &str, friend_user_id: &str) -> Result<()> {
        ensure_pair(owner_user_id, friend_user_id)?;
        self.friends
            .remove(&(owner_user_id.to_string(), friend_user_id.to_string()));
        Ok(())
    }

    pub fn get_friend(
        &self,
        owner_user_id: &str,
        friend_user_id: &str,
    ) -> Result<Option<FriendInfo>> {
        ensure_pair(owner_user_id, friend_user_id)?;
        Ok(self
            .friends
            .get(&(owner_user_id.to_string(), friend_user_id.to_string()))
            .cloned())
    }

    pub fn all_friends(&self, owner_user_id: &str) -> Result<Vec<FriendInfo>> {
        ensure_user_id(owner_user_id, "owner_user_id")?;
        let mut friends = self
            .friends
            .values()
            .filter(|friend| friend.owner_user_id == owner_user_id)
            .cloned()
            .collect::<Vec<_>>();
        friends.sort_by(|left, right| left.friend_user_id.cmp(&right.friend_user_id));
        Ok(friends)
    }

    pub fn sync_friends(
        &mut self,
        owner_user_id: &str,
        server_friends: Vec<FriendInfo>,
    ) -> Result<DomainSyncSummary> {
        ensure_user_id(owner_user_id, "owner_user_id")?;
        for friend in &server_friends {
            ensure_owner(owner_user_id, &friend.owner_user_id)?;
            ensure_pair(&friend.owner_user_id, &friend.friend_user_id)?;
        }

        let local_friends = self.all_friends(owner_user_id)?;
        let plan = diff_by(
            &server_friends,
            &local_friends,
            |friend| friend.friend_user_id.clone(),
            |server, local| server == local,
            DiffOptions::default(),
        );

        let mut summary = DomainSyncSummary::default();
        for action in plan.actions {
            summarize_action(&mut summary, &action);
            match action {
                SyncAction::Insert { server } | SyncAction::Update { server, .. } => {
                    self.friends.insert(friend_key(&server), server);
                }
                SyncAction::Delete { local } => {
                    self.friends.remove(&friend_key(&local));
                }
                SyncAction::Unchanged { .. } => {}
            }
        }

        Ok(summary)
    }

    pub fn upsert_blacklist(&mut self, black: BlacklistInfo) -> Result<()> {
        ensure_pair(&black.owner_user_id, &black.blocked_user_id)?;
        self.blacklist.insert(black_key(&black), black);
        Ok(())
    }

    pub fn delete_blacklist(&mut self, owner_user_id: &str, blocked_user_id: &str) -> Result<()> {
        ensure_pair(owner_user_id, blocked_user_id)?;
        self.blacklist
            .remove(&(owner_user_id.to_string(), blocked_user_id.to_string()));
        Ok(())
    }

    pub fn all_blacklist(&self, owner_user_id: &str) -> Result<Vec<BlacklistInfo>> {
        ensure_user_id(owner_user_id, "owner_user_id")?;
        let mut blacklist = self
            .blacklist
            .values()
            .filter(|black| black.owner_user_id == owner_user_id)
            .cloned()
            .collect::<Vec<_>>();
        blacklist.sort_by(|left, right| left.blocked_user_id.cmp(&right.blocked_user_id));
        Ok(blacklist)
    }

    pub fn sync_blacklist(
        &mut self,
        owner_user_id: &str,
        server_blacklist: Vec<BlacklistInfo>,
    ) -> Result<DomainSyncSummary> {
        ensure_user_id(owner_user_id, "owner_user_id")?;
        for black in &server_blacklist {
            ensure_owner(owner_user_id, &black.owner_user_id)?;
            ensure_pair(&black.owner_user_id, &black.blocked_user_id)?;
        }

        let local_blacklist = self.all_blacklist(owner_user_id)?;
        let plan = diff_by(
            &server_blacklist,
            &local_blacklist,
            |black| black.blocked_user_id.clone(),
            |server, local| server == local,
            DiffOptions::default(),
        );

        let mut summary = DomainSyncSummary::default();
        for action in plan.actions {
            summarize_action(&mut summary, &action);
            match action {
                SyncAction::Insert { server } | SyncAction::Update { server, .. } => {
                    self.blacklist.insert(black_key(&server), server);
                }
                SyncAction::Delete { local } => {
                    self.blacklist.remove(&black_key(&local));
                }
                SyncAction::Unchanged { .. } => {}
            }
        }

        Ok(summary)
    }
}

impl FriendRepository for RelationService {
    fn save_friend(&mut self, friend: FriendInfo) -> Result<()> {
        RelationService::upsert_friend(self, friend)
    }

    fn remove_friend(&mut self, owner_user_id: &str, friend_user_id: &str) -> Result<()> {
        RelationService::delete_friend(self, owner_user_id, friend_user_id)
    }

    fn load_friends(&self, owner_user_id: &str) -> Result<Vec<FriendInfo>> {
        RelationService::all_friends(self, owner_user_id)
    }
}

impl BlacklistRepository for RelationService {
    fn save_blacklist(&mut self, black: BlacklistInfo) -> Result<()> {
        RelationService::upsert_blacklist(self, black)
    }

    fn remove_blacklist(&mut self, owner_user_id: &str, blocked_user_id: &str) -> Result<()> {
        RelationService::delete_blacklist(self, owner_user_id, blocked_user_id)
    }

    fn load_blacklist(&self, owner_user_id: &str) -> Result<Vec<BlacklistInfo>> {
        RelationService::all_blacklist(self, owner_user_id)
    }
}

fn ensure_pair(owner_user_id: &str, target_user_id: &str) -> Result<()> {
    ensure_user_id(owner_user_id, "owner_user_id")?;
    ensure_user_id(target_user_id, "target_user_id")
}

fn ensure_user_id(user_id: &str, field: &str) -> Result<()> {
    if user_id.is_empty() {
        Err(OpenImError::args(format!("{field} is empty")))
    } else {
        Ok(())
    }
}

fn ensure_owner(expected: &str, actual: &str) -> Result<()> {
    if expected != actual {
        Err(OpenImError::args("server item owner_user_id mismatch"))
    } else {
        Ok(())
    }
}

fn friend_key(friend: &FriendInfo) -> (UserId, UserId) {
    (friend.owner_user_id.clone(), friend.friend_user_id.clone())
}

fn black_key(black: &BlacklistInfo) -> (UserId, UserId) {
    (black.owner_user_id.clone(), black.blocked_user_id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_friends_inserts_updates_and_deletes_by_owner() {
        let mut service = RelationService::new();
        service
            .upsert_friend(friend("owner", "old", "Old"))
            .unwrap();
        service
            .upsert_friend(friend("owner", "delete", "Delete"))
            .unwrap();
        service
            .upsert_friend(friend("other-owner", "keep", "Keep"))
            .unwrap();

        let summary = service
            .sync_friends(
                "owner",
                vec![
                    friend("owner", "new", "New"),
                    friend("owner", "old", "Old 2"),
                ],
            )
            .unwrap();

        assert_eq!(summary.inserted, 1);
        assert_eq!(summary.updated, 1);
        assert_eq!(summary.deleted, 1);
        assert_eq!(
            service
                .get_friend("owner", "old")
                .unwrap()
                .unwrap()
                .nickname,
            "Old 2"
        );
        assert!(service.get_friend("owner", "delete").unwrap().is_none());
        assert!(service.get_friend("other-owner", "keep").unwrap().is_some());
    }

    #[test]
    fn sync_blacklist_deletes_missing_items() {
        let mut service = RelationService::new();
        service
            .upsert_blacklist(black("owner", "blocked-1"))
            .unwrap();

        let summary = service.sync_blacklist("owner", Vec::new()).unwrap();

        assert_eq!(summary.deleted, 1);
        assert!(service.all_blacklist("owner").unwrap().is_empty());
    }

    #[test]
    fn repository_traits_delegate_to_relation_service() {
        let mut repository = RelationService::new();
        repository
            .save_friend(friend("owner", "u1", "Alice"))
            .unwrap();
        repository.save_blacklist(black("owner", "u2")).unwrap();

        assert_eq!(repository.load_friends("owner").unwrap().len(), 1);
        assert_eq!(repository.load_blacklist("owner").unwrap().len(), 1);

        repository.remove_friend("owner", "u1").unwrap();
        repository.remove_blacklist("owner", "u2").unwrap();

        assert!(repository.load_friends("owner").unwrap().is_empty());
        assert!(repository.load_blacklist("owner").unwrap().is_empty());
    }

    fn friend(owner: &str, user_id: &str, nickname: &str) -> FriendInfo {
        FriendInfo {
            owner_user_id: owner.to_string(),
            friend_user_id: user_id.to_string(),
            nickname: nickname.to_string(),
            remark: String::new(),
            face_url: String::new(),
            updated_at: 0,
        }
    }

    fn black(owner: &str, user_id: &str) -> BlacklistInfo {
        BlacklistInfo {
            owner_user_id: owner.to_string(),
            blocked_user_id: user_id.to_string(),
            nickname: user_id.to_string(),
            face_url: String::new(),
            updated_at: 0,
        }
    }
}
