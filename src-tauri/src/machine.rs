//! Hardware detection and the parallel-jobs suggestion.
use crate::models::MachineInfo;
use sysinfo::System;

pub fn suggest_jobs(physical_cores: u32, total_mem_gb: f64, hd: bool) -> u32 {
    let by_cores = ((physical_cores as f64) / 4.0).round().max(1.0) as u32;
    let by_mem = ((total_mem_gb / 1.5).floor() as u32).max(1);
    let mut n = by_cores.min(by_mem).clamp(1, 4);
    if hd {
        n = (n / 2).max(1);
    }
    n
}

pub fn info() -> MachineInfo {
    let mut sys = System::new();
    sys.refresh_cpu_all();
    sys.refresh_memory();
    let logical = sys.cpus().len() as u32;
    let physical = System::physical_core_count().map(|c| c as u32).unwrap_or(logical.max(1));
    let mem_gb = sys.total_memory() as f64 / 1_073_741_824.0;
    MachineInfo {
        physical_cores: physical,
        logical_cores: logical,
        total_mem_gb: (mem_gb * 10.0).round() / 10.0,
        cpu_brand: sys.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default(),
        os: format!("{} {}", System::name().unwrap_or_default(), System::os_version().unwrap_or_default()),
        suggested_jobs: suggest_jobs(physical, mem_gb, false),
        suggested_jobs_hd: suggest_jobs(physical, mem_gb, true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn suggestions() {
        assert_eq!(suggest_jobs(4, 8.0, false), 1);
        assert_eq!(suggest_jobs(8, 16.0, false), 2);
        assert_eq!(suggest_jobs(10, 32.0, false), 3);
        assert_eq!(suggest_jobs(16, 64.0, false), 4);
        assert_eq!(suggest_jobs(24, 4.0, false), 2);
        assert_eq!(suggest_jobs(10, 32.0, true), 1);
    }
}
