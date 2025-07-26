use crate::config::{Config, MonitorConfig};
use crate::types::{CheckResult, ClientStatus, MonitorResult, Status, Summary};
use crate::{metrics, state::StateTracker, webhook::WebhookClient};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Duration, Utc};
use ibc_proto::ibc::core::{
    channel::v1::{query_client::QueryClient as ChannelQueryClient, QueryChannelRequest},
    client::v1::{query_client::QueryClient, QueryClientStateRequest, QueryConsensusStateRequest},
    connection::v1::{query_client::QueryClient as ConnectionQueryClient, QueryConnectionRequest},
};
use ibc_proto::{
    google::protobuf::Any,
    ibc::lightclients::tendermint::v1::{ClientState as TendermintClientState, ConsensusState as TendermintConsensusState},
};
use prost::Message;
use tokio::time;
use tonic::transport::Channel;
use tracing::info;

pub struct Monitor {
    config: Config,
    webhook: WebhookClient,
    state: StateTracker,
}

impl Monitor {
    pub fn new(config: Config, webhook_url: Option<String>) -> Self {
        Self { 
            config,
            webhook: WebhookClient::new(webhook_url),
            state: StateTracker::new(),
        }
    }

    pub async fn check_all(&self) -> CheckResult {
        let start = std::time::Instant::now();
        let mut all_statuses = Vec::new();
        let mut monitors = Vec::new();

        for cfg in &self.config.monitors {
            let status = match self.check_client(cfg).await {
                Ok(mut s) => {
                    s.channel = cfg.channel.clone();
                    s
                }
                Err(e) => self.error_status(cfg, e.to_string()),
            };
            
            // Record metrics and check for alerts
            if let Some(counterparty) = &status.counterparty_chain_id {
                let hours = match &status.status {
                    Status::Healthy { hours_until_expiry } |
                    Status::Warning { hours_until_expiry } |
                    Status::Critical { hours_until_expiry } => *hours_until_expiry,
                    Status::Expired { hours_since_expiry } => -*hours_since_expiry,
                    Status::Error { .. } => 0.0,
                };
                
                metrics::record_client_check(
                    &status.chain_id,
                    &status.client_id,
                    counterparty,
                    &status.status,
                    hours,
                );
                
                let key = format!("{}:{}", status.chain_id, status.client_id);
                if self.state.has_changed(&key, &status.status).await {
                    match &status.status {
                        Status::Healthy { .. } => {
                            tracing::info!(
                                chain = %status.chain_id,
                                client = %status.client_id,
                                counterparty = %counterparty,
                                hours_left = %hours,
                                "client recovered"
                            );
                        }
                        Status::Warning { .. } => {
                            tracing::warn!(
                                chain = %status.chain_id,
                                client = %status.client_id,
                                counterparty = %counterparty,
                                hours_left = %hours,
                                "client expiry warning"
                            );
                        }
                        Status::Critical { .. } => {
                            tracing::error!(
                                chain = %status.chain_id,
                                client = %status.client_id,
                                counterparty = %counterparty,
                                hours_left = %hours,
                                "client expiry critical"
                            );
                        }
                        Status::Expired { .. } => {
                            tracing::error!(
                                chain = %status.chain_id,
                                client = %status.client_id,
                                counterparty = %counterparty,
                                hours_ago = %hours.abs(),
                                "client expired"
                            );
                        }
                        Status::Error { reason } => {
                            tracing::error!(
                                chain = %status.chain_id,
                                client = %status.client_id,
                                counterparty = %counterparty,
                                reason = %reason,
                                "client error"
                            );
                        }
                    }
                    
                    // Optionally still send webhook for critical states
                    if !matches!(&status.status, Status::Healthy { .. }) {
                        if let Err(e) = self.webhook
                            .send_alert(&status.chain_id, &status.client_id, Some(counterparty), &status.status)
                            .await 
                        {
                            tracing::debug!("webhook failed: {}", e);
                        }
                    }
                }
            }
            
            all_statuses.push(status.clone());
            monitors.push(MonitorResult {
                clients: vec![status],
            });
        }

        metrics::record_check_duration(start.elapsed().as_secs_f64());

        CheckResult {
            timestamp: Utc::now(),
            monitors,
            summary: Summary::from_statuses(&all_statuses),
        }
    }

    pub async fn run(&self) -> Result<()> {
        info!("monitoring interval: {}s", self.config.global.check_interval);
        loop {
            crate::output::print_results(&self.check_all().await);
            time::sleep(time::Duration::from_secs(self.config.global.check_interval)).await;
        }
    }

    async fn discover_client_id(&self, grpc_addr: &str, channel_id: &str) -> Result<String> {
        let channel = Channel::from_shared(grpc_addr.to_string())?
            .connect_timeout(std::time::Duration::from_secs(10))
            .connect()
            .await?;

        let chan = ChannelQueryClient::new(channel.clone())
            .channel(QueryChannelRequest {
                port_id: "transfer".to_string(),
                channel_id: channel_id.to_string(),
            })
            .await?
            .into_inner()
            .channel
            .ok_or_else(|| anyhow!("channel not found"))?;

        let conn_id = chan.connection_hops
            .first()
            .ok_or_else(|| anyhow!("no connection hops"))?
            .clone();

        let client_id = ConnectionQueryClient::new(channel)
            .connection(QueryConnectionRequest { connection_id: conn_id })
            .await?
            .into_inner()
            .connection
            .ok_or_else(|| anyhow!("connection not found"))?
            .client_id;

        info!("discovered {} for {}", client_id, channel_id);
        Ok(client_id)
    }

    async fn check_client(&self, cfg: &MonitorConfig) -> Result<ClientStatus> {
        let client_id = match &cfg.client_id {
            Some(id) => id.clone(),
            None => self.discover_client_id(&cfg.grpc_addr, &cfg.channel).await?,
        };

        let channel = Channel::from_shared(cfg.grpc_addr.clone())?
            .connect_timeout(std::time::Duration::from_secs(10))
            .connect()
            .await?;

        let mut client = QueryClient::new(channel);

        let resp = match client
            .client_state(QueryClientStateRequest { client_id: client_id.clone() })
            .await
        {
            Ok(r) => r.into_inner(),
            Err(status) if status.code() == tonic::Code::NotFound || 
                          status.message().contains("not found") ||
                          status.message().contains("expired") => {
                return Ok(ClientStatus {
                    chain_id: cfg.chain_id.clone(),
                    client_id,
                    status: Status::Expired { hours_since_expiry: -1.0 },
                    last_update: None,
                    trusting_period: Duration::zero(),
                    unbonding_period: Duration::zero(),
                    latest_height: None,
                    counterparty_chain_id: None,
                    channel: String::new(),
                });
            }
            Err(e) => return Err(anyhow!("client query failed: {}", e)),
        };

        let client_state_any = resp.client_state
            .ok_or_else(|| anyhow!("no client state"))?;

        let client_state = parse_tendermint_client_state(&client_state_any)?;
        
        let trusting_period = client_state.trusting_period.as_ref()
            .ok_or_else(|| anyhow!("no trusting period"))
            .map(|p| Duration::seconds(p.seconds) + Duration::nanoseconds(p.nanos as i64))?;
        
        let unbonding_period = client_state.unbonding_period.as_ref()
            .ok_or_else(|| anyhow!("no unbonding period"))
            .map(|p| Duration::seconds(p.seconds) + Duration::nanoseconds(p.nanos as i64))?;
        
        let latest_height = client_state.latest_height
            .ok_or_else(|| anyhow!("no latest height"))?;
        
        let counterparty_chain_id = client_state.chain_id.clone();

        let consensus_state_any = client
            .consensus_state(QueryConsensusStateRequest {
                client_id: client_id.clone(),
                revision_number: latest_height.revision_number,
                revision_height: latest_height.revision_height,
                latest_height: false,
            })
            .await?
            .into_inner()
            .consensus_state
            .ok_or_else(|| anyhow!("no consensus state"))?;

        let consensus_state = parse_tendermint_consensus_state(&consensus_state_any)?;
        let ts = consensus_state.timestamp
            .ok_or_else(|| anyhow!("no timestamp"))?;
        
        let last_update = DateTime::from_timestamp(ts.seconds, ts.nanos as u32)
            .ok_or_else(|| anyhow!("invalid timestamp"))?;
        
        let expires_at = last_update + trusting_period;
        let time_until_expiry = expires_at - Utc::now();

        let hours = time_until_expiry.num_hours() as f64;
        let status = match hours {
            h if h < 0.0 => Status::Expired { hours_since_expiry: -h },
            h if h < self.config.global.critical_threshold as f64 => Status::Critical { hours_until_expiry: h },
            h if h < self.config.global.warning_threshold as f64 => Status::Warning { hours_until_expiry: h },
            h => Status::Healthy { hours_until_expiry: h },
        };

        Ok(ClientStatus {
            chain_id: cfg.chain_id.clone(),
            client_id,
            status,
            last_update: Some(last_update),
            trusting_period,
            unbonding_period,
            latest_height: Some((latest_height.revision_number, latest_height.revision_height)),
            counterparty_chain_id: Some(counterparty_chain_id),
            channel: String::new(),
        })
    }

    fn error_status(&self, cfg: &MonitorConfig, error: String) -> ClientStatus {
        ClientStatus {
            chain_id: cfg.chain_id.clone(),
            client_id: cfg.client_id.clone().unwrap_or_else(|| "unknown".to_string()),
            status: Status::Error { reason: error },
            last_update: None,
            trusting_period: Duration::zero(),
            unbonding_period: Duration::zero(),
            latest_height: None,
            counterparty_chain_id: None,
            channel: cfg.channel.clone(),
        }
    }
}

fn parse_tendermint_client_state(any: &Any) -> Result<TendermintClientState> {
    (any.type_url == "/ibc.lightclients.tendermint.v1.ClientState")
        .then(|| TendermintClientState::decode(&any.value[..]))
        .ok_or_else(|| anyhow!("wrong type: {}", any.type_url))?
        .context("decode failed")
}

fn parse_tendermint_consensus_state(any: &Any) -> Result<TendermintConsensusState> {
    (any.type_url == "/ibc.lightclients.tendermint.v1.ConsensusState")
        .then(|| TendermintConsensusState::decode(&any.value[..]))
        .ok_or_else(|| anyhow!("wrong type: {}", any.type_url))?
        .context("decode failed")
}