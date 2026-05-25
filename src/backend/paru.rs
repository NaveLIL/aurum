use tokio::process::Command;
use crate::types::{Package, Update, CacheEntry};
use anyhow::{Result, Context};
use regex::Regex;
use dirs;

pub struct Paru;

impl Paru {
    pub async fn get_updates() -> Result<Vec<Update>> {
        let output = Command::new("paru")
            .arg("-Qua")
            .output()
            .await
            .context("Failed to execute paru -Qua")?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8(output.stdout)?;
        let mut updates = Vec::new();
        let re = Regex::new(r"^(\S+)\s+(\S+)\s+->\s+(\S+)")?;

        for line in stdout.lines() {
            if let Some(caps) = re.captures(line) {
                updates.push(Update {
                    name: caps[1].to_string(),
                    old_version: caps[2].to_string(),
                    new_version: caps[3].to_string(),
                    repository: "aur".to_string(),
                });
            }
        }

        Ok(updates)
    }

    pub async fn get_installed() -> Result<Vec<Package>> {
        let output = Command::new("paru")
            .arg("-Qm")
            .output()
            .await
            .context("Failed to execute paru -Qm")?;

        let stdout = String::from_utf8(output.stdout)?;
        let mut packages = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                packages.push(Package {
                    name: parts[0].to_string(),
                    version: parts[1].to_string(),
                    description: None,
                    maintainer: None,
                    url: None,
                    votes: 0,
                    popularity: 0.0,
                    last_modified: 0,
                    out_of_date: None,
                    installed_version: Some(parts[1].to_string()),
                    repository: "aur".to_string(),
                });
            }
        }

        Ok(packages)
    }

    pub async fn get_info(pkg_name: &str) -> Result<Option<Package>> {
        // Try local info first (installed packages)
        let output = Command::new("paru")
            .args(["-Qi", pkg_name])
            .output()
            .await
            .context("Failed to execute paru -Qi")?;

        let stdout = if output.status.success() {
            String::from_utf8(output.stdout)?
        } else {
            // Try remote info
            let output = Command::new("paru")
                .args(["-Si", pkg_name])
                .output()
                .await
                .context("Failed to execute paru -Si")?;
            if !output.status.success() {
                return Ok(None);
            }
            String::from_utf8(output.stdout)?
        };

        let mut pkg = Package {
            name: pkg_name.to_string(),
            ..Package::default()
        };

        for line in stdout.lines() {
            let line = line.trim();
            if let Some((key, val)) = line.split_once(':') {
                let key = key.trim();
                let val = val.trim();
                match key {
                    "Name" => pkg.name = val.to_string(),
                    "Version" => pkg.version = val.to_string(),
                    "Description" => pkg.description = Some(val.to_string()),
                    "URL" => pkg.url = Some(val.to_string()),
                    "Maintainer" | "Packager" => pkg.maintainer = Some(val.to_string()),
                    "Repository" => pkg.repository = val.to_string(),
                    _ => {}
                }
            }
        }

        Ok(Some(pkg))
    }

    pub async fn get_pkgbuild(pkg_name: &str) -> Result<String> {
        let output = Command::new("paru")
            .arg("-Gp")
            .arg(pkg_name)
            .output()
            .await
            .context("Failed to get PKGBUILD")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to fetch PKGBUILD"));
        }

        Ok(String::from_utf8(output.stdout)?)
    }

    pub async fn get_cache_entries() -> Result<Vec<String>> {
        tokio::task::spawn_blocking(move || {
            let cache_dir = dirs::cache_dir()
                .ok_or_else(|| anyhow::anyhow!("No cache dir"))?
                .join("paru/clone");

            if !cache_dir.exists() {
                return Ok(Vec::new());
            }

            let mut entries = Vec::new();
            for entry in std::fs::read_dir(cache_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    if let Ok(name) = entry.file_name().into_string() {
                        entries.push(name);
                    }
                }
            }
            entries.sort();
            Ok(entries)
        })
        .await
        .context("spawn_blocking failed")?
    }

    pub async fn get_cache_entries_with_size() -> Result<Vec<CacheEntry>> {
        tokio::task::spawn_blocking(move || {
            let cache_dir = dirs::cache_dir()
                .ok_or_else(|| anyhow::anyhow!("No cache dir"))?
                .join("paru/clone");

            if !cache_dir.exists() {
                return Ok(Vec::new());
            }

            let mut entries = Vec::new();
            for entry in std::fs::read_dir(&cache_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    let name = match entry.file_name().into_string() {
                        Ok(n) => n,
                        Err(_) => continue,
                    };

                    // Calculate dir size
                    let size = dir_size(&entry.path()).unwrap_or(0);

                    let modified = entry.metadata()
                        .and_then(|m| m.modified())
                        .map(|t| {
                            let duration = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                            chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
                                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                                .unwrap_or_else(|| "unknown".to_string())
                        })
                        .unwrap_or_else(|_| "unknown".to_string());

                    entries.push(CacheEntry {
                        name,
                        size_bytes: size,
                        last_modified: modified,
                    });
                }
            }
            entries.sort_by(|a, b| a.name.cmp(&b.name));
            Ok(entries)
        })
        .await
        .context("spawn_blocking failed")?
    }

    pub async fn clean_cache(name: String) -> Result<()> {
        tokio::task::spawn_blocking(move || {
            let cache_dir = dirs::cache_dir()
                .ok_or_else(|| anyhow::anyhow!("No cache dir"))?
                .join("paru/clone")
                .join(name);

            if cache_dir.exists() {
                std::fs::remove_dir_all(&cache_dir)?;
            }
            Ok(())
        })
        .await
        .context("spawn_blocking failed")?
    }

    pub async fn clean_all_cache() -> Result<()> {
        tokio::task::spawn_blocking(move || {
            let cache_dir = dirs::cache_dir()
                .ok_or_else(|| anyhow::anyhow!("No cache dir"))?
                .join("paru/clone");

            if cache_dir.exists() {
                for entry in std::fs::read_dir(&cache_dir)? {
                    let entry = entry?;
                    if entry.file_type()?.is_dir() {
                        std::fs::remove_dir_all(entry.path())?;
                    }
                }
            }
            Ok(())
        })
        .await
        .context("spawn_blocking failed")?
    }

    pub async fn get_orphans() -> Result<Vec<Package>> {
        let output = Command::new("paru")
            .arg("-Qdt")
            .output()
            .await
            .context("Failed to execute paru -Qdt")?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8(output.stdout)?;
        let mut packages = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                packages.push(Package {
                    name: parts[0].to_string(),
                    version: parts[1].to_string(),
                    description: None,
                    maintainer: None,
                    url: None,
                    votes: 0,
                    popularity: 0.0,
                    last_modified: 0,
                    out_of_date: None,
                    installed_version: Some(parts[1].to_string()),
                    repository: "orphan".to_string(),
                });
            }
        }

        Ok(packages)
    }
}

fn dir_size(path: &std::path::Path) -> Result<u64> {
    let mut total: u64 = 0;
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            if ft.is_file() {
                total += entry.metadata()?.len();
            } else if ft.is_dir() {
                total += dir_size(&entry.path())?;
            }
        }
    }
    Ok(total)
}

/// Format bytes into human-readable format
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
