use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub global: GlobalConfig,
    pub monitors: Vec<MonitorConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalConfig {
    pub check_interval: u64,
    pub warning_threshold: u64,
    pub critical_threshold: u64,
    pub webhook_url: Option<String>,
    pub metrics_enabled: Option<bool>,
    pub metrics_addr: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MonitorConfig {
    pub name: String,
    pub description: String,
    pub chain_id: String,
    pub rpc_addr: String,
    pub grpc_addr: String,
    pub client_id: Option<String>,
    pub channel: String,
}


impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        toml::from_str(&std::fs::read_to_string(path)?)
            .context("invalid config")
    }
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            check_interval: 300,
            warning_threshold: 48,
            critical_threshold: 24,
            webhook_url: None,
            metrics_enabled: Some(true),
            metrics_addr: Some("0.0.0.0:9090".to_string()),
        }
    }
}