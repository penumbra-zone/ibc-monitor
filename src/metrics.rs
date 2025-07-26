use crate::types::Status;
use metrics::{counter, describe_counter, describe_gauge, gauge};

pub fn init() {
    describe_gauge!("ibc_client_hours_until_expiry", "Hours until IBC client expires");
    describe_counter!("ibc_client_checks_total", "Total number of client checks");
    describe_gauge!("ibc_client_status", "Current status of IBC client (1=active, 0=inactive)");
    describe_gauge!("ibc_monitor_check_duration_seconds", "Duration of monitor check in seconds");
}

pub fn record_client_check(chain: &str, client: &str, counterparty: &str, status: &Status, hours: f64) {
    let labels = [
        ("chain", chain.to_string()),
        ("client", client.to_string()),
        ("counterparty", counterparty.to_string()),
    ];

    match status {
        Status::Healthy { .. } | Status::Warning { .. } | Status::Critical { .. } => {
            gauge!("ibc_client_hours_until_expiry", &labels).set(hours);
            gauge!("ibc_client_status", &labels).set(1.0);
        }
        Status::Expired { .. } => {
            gauge!("ibc_client_hours_until_expiry", &labels).set(-hours);
            gauge!("ibc_client_status", &labels).set(0.0);
        }
        Status::Error { .. } => gauge!("ibc_client_status", &labels).set(0.0),
    }

    let label = match status {
        Status::Healthy { .. } => "healthy",
        Status::Warning { .. } => "warning",
        Status::Critical { .. } => "critical",
        Status::Expired { .. } => "expired",
        Status::Error { .. } => "error",
    };

    counter!("ibc_client_checks_total", &[("status", label.to_string())]).increment(1);
}

pub fn record_check_duration(duration: f64) {
    gauge!("ibc_monitor_check_duration_seconds").set(duration);
}