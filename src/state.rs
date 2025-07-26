use crate::types::Status;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

#[derive(Clone, Default)]
pub struct StateTracker(Arc<Mutex<HashMap<String, Status>>>);

impl StateTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn has_changed(&self, key: &str, status: &Status) -> bool {
        let mut states = self.0.lock().await;
        match states.get(key) {
            Some(prev) if std::mem::discriminant(prev) == std::mem::discriminant(status) => false,
            _ => {
                states.insert(key.into(), status.clone());
                true
            }
        }
    }
}