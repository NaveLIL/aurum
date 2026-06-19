use std::fs;
use anyhow::{Result, anyhow};
use crate::types::CpuMemStats;

pub struct CpuTimes {
    pub idle: u64,
    pub total: u64,
}

pub fn read_cpu_times() -> Result<CpuTimes> {
    let content = fs::read_to_string("/proc/stat")?;
    let cpu_line = content.lines().next().ok_or_else(|| anyhow!("Empty /proc/stat"))?;
    if !cpu_line.starts_with("cpu ") {
        return Err(anyhow!("Invalid /proc/stat format"));
    }
    let parts: Vec<&str> = cpu_line.split_whitespace().collect();
    if parts.len() < 5 {
        return Err(anyhow!("Not enough columns in cpu line"));
    }
    
    let user: u64 = parts[1].parse().unwrap_or(0);
    let nice: u64 = parts[2].parse().unwrap_or(0);
    let system: u64 = parts[3].parse().unwrap_or(0);
    let idle: u64 = parts[4].parse().unwrap_or(0);
    let iowait: u64 = parts.get(5).and_then(|p| p.parse().ok()).unwrap_or(0);
    let irq: u64 = parts.get(6).and_then(|p| p.parse().ok()).unwrap_or(0);
    let softirq: u64 = parts.get(7).and_then(|p| p.parse().ok()).unwrap_or(0);
    let steal: u64 = parts.get(8).and_then(|p| p.parse().ok()).unwrap_or(0);
    
    let idle_time = idle + iowait;
    let total_time = user + nice + system + idle + iowait + irq + softirq + steal;
    
    Ok(CpuTimes { idle: idle_time, total: total_time })
}

pub fn read_mem_stats() -> Result<(u64, u64, u64, u64)> {
    let content = fs::read_to_string("/proc/meminfo")?;
    let mut mem_total = 0;
    let mut mem_avail = 0;
    let mut swap_total = 0;
    let mut swap_free = 0;
    
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let key = parts[0].trim_end_matches(':');
            let val: u64 = parts[1].parse().unwrap_or(0);
            match key {
                "MemTotal" => mem_total = val * 1024,
                "MemAvailable" => mem_avail = val * 1024,
                "SwapTotal" => swap_total = val * 1024,
                "SwapFree" => swap_free = val * 1024,
                _ => {}
            }
        }
    }
    
    let mem_used = mem_total.saturating_sub(mem_avail);
    let swap_used = swap_total.saturating_sub(swap_free);
    
    Ok((mem_total, mem_used, swap_total, swap_used))
}

pub fn read_cpu_temp() -> Option<f64> {
    // 1. Try hwmon devices with preferred names (coretemp, k10temp, zenpower)
    if let Ok(entries) = fs::read_dir("/sys/class/hwmon") {
        let mut fallback_temp = None;
        for entry in entries.flatten() {
            let path = entry.path();
            if let Ok(name) = fs::read_to_string(path.join("name")) {
                let name = name.trim();
                if name == "coretemp" || name == "k10temp" || name == "zenpower" {
                    // Try to find temp1_input or any temp*_input
                    // Typically temp1_input is Package id 0 or Tdie
                    if let Ok(temp_str) = fs::read_to_string(path.join("temp1_input")) {
                        if let Ok(temp_val) = temp_str.trim().parse::<f64>() {
                            return Some(temp_val / 1000.0);
                        }
                    }
                    // If temp1_input failed, try to find any temp*_input
                    if let Ok(subentries) = fs::read_dir(&path) {
                        for subentry in subentries.flatten() {
                            let subpath = subentry.path();
                            if let Some(filename) = subpath.file_name().and_then(|n| n.to_str()) {
                                if filename.starts_with("temp") && filename.ends_with("_input") {
                                    if let Ok(temp_str) = fs::read_to_string(&subpath) {
                                        if let Ok(temp_val) = temp_str.trim().parse::<f64>() {
                                            return Some(temp_val / 1000.0);
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if name == "dell_smm" {
                    // dell_smm is also a good CPU temp sensor fallback
                    if let Ok(temp_str) = fs::read_to_string(path.join("temp1_input")) {
                        if let Ok(temp_val) = temp_str.trim().parse::<f64>() {
                            fallback_temp = Some(temp_val / 1000.0);
                        }
                    }
                }
            }
        }
        if fallback_temp.is_some() {
            return fallback_temp;
        }
    }

    // 2. Try thermal zones with preferred types
    if let Ok(entries) = fs::read_dir("/sys/class/thermal") {
        let mut acpi_fallback = None;
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.starts_with("thermal_zone") {
                    if let Ok(type_str) = fs::read_to_string(path.join("type")) {
                        let type_str = type_str.trim().to_lowercase();
                        if type_str == "x86_pkg_temp" || type_str.contains("cpu") {
                            if let Ok(temp_str) = fs::read_to_string(path.join("temp")) {
                                if let Ok(temp_val) = temp_str.trim().parse::<f64>() {
                                    return Some(temp_val / 1000.0);
                                }
                            }
                        } else if type_str == "acpitz" {
                            if let Ok(temp_str) = fs::read_to_string(path.join("temp")) {
                                if let Ok(temp_val) = temp_str.trim().parse::<f64>() {
                                    acpi_fallback = Some(temp_val / 1000.0);
                                }
                            }
                        }
                    }
                }
            }
        }
        if acpi_fallback.is_some() {
            return acpi_fallback;
        }
    }

    None
}

pub async fn get_cpu_mem_stats(prev_cpu: &mut Option<(u64, u64)>) -> Result<CpuMemStats> {
    let cpu_times = read_cpu_times()?;
    let cpu_usage = if let Some((prev_idle, prev_total)) = prev_cpu {
        let total_delta = cpu_times.total.saturating_sub(*prev_total);
        let idle_delta = cpu_times.idle.saturating_sub(*prev_idle);
        if total_delta > 0 {
            100.0 * (1.0 - (idle_delta as f64 / total_delta as f64))
        } else {
            0.0
        }
    } else {
        0.0
    };
    
    *prev_cpu = Some((cpu_times.idle, cpu_times.total));
    
    let (mem_total, mem_used, swap_total, swap_used) = read_mem_stats()?;
    let cpu_temp_c = read_cpu_temp();
    
    Ok(CpuMemStats {
        cpu_usage,
        cpu_temp_c,
        mem_total_bytes: mem_total,
        mem_used_bytes: mem_used,
        swap_total_bytes: swap_total,
        swap_used_bytes: swap_used,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_cpu_times() {
        let res = read_cpu_times();
        assert!(res.is_ok(), "Failed to read CPU times: {:?}", res.err());
        let times = res.unwrap();
        assert!(times.total > 0, "Total CPU time should be greater than 0");
        assert!(times.idle <= times.total, "Idle time should not exceed total time");
    }

    #[test]
    fn test_read_mem_stats() {
        let res = read_mem_stats();
        assert!(res.is_ok(), "Failed to read memory stats: {:?}", res.err());
        let (mem_total, mem_used, swap_total, swap_used) = res.unwrap();
        assert!(mem_total > 0, "Total memory should be greater than 0");
        assert!(mem_used <= mem_total, "Used memory should not exceed total memory");
        assert!(swap_used <= swap_total, "Used swap should not exceed total swap");
    }

    #[test]
    fn test_read_cpu_temp() {
        let temp = read_cpu_temp();
        if let Some(t) = temp {
            assert!(t > -50.0 && t < 150.0, "CPU temperature should be in a reasonable range, got: {}", t);
        }
    }
}

