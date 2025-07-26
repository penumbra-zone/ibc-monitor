use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

mod config;
mod metrics;
mod monitor;
mod output;
mod server;
mod state;
mod types;
mod webhook;

use config::Config;
use monitor::Monitor;

#[derive(Parser)]
#[command(name = "ibc-monitor")]
#[command(about = "Monitor IBC client expiry", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Check {
        #[arg(short, long, default_value = "monitor.toml")]
        config: PathBuf,
    },
    Run {
        #[arg(short, long, default_value = "monitor.toml")]
        config: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let json_output = std::env::var("LOG_FORMAT")
        .map(|f| f.eq_ignore_ascii_case("json"))
        .unwrap_or(false);

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    if json_output {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(filter)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .init();
    }

    // Initialize metrics
    metrics::init();
    let prometheus_handle = metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()?;

    let cli = Cli::parse();
    match cli.command {
        Commands::Check { config } => {
            let cfg = Config::load(&config)?;
            let webhook_url = std::env::var("WEBHOOK_URL").ok()
                .or_else(|| cfg.global.webhook_url.clone());
            let monitor = Monitor::new(cfg, webhook_url);
            let results = monitor.check_all().await;
            output::print_results(&results);
        }
        Commands::Run { config } => {
            let cfg = Config::load(&config)?;
            let webhook_url = std::env::var("WEBHOOK_URL").ok()
                .or_else(|| cfg.global.webhook_url.clone());
            let monitor = Monitor::new(cfg.clone(), webhook_url);
            
            // Start metrics server if enabled
            if cfg.global.metrics_enabled.unwrap_or(true) {
                let addr = cfg.global.metrics_addr
                    .as_deref()
                    .unwrap_or("0.0.0.0:9090")
                    .parse()?;
                tokio::spawn(server::run(addr, prometheus_handle));
            }
            
            monitor.run().await?;
        }
    }

    Ok(())
}
