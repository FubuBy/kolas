use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub type EventId = String;

pub trait LockStore: Send + Sync {
    fn try_lock(&self, id: &EventId, ttl: Duration) -> bool;
    fn release(&self, id: &EventId);
}

pub struct NoOpLockStore;

impl LockStore for NoOpLockStore {
    fn try_lock(&self, _id: &EventId, _ttl: Duration) -> bool {
        true
    }

    fn release(&self, _id: &EventId) {}
}

pub struct InMemoryLockStore {
    locks: Mutex<HashMap<EventId, Instant>>,
}

impl InMemoryLockStore {
    pub fn new() -> Self {
        Self {
            locks: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryLockStore {
    fn default() -> Self {
        Self::new()
    }
}

impl LockStore for InMemoryLockStore {
    fn try_lock(&self, id: &EventId, ttl: Duration) -> bool {
        let mut locks = self.locks.lock().expect("lock poisoned");
        let now = Instant::now();

        if let Some(&acquired_at) = locks.get(id) && now.duration_since(acquired_at) < ttl {
            return false;
        }
        locks.insert(id.clone(), now);
        true
    }

    fn release(&self, id: &EventId) {
        self.locks.lock().expect("lock poisoned").remove(id);
    }
}
