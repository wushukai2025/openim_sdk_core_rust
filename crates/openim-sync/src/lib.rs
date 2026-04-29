use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use openim_types::VersionState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncState {
    Unchanged,
    Insert,
    Update,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncAction<T> {
    Unchanged { server: T, local: T },
    Insert { server: T },
    Update { server: T, local: T },
    Delete { local: T },
}

impl<T> SyncAction<T> {
    pub const fn state(&self) -> SyncState {
        match self {
            Self::Unchanged { .. } => SyncState::Unchanged,
            Self::Insert { .. } => SyncState::Insert,
            Self::Update { .. } => SyncState::Update,
            Self::Delete { .. } => SyncState::Delete,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncPlan<T> {
    pub actions: Vec<SyncAction<T>>,
}

impl<T> SyncPlan<T> {
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    pub fn action_states(&self) -> Vec<SyncState> {
        self.actions.iter().map(SyncAction::state).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffOptions {
    pub skip_deletion: bool,
    pub include_unchanged: bool,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            skip_deletion: false,
            include_unchanged: true,
        }
    }
}

pub fn diff_by<T, K, Key, Equal>(
    server: &[T],
    local: &[T],
    key: Key,
    equal: Equal,
    options: DiffOptions,
) -> SyncPlan<T>
where
    T: Clone,
    K: Eq + Hash + Clone,
    Key: Fn(&T) -> K,
    Equal: Fn(&T, &T) -> bool,
{
    let mut local_by_key = HashMap::<K, usize>::new();
    for (idx, item) in local.iter().enumerate() {
        local_by_key.insert(key(item), idx);
    }

    let mut consumed = HashSet::<K>::new();
    let mut actions = Vec::new();

    for server_item in server {
        let id = key(server_item);
        let Some(local_idx) = local_by_key.get(&id).copied() else {
            actions.push(SyncAction::Insert {
                server: server_item.clone(),
            });
            continue;
        };

        consumed.insert(id);
        let local_item = &local[local_idx];
        if equal(server_item, local_item) {
            if options.include_unchanged {
                actions.push(SyncAction::Unchanged {
                    server: server_item.clone(),
                    local: local_item.clone(),
                });
            }
        } else {
            actions.push(SyncAction::Update {
                server: server_item.clone(),
                local: local_item.clone(),
            });
        }
    }

    if !options.skip_deletion {
        for local_item in local {
            let id = key(local_item);
            if !consumed.contains(&id) {
                actions.push(SyncAction::Delete {
                    local: local_item.clone(),
                });
            }
        }
    }

    SyncPlan { actions }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionDelta<T> {
    pub version_id: String,
    pub version: u64,
    pub full: bool,
    pub delete_ids: Vec<String>,
    pub updates: Vec<T>,
    pub inserts: Vec<T>,
    pub id_order_changed: bool,
}

impl<T> VersionDelta<T> {
    pub fn has_changes(&self) -> bool {
        self.full
            || !self.delete_ids.is_empty()
            || !self.updates.is_empty()
            || !self.inserts.is_empty()
            || self.id_order_changed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionDecision<T> {
    Noop,
    Full,
    Incremental {
        server_items: Vec<T>,
        next_state: VersionState,
    },
}

pub fn plan_version_sync<T, Key>(
    local_state: &VersionState,
    local_items: &[T],
    delta: &VersionDelta<T>,
    key: Key,
    refreshed_full_ids: Option<Vec<String>>,
) -> VersionDecision<T>
where
    T: Clone,
    Key: Fn(&T) -> String,
{
    if !delta.has_changes() {
        return VersionDecision::Noop;
    }

    if delta.full || delta.version_id != local_state.version_id {
        return VersionDecision::Full;
    }

    if delta.version <= local_state.version {
        return VersionDecision::Noop;
    }

    if delta.version != local_state.version + 1 {
        return VersionDecision::Full;
    }

    let mut uid_list = delete_ids(&local_state.uid_list, &delta.delete_ids);
    let mut items = HashMap::<String, T>::new();
    for item in local_items {
        items.insert(key(item), item.clone());
    }

    for id in &delta.delete_ids {
        items.remove(id);
    }

    for item in delta.updates.iter().chain(delta.inserts.iter()) {
        let id = key(item);
        if !uid_list.contains(&id) {
            uid_list.push(id.clone());
        }
        items.insert(id, item.clone());
    }

    if delta.id_order_changed {
        if let Some(ids) = refreshed_full_ids {
            uid_list = ids;
        }
    }

    let server_items = uid_list
        .iter()
        .filter_map(|id| items.get(id).cloned())
        .collect();

    VersionDecision::Incremental {
        server_items,
        next_state: VersionState {
            version_id: delta.version_id.clone(),
            version: delta.version,
            uid_list,
        },
    }
}

fn delete_ids(ids: &[String], deletes: &[String]) -> Vec<String> {
    if deletes.is_empty() {
        return ids.to_vec();
    }

    let deletes = deletes.iter().collect::<HashSet<_>>();
    ids.iter()
        .filter(|id| !deletes.contains(id))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Item {
        id: String,
        value: i32,
    }

    fn item(id: &str, value: i32) -> Item {
        Item {
            id: id.to_string(),
            value,
        }
    }

    #[test]
    fn diff_plans_insert_update_delete_and_unchanged() {
        let server = vec![item("a", 1), item("b", 3), item("c", 1)];
        let local = vec![item("a", 1), item("b", 2), item("d", 1)];

        let plan = diff_by(
            &server,
            &local,
            |item| item.id.clone(),
            |server, local| server == local,
            DiffOptions::default(),
        );

        assert_eq!(
            plan.action_states(),
            vec![
                SyncState::Unchanged,
                SyncState::Update,
                SyncState::Insert,
                SyncState::Delete,
            ]
        );
    }

    #[test]
    fn diff_can_skip_deletion_and_unchanged_notice() {
        let plan = diff_by(
            &[item("a", 1)],
            &[item("a", 1), item("b", 2)],
            |item| item.id.clone(),
            |server, local| server == local,
            DiffOptions {
                skip_deletion: true,
                include_unchanged: false,
            },
        );

        assert!(plan.is_empty());
    }

    #[test]
    fn version_delta_one_step_incremental_merges_changes() {
        let state = VersionState {
            version_id: "v".to_string(),
            version: 1,
            uid_list: vec!["a".to_string(), "b".to_string()],
        };
        let local = vec![item("a", 1), item("b", 2)];
        let delta = VersionDelta {
            version_id: "v".to_string(),
            version: 2,
            full: false,
            delete_ids: vec!["a".to_string()],
            updates: vec![item("b", 3)],
            inserts: vec![item("c", 4)],
            id_order_changed: false,
        };

        let decision = plan_version_sync(&state, &local, &delta, |item| item.id.clone(), None);

        match decision {
            VersionDecision::Incremental {
                server_items,
                next_state,
            } => {
                assert_eq!(server_items, vec![item("b", 3), item("c", 4)]);
                assert_eq!(next_state.version, 2);
                assert_eq!(next_state.uid_list, vec!["b".to_string(), "c".to_string()]);
            }
            _ => panic!("expected incremental sync"),
        }
    }

    #[test]
    fn version_gap_or_id_mismatch_requires_full_sync() {
        let state = VersionState {
            version_id: "v1".to_string(),
            version: 1,
            uid_list: vec![],
        };
        let mut delta = VersionDelta {
            version_id: "v1".to_string(),
            version: 3,
            full: false,
            delete_ids: vec![],
            updates: vec![item("a", 1)],
            inserts: vec![],
            id_order_changed: false,
        };

        assert_eq!(
            plan_version_sync(&state, &[], &delta, |item| item.id.clone(), None),
            VersionDecision::Full
        );

        delta.version = 2;
        delta.version_id = "v2".to_string();
        assert_eq!(
            plan_version_sync(&state, &[], &delta, |item| item.id.clone(), None),
            VersionDecision::Full
        );
    }
}
