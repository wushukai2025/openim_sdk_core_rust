use std::collections::HashMap;

use openim_errors::{OpenImError, Result};
use openim_sync::{diff_by, DiffOptions, SyncAction};
use openim_types::{GroupId, UserId};
use serde::{Deserialize, Serialize};

use crate::{summarize_action, DomainSyncSummary};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInfo {
    pub group_id: GroupId,
    pub group_name: String,
    pub face_url: String,
    pub owner_user_id: UserId,
    pub member_count: u32,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupMemberInfo {
    pub group_id: GroupId,
    pub user_id: UserId,
    pub nickname: String,
    pub face_url: String,
    pub role_level: i32,
    pub join_time: i64,
}

#[derive(Debug, Default)]
pub struct GroupService {
    groups: HashMap<GroupId, GroupInfo>,
    members: HashMap<(GroupId, UserId), GroupMemberInfo>,
}

impl GroupService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert_group(&mut self, group: GroupInfo) -> Result<()> {
        ensure_id(&group.group_id, "group_id")?;
        self.groups.insert(group.group_id.clone(), group);
        Ok(())
    }

    pub fn get_group(&self, group_id: &str) -> Result<Option<GroupInfo>> {
        ensure_id(group_id, "group_id")?;
        Ok(self.groups.get(group_id).cloned())
    }

    pub fn joined_groups(&self) -> Vec<GroupInfo> {
        let mut groups = self.groups.values().cloned().collect::<Vec<_>>();
        groups.sort_by(|left, right| left.group_id.cmp(&right.group_id));
        groups
    }

    pub fn sync_groups(&mut self, server_groups: Vec<GroupInfo>) -> Result<DomainSyncSummary> {
        for group in &server_groups {
            ensure_id(&group.group_id, "group_id")?;
        }

        let local_groups = self.joined_groups();
        let plan = diff_by(
            &server_groups,
            &local_groups,
            |group| group.group_id.clone(),
            |server, local| server == local,
            DiffOptions::default(),
        );

        let mut summary = DomainSyncSummary::default();
        for action in plan.actions {
            summarize_action(&mut summary, &action);
            match action {
                SyncAction::Insert { server } | SyncAction::Update { server, .. } => {
                    self.groups.insert(server.group_id.clone(), server);
                }
                SyncAction::Delete { local } => {
                    self.groups.remove(&local.group_id);
                    self.members
                        .retain(|(group_id, _), _| group_id != &local.group_id);
                }
                SyncAction::Unchanged { .. } => {}
            }
        }

        Ok(summary)
    }

    pub fn upsert_member(&mut self, member: GroupMemberInfo) -> Result<()> {
        ensure_pair(&member.group_id, &member.user_id)?;
        self.members.insert(member_key(&member), member);
        Ok(())
    }

    pub fn group_members(&self, group_id: &str) -> Result<Vec<GroupMemberInfo>> {
        ensure_id(group_id, "group_id")?;
        let mut members = self
            .members
            .values()
            .filter(|member| member.group_id == group_id)
            .cloned()
            .collect::<Vec<_>>();
        members.sort_by(|left, right| left.user_id.cmp(&right.user_id));
        Ok(members)
    }

    pub fn sync_group_members(
        &mut self,
        group_id: &str,
        server_members: Vec<GroupMemberInfo>,
    ) -> Result<DomainSyncSummary> {
        ensure_id(group_id, "group_id")?;
        for member in &server_members {
            ensure_pair(&member.group_id, &member.user_id)?;
            if member.group_id != group_id {
                return Err(OpenImError::args("server member group_id mismatch"));
            }
        }

        let local_members = self.group_members(group_id)?;
        let plan = diff_by(
            &server_members,
            &local_members,
            |member| member.user_id.clone(),
            |server, local| server == local,
            DiffOptions::default(),
        );

        let mut summary = DomainSyncSummary::default();
        for action in plan.actions {
            summarize_action(&mut summary, &action);
            match action {
                SyncAction::Insert { server } | SyncAction::Update { server, .. } => {
                    self.members.insert(member_key(&server), server);
                }
                SyncAction::Delete { local } => {
                    self.members.remove(&member_key(&local));
                }
                SyncAction::Unchanged { .. } => {}
            }
        }

        Ok(summary)
    }
}

fn ensure_pair(group_id: &str, user_id: &str) -> Result<()> {
    ensure_id(group_id, "group_id")?;
    ensure_id(user_id, "user_id")
}

fn ensure_id(value: &str, field: &str) -> Result<()> {
    if value.is_empty() {
        Err(OpenImError::args(format!("{field} is empty")))
    } else {
        Ok(())
    }
}

fn member_key(member: &GroupMemberInfo) -> (GroupId, UserId) {
    (member.group_id.clone(), member.user_id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_groups_inserts_updates_and_deletes() {
        let mut service = GroupService::new();
        service.upsert_group(group("g-old", "Old")).unwrap();
        service.upsert_group(group("g-delete", "Delete")).unwrap();
        service
            .upsert_member(member("g-delete", "u-delete", "Delete"))
            .unwrap();

        let summary = service
            .sync_groups(vec![group("g-new", "New"), group("g-old", "Old 2")])
            .unwrap();

        assert_eq!(summary.inserted, 1);
        assert_eq!(summary.updated, 1);
        assert_eq!(summary.deleted, 1);
        assert_eq!(
            service.get_group("g-old").unwrap().unwrap().group_name,
            "Old 2"
        );
        assert!(service.get_group("g-delete").unwrap().is_none());
        assert!(service.group_members("g-delete").unwrap().is_empty());
    }

    #[test]
    fn sync_group_members_is_scoped_by_group() {
        let mut service = GroupService::new();
        service.upsert_member(member("g1", "u1", "old")).unwrap();
        service.upsert_member(member("g2", "u2", "keep")).unwrap();

        let summary = service
            .sync_group_members("g1", vec![member("g1", "u3", "new")])
            .unwrap();

        assert_eq!(summary.inserted, 1);
        assert_eq!(summary.deleted, 1);
        assert_eq!(
            service
                .group_members("g1")
                .unwrap()
                .iter()
                .map(|member| member.user_id.as_str())
                .collect::<Vec<_>>(),
            vec!["u3"]
        );
        assert_eq!(service.group_members("g2").unwrap().len(), 1);
    }

    fn group(group_id: &str, name: &str) -> GroupInfo {
        GroupInfo {
            group_id: group_id.to_string(),
            group_name: name.to_string(),
            face_url: String::new(),
            owner_user_id: "owner".to_string(),
            member_count: 1,
            updated_at: 0,
        }
    }

    fn member(group_id: &str, user_id: &str, nickname: &str) -> GroupMemberInfo {
        GroupMemberInfo {
            group_id: group_id.to_string(),
            user_id: user_id.to_string(),
            nickname: nickname.to_string(),
            face_url: String::new(),
            role_level: 0,
            join_time: 0,
        }
    }
}
