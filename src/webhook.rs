use crate::types::Status;
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::time::Duration;

#[derive(Clone)]
pub struct WebhookClient {
    client: reqwest::Client,
    url: Option<String>,
}

#[derive(Serialize)]
struct SlackMessage<'a> {
    text: String,
    attachments: [Attachment<'a>; 1],
}

#[derive(Serialize)]
struct Attachment<'a> {
    color: &'a str,
    fields: [Field<'a>; 4],
    footer: &'a str,
    ts: i64,
}

#[derive(Serialize)]
struct Field<'a> {
    title: &'a str,
    value: String,
    short: bool,
}

impl WebhookClient {
    pub fn new(url: Option<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
            url,
        }
    }

    pub async fn send_alert(
        &self,
        chain: &str,
        client_id: &str,
        counterparty: Option<&str>,
        status: &Status,
    ) -> Result<()> {
        let Some(url) = &self.url else { return Ok(()) };

        let (emoji, label, color, desc) = match status {
            Status::Healthy { hours_until_expiry: h } => 
                ("✅", "healthy", "good", format!("{:.1}h left", h)),
            Status::Warning { hours_until_expiry: h } => 
                ("⚠️", "warning", "warning", format!("{:.1}h left", h)),
            Status::Critical { hours_until_expiry: h } => 
                ("🚨", "critical", "danger", format!("{:.1}h left", h)),
            Status::Expired { hours_since_expiry: h } => 
                ("❌", "expired", "danger", format!("{:.1}h ago", h)),
            Status::Error { reason } => 
                ("❗", "error", "danger", reason.clone()),
        };

        let msg = SlackMessage {
            text: format!("{} ibc alert: {}", emoji, label),
            attachments: [Attachment {
                color,
                fields: [
                    Field { title: "chain", value: chain.into(), short: true },
                    Field { title: "client", value: client_id.into(), short: true },
                    Field { title: "counterparty", value: counterparty.unwrap_or("?").into(), short: true },
                    Field { title: "status", value: format!("{} - {}", label, desc), short: true },
                ],
                footer: "ibc-monitor",
                ts: chrono::Utc::now().timestamp(),
            }],
        };

        tracing::info!(%chain, %client_id, %label, "webhook");

        let res = self.client.post(url).json(&msg).send().await?;
        if !res.status().is_success() {
            return Err(anyhow!("webhook {}: {}", res.status(), res.text().await?));
        }
        Ok(())
    }
}