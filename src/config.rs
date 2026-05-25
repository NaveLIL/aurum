use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;
use anyhow::Result;
use directories::ProjectDirs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub check_interval_minutes: u64,
    pub aur_rpc_url: String,
    pub max_cache_size_mb: u64,
    pub risky_patterns: Vec<String>,
    pub theme: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            check_interval_minutes: 60,
            aur_rpc_url: "https://aur.archlinux.org/rpc/".to_string(),
            max_cache_size_mb: 5000,
            risky_patterns: vec![
                r"rm\s+-rf\s+.*".to_string(),
                r"curl\s+.*\|\s*sh".to_string(),
                r"wget\s+.*\|\s*sh".to_string(),
                r"eval\s+".to_string(),
                r"base64\s+-d".to_string(),
                r"sudo\s+".to_string(),
            ],
            theme: "default".to_string(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path()?;
        
        if !config_path.exists() {
            let config = Config::default();
            config.save()?;
            return Ok(config);
        }

        let content = fs::read_to_string(config_path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path()?;
        let config_dir = config_path.parent().unwrap();
        
        if !config_dir.exists() {
            fs::create_dir_all(config_dir)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(config_path, content)?;
        Ok(())
    }

    fn get_config_path() -> Result<PathBuf> {
        if let Some(proj_dirs) = ProjectDirs::from("org", "aurum", "dashboard") {
            Ok(proj_dirs.config_dir().join("config.json"))
        } else {
            // Fallback to local directory if we can't determine config dir
            Ok(PathBuf::from("config.json"))
        }
    }
}
