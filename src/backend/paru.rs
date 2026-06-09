use tokio::process::Command;
use crate::types::{Package, Update, CacheEntry, DiskStats, SystemInfo};
use anyhow::{Result, Context};
use regex::Regex;
use dirs;
use std::sync::OnceLock;

static UPDATE_RE: OnceLock<Regex> = OnceLock::new();

async fn run_command_with_timeout(mut cmd: Command, timeout_dur: std::time::Duration) -> Result<std::process::Output> {
    cmd.kill_on_drop(true);
    tokio::time::timeout(timeout_dur, cmd.output())
        .await
        .context("Command timed out")?
        .context("Command execution failed")
}

pub struct Paru;

impl Paru {
    pub async fn get_updates() -> Result<Vec<Update>> {
        let mut updates = Vec::new();
        let re = UPDATE_RE.get_or_init(|| Regex::new(r"^(\S+)\s+(\S+)\s+->\s+(\S+)").unwrap());

        // 1. Fetch official repo updates via checkupdates
        let cmd = Command::new("checkupdates");
        match run_command_with_timeout(cmd, std::time::Duration::from_secs(10)).await {
            Ok(output) => {
                if output.status.success() || output.status.code() == Some(2) {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    for line in stdout.lines() {
                        if let Some(caps) = re.captures(line) {
                            updates.push(Update {
                                name: caps[1].to_string(),
                                old_version: caps[2].to_string(),
                                new_version: caps[3].to_string(),
                                repository: "repo".to_string(),
                            });
                        }
                    }
                }
            }
            Err(_) => {
                // checkupdates not installed or failed, fallback to none
            }
        }

        // 2. Fetch AUR updates via paru -Qua
        let mut cmd = Command::new("paru");
        cmd.arg("-Qua");
        if let Ok(output) = run_command_with_timeout(cmd, std::time::Duration::from_secs(10)).await {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
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
            }
        }

        updates.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(updates)
    }

    pub async fn get_installed() -> Result<Vec<Package>> {
        let mut cmd = Command::new("paru");
        cmd.arg("-Qm");
        let output = run_command_with_timeout(cmd, std::time::Duration::from_secs(10))
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
                    size: None,
                });
            }
        }

        Ok(packages)
    }

    pub async fn get_info(pkg_name: &str) -> Result<Option<Package>> {
        // Try local info first (installed packages)
        let mut cmd = Command::new("paru");
        cmd.args(["-Qi", pkg_name]);
        let output = run_command_with_timeout(cmd, std::time::Duration::from_secs(10))
            .await
            .context("Failed to execute paru -Qi")?;

        let stdout = if output.status.success() {
            String::from_utf8(output.stdout)?
        } else {
            // Try remote info
            let mut cmd = Command::new("paru");
            cmd.args(["-Si", pkg_name]);
            let output = run_command_with_timeout(cmd, std::time::Duration::from_secs(10))
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
        let mut cmd = Command::new("paru");
        cmd.arg("-Gp").arg(pkg_name);
        let output = run_command_with_timeout(cmd, std::time::Duration::from_secs(10))
            .await
            .context("Failed to get PKGBUILD")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to fetch PKGBUILD"));
        }

        Ok(String::from_utf8(output.stdout)?)
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
            use std::path::Component;
            let path_name = std::path::Path::new(&name);
            for component in path_name.components() {
                match component {
                    Component::Normal(_) => {},
                    _ => return Err(anyhow::anyhow!("Invalid cache entry name: safety check failed")),
                }
            }

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
        let mut cmd = Command::new("paru");
        cmd.arg("-Qdtq");
        let output = run_command_with_timeout(cmd, std::time::Duration::from_secs(10))
            .await
            .context("Failed to execute paru -Qdtq")?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8(output.stdout)?;
        let orphan_names: Vec<String> = stdout
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();

        let mut packages = Vec::new();
        if orphan_names.is_empty() {
            return Ok(packages);
        }

        // Run pacman -Qi with LC_ALL=C for all orphan packages to get sizes & descriptions in a single call
        let mut qi_cmd = Command::new("pacman");
        qi_cmd.arg("-Qi").args(&orphan_names).env("LC_ALL", "C");
        let qi_output = run_command_with_timeout(qi_cmd, std::time::Duration::from_secs(10))
            .await
            .context("Failed to run pacman -Qi")?;

        if qi_output.status.success() {
            let qi_stdout = String::from_utf8_lossy(&qi_output.stdout);
            let mut current_name = String::new();
            let mut current_version = String::new();
            let mut current_size = String::new();
            let mut current_desc = String::new();

            for line in qi_stdout.lines() {
                let line_trimmed = line.trim();
                if line_trimmed.starts_with("Name            :") {
                    if !current_name.is_empty() {
                        packages.push(Package {
                            name: current_name.clone(),
                            version: current_version.clone(),
                            description: Some(current_desc.clone()),
                            maintainer: None,
                            url: None,
                            votes: 0,
                            popularity: 0.0,
                            last_modified: 0,
                            out_of_date: None,
                            installed_version: Some(current_version.clone()),
                            repository: "orphan".to_string(),
                            size: Some(current_size.clone()),
                        });
                    }
                    current_name = line_trimmed.split(':').nth(1).unwrap_or("").trim().to_string();
                    current_version.clear();
                    current_size.clear();
                    current_desc.clear();
                } else if line_trimmed.starts_with("Version         :") {
                    current_version = line_trimmed.split(':').nth(1).unwrap_or("").trim().to_string();
                } else if line_trimmed.starts_with("Description     :") {
                    current_desc = line_trimmed.split(':').nth(1).unwrap_or("").trim().to_string();
                } else if line_trimmed.starts_with("Installed Size  :") {
                    current_size = line_trimmed.split(':').nth(1).unwrap_or("").trim().to_string();
                }
            }
            if !current_name.is_empty() {
                packages.push(Package {
                    name: current_name,
                    version: current_version.clone(),
                    description: Some(current_desc),
                    maintainer: None,
                    url: None,
                    votes: 0,
                    popularity: 0.0,
                    last_modified: 0,
                    out_of_date: None,
                    installed_version: Some(current_version),
                    repository: "orphan".to_string(),
                    size: Some(current_size),
                });
            }
        } else {
            // Fallback to basic list if pacman -Qi fails
            for name in orphan_names {
                packages.push(Package {
                    name,
                    version: "unknown".to_string(),
                    description: None,
                    maintainer: None,
                    url: None,
                    votes: 0,
                    popularity: 0.0,
                    last_modified: 0,
                    out_of_date: None,
                    installed_version: Some("unknown".to_string()),
                    repository: "orphan".to_string(),
                    size: None,
                });
            }
        }

        Ok(packages)
    }

    pub async fn get_disk_stats() -> Result<DiskStats> {
        let mut df_cmd = Command::new("df");
        df_cmd.arg("-B1").arg("--output=avail,size").arg("/").env("LC_ALL", "C");
        let df_output = run_command_with_timeout(df_cmd, std::time::Duration::from_secs(5))
            .await?;

        let mut free_bytes = 0;
        let mut total_bytes = 0;

        if df_output.status.success() {
            let stdout = String::from_utf8_lossy(&df_output.stdout);
            let lines: Vec<&str> = stdout.lines().collect();
            if lines.len() >= 2 {
                let parts: Vec<&str> = lines[1].split_whitespace().collect();
                if parts.len() >= 2 {
                    free_bytes = parts[0].parse().unwrap_or(0);
                    total_bytes = parts[1].parse().unwrap_or(0);
                }
            }
        }

        let pacman_cache = tokio::task::spawn_blocking(move || {
            let path = std::path::Path::new("/var/cache/pacman/pkg");
            dir_size_safe(path)
        }).await.unwrap_or(0);

        let paru_cache = tokio::task::spawn_blocking(move || {
            let cache_dir = dirs::cache_dir().map(|d| d.join("paru"));
            cache_dir.map_or(0, |p| dir_size_safe(&p))
        }).await.unwrap_or(0);

        Ok(DiskStats {
            root_free_bytes: free_bytes,
            root_total_bytes: total_bytes,
            pacman_cache_bytes: pacman_cache,
            paru_cache_bytes: paru_cache,
        })
    }

    pub async fn get_kernel_info() -> Result<(Vec<String>, bool, bool)> {
        let kernel_list = [
            "linux",
            "linux-lts",
            "linux-zen",
            "linux-hardened",
            "linux-rt",
            "linux-rt-lts",
            "linux-cachyos",
            "linux-cachyos-lts",
            "linux-cachyos-zen",
            "linux-cachyos-hardened",
            "linux-cachyos-rt",
            "linux-cachyos-bore",
            "linux-cachyos-rc",
            "linux-cachyos-sched-ext",
            "linux-cachyos-rc-sched-ext",
        ];

        let mut cmd = Command::new("pacman");
        cmd.arg("-Qq").args(&kernel_list);
        let output = run_command_with_timeout(cmd, std::time::Duration::from_secs(5))
            .await;

        let mut installed_kernels = Vec::new();
        let mut lts_installed = false;
        let mut cachyos_installed = false;

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                let name = line.trim().to_string();
                if !name.is_empty() {
                    if name.contains("-lts") {
                        lts_installed = true;
                    }
                    if name.contains("cachyos") {
                        cachyos_installed = true;
                    }
                    installed_kernels.push(name);
                }
            }
        }

        Ok((installed_kernels, lts_installed, cachyos_installed))
    }

    pub async fn get_system_info() -> Result<SystemInfo> {
        let mut last_upgrade_days = 0;
        let pacman_log_path = std::path::Path::new("/var/log/pacman.log");

        if let Ok(metadata) = std::fs::metadata(pacman_log_path) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(elapsed) = modified.elapsed() {
                    last_upgrade_days = (elapsed.as_secs() / (24 * 3600)) as u32;
                }
            }
        }

        let pacman_lock_exists = std::path::Path::new("/var/lib/pacman/db.lck").exists();

        // Check snapper availability and configuration
        let snapper_available = tokio::task::spawn_blocking(move || {
            let snapper_exists = std::process::Command::new("which")
                .arg("snapper")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

            if !snapper_exists {
                return false;
            }

            // check if 'root' config is active in snapper list-configs
            let output = std::process::Command::new("snapper")
                .arg("list-configs")
                .output();

            if let Ok(out) = output {
                if out.status.success() {
                    let text = String::from_utf8_lossy(&out.stdout);
                    return text.contains("root");
                }
            }
            false
        }).await.unwrap_or(false);

        let (installed_kernels, lts_kernel_installed, cachyos_kernel_installed) = Self::get_kernel_info().await.unwrap_or((Vec::new(), false, false));
        let multiple_kernels_installed = installed_kernels.len() >= 2;

        Ok(SystemInfo {
            last_upgrade_days,
            pacman_lock_exists,
            snapper_available,
            lts_kernel_installed,
            multiple_kernels_installed,
            cachyos_kernel_installed,
            is_online: true,
        })
    }
}

fn dir_size_safe(path: &std::path::Path) -> u64 {
    let mut total: u64 = 0;
    if path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries {
                if let Ok(entry) = entry {
                    if let Ok(ft) = entry.file_type() {
                        if ft.is_file() {
                            if let Ok(meta) = entry.metadata() {
                                total += meta.len();
                            }
                        } else if ft.is_dir() {
                            total += dir_size_safe(&entry.path());
                        }
                    }
                }
            }
        }
    }
    total
}

fn dir_size(path: &std::path::Path) -> Result<u64> {
    Ok(dir_size_safe(path))
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

