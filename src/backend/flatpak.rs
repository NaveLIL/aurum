use tokio::process::Command;
use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use reqwest::Client;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct FlatpakSearchApp {
    pub name: String,
    pub app_id: String,
    pub summary: Option<String>,
    pub developer_name: Option<String>,
    pub project_license: Option<String>,
}

#[derive(Deserialize, Debug)]
struct FlathubSearchResponse {
    pub hits: Vec<FlatpakSearchApp>,
}

#[derive(Debug, Clone)]
pub struct FlatpakApp {
    pub name: String,
    pub app_id: String,
    pub version: String,
    pub branch: String,
}

async fn run_command_with_timeout(mut cmd: Command, timeout_dur: std::time::Duration) -> Result<std::process::Output> {
    cmd.kill_on_drop(true);
    tokio::time::timeout(timeout_dur, cmd.output())
        .await
        .context("Command timed out")?
        .context("Command execution failed")
}

pub struct Flatpak;

impl Flatpak {
    /// Check if flatpak CLI is installed in the system.
    pub async fn is_available() -> bool {
        let mut cmd = Command::new("flatpak");
        cmd.arg("--version");
        run_command_with_timeout(cmd, std::time::Duration::from_secs(3))
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Fetch installed Flatpak apps by parsing `flatpak list --app --columns=name,application,version,branch`
    pub async fn get_installed() -> Result<Vec<FlatpakApp>> {
        let mut cmd = Command::new("flatpak");
        cmd.args(["list", "--app", "--columns=name,application,version,branch"]);
        let output = run_command_with_timeout(cmd, std::time::Duration::from_secs(5))
            .await
            .context("Failed to run flatpak list")?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8(output.stdout)?;
        Ok(Self::parse_list_output(&stdout))
    }

    /// Parse output of flatpak list tab-separated command
    pub fn parse_list_output(stdout: &str) -> Vec<FlatpakApp> {
        let mut apps = Vec::new();
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 4 {
                apps.push(FlatpakApp {
                    name: parts[0].trim().to_string(),
                    app_id: parts[1].trim().to_string(),
                    version: parts[2].trim().to_string(),
                    branch: parts[3].trim().to_string(),
                });
            }
        }
        apps
    }

    /// Search for applications on Flathub using their public JSON HTTP API
    pub async fn search(query: &str) -> Result<Vec<FlatpakSearchApp>> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| Client::new());
        
        #[derive(Serialize)]
        struct SearchPayload<'a> {
            query: &'a str,
        }

        let payload = SearchPayload { query };

        let resp: FlathubSearchResponse = client
            .post("https://flathub.org/api/v2/search")
            .header("Content-Type", "application/json")
            .header("User-Agent", "AurumTUI/0.2.0")
            .json(&payload)
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_flatpak_list() {
        let mock_stdout = "Firefox\torg.mozilla.firefox\t126.0\tstable\n\
                           GIMP\torg.gimp.GIMP\t2.10.38\tstable\n\
                           Invalid line without tabs\n\
                           Partial\torg.partial.App\t1.0";
        
        let apps = Flatpak::parse_list_output(mock_stdout);
        assert_eq!(apps.len(), 2);
        
        assert_eq!(apps[0].name, "Firefox");
        assert_eq!(apps[0].app_id, "org.mozilla.firefox");
        assert_eq!(apps[0].version, "126.0");
        assert_eq!(apps[0].branch, "stable");

        assert_eq!(apps[1].name, "GIMP");
        assert_eq!(apps[1].app_id, "org.gimp.GIMP");
        assert_eq!(apps[1].version, "2.10.38");
        assert_eq!(apps[1].branch, "stable");
    }
}
