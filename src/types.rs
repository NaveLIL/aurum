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
    pub size: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct DiskStats {
    pub root_free_bytes: u64,
    pub root_total_bytes: u64,
    pub pacman_cache_bytes: u64,
    pub paru_cache_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct FailedService {
    pub unit: String,
    pub load: String,
    pub active: String,
    pub sub: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SystemInfo {
    pub last_upgrade_days: u32,
    pub pacman_lock_exists: bool,
    pub snapper_available: bool,
    pub lts_kernel_installed: bool,
    pub multiple_kernels_installed: bool,
    pub cachyos_kernel_installed: bool,
    pub is_online: bool,
    pub failed_services_count: u32,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CacheSource {
    Aur,
    Pacman,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CacheEntry {
    pub name: String,
    pub size_bytes: u64,
    pub last_modified: String,
    pub source: CacheSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct CpuMemStats {
    pub cpu_usage: f64,
    pub cpu_temp_c: Option<f64>,
    pub mem_total_bytes: u64,
    pub mem_used_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
}

pub const TEAM_SIG: &str = "EREZ Dev";
