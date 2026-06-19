use tokio::process::Command;
use anyhow::{Result, anyhow};
use crate::types::FailedService;
use std::time::Duration;

/// Helper to run a command with a timeout (copied from paru.rs pattern)
async fn run_cmd_timeout(mut cmd: Command, timeout_dur: Duration) -> Result<std::process::Output> {
    tokio::select! {
        res = cmd.output() => {
            res.map_err(|e| anyhow!("Command execution failed: {}", e))
        }
        _ = tokio::time::sleep(timeout_dur) => {
            Err(anyhow!("Command timed out after {:?}", timeout_dur))
        }
    }
}

fn parse_failed_services(content: &str) -> Vec<FailedService> {
    let mut services = Vec::new();
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        // systemctl output format:
        // UNIT LOAD ACTIVE SUB DESCRIPTION
        // e.g. systemd-networkd-wait-online.service loaded failed failed System Connection Wait
        if parts.len() >= 5 {
            let unit = parts[0].to_string();
            let load = parts[1].to_string();
            let active = parts[2].to_string();
            let sub = parts[3].to_string();
            let description = parts[4..].join(" ");
            services.push(FailedService {
                unit,
                load,
                active,
                sub,
                description,
            });
        }
    }
    services
}

/// Fetch all failed systemd services
pub async fn get_failed_services() -> Result<Vec<FailedService>> {
    let mut cmd = Command::new("systemctl");
    cmd.args(["list-units", "--state=failed", "--type=service", "--legend=no"]);
    
    let output = run_cmd_timeout(cmd, Duration::from_secs(5)).await?;
    if !output.status.success() {
        return Err(anyhow!("systemctl exited with non-zero status"));
    }

    let content = String::from_utf8_lossy(&output.stdout);
    Ok(parse_failed_services(&content))
}

/// Fetch last N lines of logs for a specific service using journalctl
pub async fn get_journal_logs(service_name: &str, num_lines: usize) -> Result<Vec<String>> {
    let mut cmd = Command::new("journalctl");
    cmd.args(["-u", service_name, "-n", &num_lines.to_string(), "--no-pager"]);

    let output = run_cmd_timeout(cmd, Duration::from_secs(5)).await?;
    let content = String::from_utf8_lossy(&output.stdout).to_string();

    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

    // Check if the output is empty or contains standard permission warnings
    if lines.is_empty() {
        lines.push("No log entries found for this service.".to_string());
    } else if content.contains("You are currently not seeing") || content.contains("permission") {
        lines.push("".to_string());
        lines.push("⚠️ Permission Denied: Unable to read system journal logs.".to_string());
        lines.push("To resolve this, you can:".to_string());
        lines.push("  1. Run Aurum as root/sudo: 'sudo aurum'".to_string());
        lines.push("  2. Add your user to the systemd-journal group:".to_string());
        lines.push("     'sudo usermod -aG systemd-journal $USER'".to_string());
    }

    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_failed_services() {
        let sample_output = "\
systemd-networkd-wait-online.service loaded failed failed System Connection Wait
fake-broken.service                  loaded failed failed Broken description text here
";
        let parsed = parse_failed_services(sample_output);
        assert_eq!(parsed.len(), 2);
        
        assert_eq!(parsed[0].unit, "systemd-networkd-wait-online.service");
        assert_eq!(parsed[0].load, "loaded");
        assert_eq!(parsed[0].active, "failed");
        assert_eq!(parsed[0].sub, "failed");
        assert_eq!(parsed[0].description, "System Connection Wait");

        assert_eq!(parsed[1].unit, "fake-broken.service");
        assert_eq!(parsed[1].description, "Broken description text here");
    }

    #[test]
    fn test_parse_failed_services_empty() {
        let parsed = parse_failed_services("");
        assert!(parsed.is_empty());
    }
}
