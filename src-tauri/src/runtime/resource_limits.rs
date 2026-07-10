use std::thread;

use sysinfo::{System, MINIMUM_CPU_UPDATE_INTERVAL};

pub const CPU_USAGE_LIMIT_PERCENT: f32 = 70.0;

/// Samples total system CPU usage twice so the second reading represents an interval.
pub fn is_system_cpu_usage_high() -> bool {
    let mut system = System::new();
    system.refresh_cpu_usage();
    thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL);
    system.refresh_cpu_usage();

    exceeds_cpu_usage_limit(system.global_cpu_usage())
}

fn exceeds_cpu_usage_limit(cpu_usage: f32) -> bool {
    cpu_usage > CPU_USAGE_LIMIT_PERCENT
}
