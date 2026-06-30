use super::lock::EventId;

pub trait ServerSelector: Send + Sync {
    fn is_leader(&self, event_id: &EventId) -> bool;
}

pub struct NoOpServerSelector;

impl ServerSelector for NoOpServerSelector {
    fn is_leader(&self, _event_id: &EventId) -> bool {
        true
    }
}
