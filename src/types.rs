use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub maintainer: Option<String>,
    pub url: Option<String>,
    pub votes: u32,
    pub popularity: f64,
    pub last_modified: u64,
    pub out_of_date: Option<u64>,
    pub installed_version: Option<String>,
    pub repository: String, // "aur" or "core/extra/etc"
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Update {
    pub name: String,
    pub old_version: String,
    pub new_version: String,
    pub repository: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RiskLevel {
    Safe,
    Warning,
    Suspicious,
    Dangerous,
}

impl RiskLevel {
    pub fn score(&self) -> u8 {
        match self {
            RiskLevel::Safe => 0,
            RiskLevel::Warning => 30,
            RiskLevel::Suspicious => 60,
            RiskLevel::Dangerous => 100,
        }
    }
    
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            RiskLevel::Safe => (0, 255, 0),
            RiskLevel::Warning => (255, 255, 0),
            RiskLevel::Suspicious => (255, 165, 0),
            RiskLevel::Dangerous => (255, 0, 0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vulnerability {
    pub check_name: String,
    pub description: String,
    pub risk_level: RiskLevel,
    pub line_number: Option<usize>,
    pub line_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScanResult {
    pub package_name: String,
    pub score: u8,
    pub vulnerabilities: Vec<Vulnerability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsItem {
    pub title: String,
    pub link: String,
    pub description: String,
    pub pub_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub name: String,
    pub size_bytes: u64,
    pub last_modified: String,
}
