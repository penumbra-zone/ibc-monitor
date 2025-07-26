use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ClientStatus {
    pub chain_id: String,
    pub client_id: String,
    pub status: Status,
    pub last_update: Option<DateTime<Utc>>,
    pub trusting_period: Duration,
    pub unbonding_period: Duration,
    pub latest_height: Option<(u64, u64)>,
    pub counterparty_chain_id: Option<String>,
    pub channel: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Status {
    Healthy { hours_until_expiry: f64 },
    Warning { hours_until_expiry: f64 },
    Critical { hours_until_expiry: f64 },
    Expired { hours_since_expiry: f64 },
    Error { reason: String },
}


#[derive(Debug, Clone)]
pub struct CheckResult {
    pub timestamp: DateTime<Utc>,
    pub monitors: Vec<MonitorResult>,
    pub summary: Summary,
}

#[derive(Debug, Clone)]
pub struct MonitorResult {
    pub clients: Vec<ClientStatus>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Summary {
    pub total: usize,
    pub healthy: usize,
    pub warning: usize,
    pub critical: usize,
    pub expired: usize,
    pub error: usize,
}

impl Summary {
    pub fn from_statuses(statuses: &[ClientStatus]) -> Self {
        let mut s = Self::default();
        s.total = statuses.len();
        
        for status in statuses {
            match &status.status {
                Status::Healthy { .. } => s.healthy += 1,
                Status::Warning { .. } => s.warning += 1,
                Status::Critical { .. } => s.critical += 1,
                Status::Expired { .. } => s.expired += 1,
                Status::Error { .. } => s.error += 1,
            }
        }
        s
    }
}