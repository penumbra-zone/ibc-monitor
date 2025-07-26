use crate::types::{CheckResult, Status, ClientStatus};
use chrono::Utc;
use colored::*;
use std::collections::HashMap;

pub fn print_results(result: &CheckResult) {
    println!("ibc client monitor - {}", result.timestamp.format("%Y-%m-%d %H:%M:%S UTC"));
    println!();
    
    let mut grouped: HashMap<(String, String), Vec<&ClientStatus>> = HashMap::new();
    
    for monitor in &result.monitors {
        for client in &monitor.clients {
            let a = client.chain_id.clone();
            let b = client.counterparty_chain_id.clone().unwrap_or_else(|| "?".to_string());
            let key = if a < b { (a, b) } else { (b, a) };
            grouped.entry(key).or_default().push(client);
        }
    }
    
    println!("{:<8} {:<30} {:<15} {:<22} {:<15} {:<10} {:<25} {:<20}", 
        "status", "connection", "channel", "client", "time left", "trust/ub", "height@chain", "last update");
    println!("{}", "─".repeat(160));
    
    let mut sorted: Vec<_> = grouped.into_iter().collect();
    sorted.sort_by_key(|(k, _)| k.clone());
    
    for (_, mut clients) in sorted {
        clients.sort_by_key(|c| &c.chain_id);
        
        for client in clients {
            let status_str = match &client.status {
                Status::Healthy { .. } => "[ok]   ",
                Status::Warning { .. } => "[warn] ",
                Status::Critical { .. } => "[crit] ",
                Status::Expired { .. } => "[expd] ",
                Status::Error { .. } => "[err]  ",
            };
            
            let counterparty = client.counterparty_chain_id.as_ref()
                .and_then(|c| c.split('-').next())
                .unwrap_or("?");
            let host = client.chain_id.split('-').next().unwrap_or(&client.chain_id);
            let connection = format!("{:<12} @ {:<12}", counterparty, host);
            
            let time_left = match &client.status {
                Status::Healthy { hours_until_expiry } | 
                Status::Warning { hours_until_expiry } | 
                Status::Critical { hours_until_expiry } => {
                    if *hours_until_expiry > 24.0 {
                        format!("{:.1}d left", hours_until_expiry / 24.0)
                    } else {
                        format!("{:.0}h left", hours_until_expiry)
                    }
                }
                Status::Expired { hours_since_expiry } => {
                    if *hours_since_expiry > 24.0 {
                        format!("{:.0}d expired", hours_since_expiry / 24.0)
                    } else {
                        format!("{:.0}h expired", hours_since_expiry)
                    }
                }
                Status::Error { .. } => "error".to_string(),
            };
            
            let periods = format!("{:.0}d/{:.0}d", 
                client.trusting_period.num_days(),
                client.unbonding_period.num_days()
            );
            
            let height_info = match (client.latest_height, &client.counterparty_chain_id) {
                (Some((_, h)), Some(c)) => format!("{}@{}", h, c.split('-').next().unwrap_or(c)),
                _ => String::new(),
            };
            
            let last_update = client.last_update
                .map(|t| {
                    let d = Utc::now() - t;
                    match (d.num_days(), d.num_hours(), d.num_minutes()) {
                        (days, _, _) if days > 0 => format!("{}d ago", days),
                        (_, hours, _) if hours > 0 => format!("{}h ago", hours),
                        (_, _, mins) => format!("{}m ago", mins),
                    }
                })
                .unwrap_or_else(|| "unknown".to_string());
            
            let line = format!("{:<8} {:<30} {:<15} {:<22} {:<15} {:<10} {:<25} {:<20}", 
                status_str, connection, &client.channel, &client.client_id, time_left, periods, height_info, last_update);
            
            match &client.status {
                Status::Expired { .. } => println!("{}", line.red()),
                _ => println!("{}", line),
            }
        }
        println!();
    }
    
    println!("{}", "─".repeat(160));
    let s = &result.summary;
    println!("summary: {} total | {} healthy | {} warning | {} critical | {} expired | {} errors",
        s.total, s.healthy, s.warning, s.critical, s.expired, s.error
    );
}