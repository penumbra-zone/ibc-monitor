# Claude Code Project Context

## Project Overview

This is an IBC (Inter-Blockchain Communication) monitoring tool that tracks the status of IBC light clients across chains, alerting operators before clients expire.

## Key Commands

### Testing and Development
- **Lint**: `cargo clippy --all-targets --all-features`
- **Type check**: `cargo check --all-targets`
- **Test**: `cargo test`
- **Format**: `cargo fmt`
- **Build**: `cargo build --release`

### Running the Monitor
- **Single check**: `ibc-monitor check -c monitor.toml`
- **Continuous monitoring**: `ibc-monitor run -c monitor.toml`

## Architecture

The codebase follows a modular structure:
- `main.rs` - CLI entry point with command parsing
- `monitor.rs` - Core monitoring logic and gRPC client interactions
- `types.rs` - Data structures for client status and results
- `config.rs` - TOML configuration parsing
- `metrics.rs` - Prometheus metrics instrumentation
- `webhook.rs` - Slack webhook notifications
- `state.rs` - State tracking to prevent duplicate alerts
- `output.rs` - Formatted output for CLI
- `server.rs` - HTTP server for metrics endpoint

## Code Style Guidelines

Following Penumbra project conventions:
- Use `#![deny(clippy::unwrap_used)]` to enforce error handling
- Add module-level documentation comments
- Use descriptive variable names
- Group related functionality into well-defined modules
- Prefer explicit error handling with `anyhow::Result`
- Add inline comments for complex logic
- Use structured logging with `tracing`

## Important Context

- The monitor discovers client IDs automatically via channel queries if not specified
- State tracking prevents alert fatigue by only notifying on state changes
- Metrics are exposed in Prometheus format for integration with observability stacks
- The tool supports both one-shot checks and continuous monitoring modes

## Common Tasks

### Adding a New Monitor
Add to `monitor.toml`:
```toml
[[monitors]]
name = "Chain A to Chain B"
chain_id = "chain-a-id"
rpc_addr = "https://rpc.chain-a.com"
grpc_addr = "http://grpc.chain-a.com:9090"
channel = "channel-123"
```

### Adjusting Alert Thresholds
In `monitor.toml`:
```toml
[global]
warning_threshold = 120    # hours
critical_threshold = 24    # hours
```

### Enabling Webhook Notifications
Set the webhook URL in config or environment:
```toml
[global]
webhook_url = "https://hooks.slack.com/services/..."
```

Or:
```bash
WEBHOOK_URL="https://hooks.slack.com/..." ibc-monitor run
```