pub mod conversation;
pub mod file;
pub mod group;
pub mod message;
pub mod relation;
pub mod user;

use openim_sync::{SyncAction, SyncState};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DomainSyncSummary {
    pub inserted: usize,
    pub updated: usize,
    pub deleted: usize,
    pub unchanged: usize,
}

impl DomainSyncSummary {
    pub fn total(self) -> usize {
        self.inserted + self.updated + self.deleted + self.unchanged
    }

    fn record(&mut self, state: SyncState) {
        match state {
            SyncState::Unchanged => self.unchanged += 1,
            SyncState::Insert => self.inserted += 1,
            SyncState::Update => self.updated += 1,
            SyncState::Delete => self.deleted += 1,
        }
    }
}

pub(crate) fn summarize_action<T>(summary: &mut DomainSyncSummary, action: &SyncAction<T>) {
    summary.record(action.state());
}
