// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

//! Fleet placement scoring helpers (unit-tested, no I/O).

// Label thresholds for the averaged headroom score (0-100 scale).
const CAPACITY_HIGH_THRESHOLD: f64 = 40.0;
const CAPACITY_MEDIUM_THRESHOLD: f64 = 20.0;

// Placement penalty weights: each vCPU costs more headroom than each GiB of
// memory, reflecting that CPU contention degrades a host faster than memory
// pressure alone at typical fleet utilization.
const PLACEMENT_VCPU_PENALTY: f64 = 3.0;
const PLACEMENT_MEMORY_GB_PENALTY: f64 = 2.0;

/// Headroom score from host CPU, memory, and root disk utilization (0–100, higher is better).
pub fn fleet_capacity_score(cpu: f64, mem: f64, disk: f64) -> (f64, &'static str) {
    let score = ((100.0 - cpu).max(0.0) + (100.0 - mem).max(0.0) + (100.0 - disk).max(0.0)) / 3.0;
    let label = if score >= CAPACITY_HIGH_THRESHOLD {
        "high"
    } else if score >= CAPACITY_MEDIUM_THRESHOLD {
        "medium"
    } else {
        "low"
    };
    // Round to one decimal place so tiny floating-point noise doesn't churn the UI.
    ((score * 10.0).round() / 10.0, label)
}

/// Lower score when the requested VM is larger (penalty on headroom).
pub fn placement_adjusted_score(base: f64, vcpus: u32, memory_mb: u64) -> f64 {
    let mem_gb = memory_mb as f64 / 1024.0;
    let penalty = (vcpus as f64 * PLACEMENT_VCPU_PENALTY) + (mem_gb * PLACEMENT_MEMORY_GB_PENALTY);
    ((base - penalty) * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_high_when_host_has_headroom() {
        let (score, label) = fleet_capacity_score(20.0, 25.0, 30.0);
        assert!(score > 40.0);
        assert_eq!(label, "high");
    }

    #[test]
    fn capacity_low_when_host_is_saturated() {
        let (score, label) = fleet_capacity_score(95.0, 92.0, 90.0);
        assert!(score < 20.0);
        assert_eq!(label, "low");
    }

    #[test]
    fn placement_penalizes_large_vm() {
        let (base, _) = fleet_capacity_score(30.0, 30.0, 30.0);
        let small = placement_adjusted_score(base, 1, 1024);
        let large = placement_adjusted_score(base, 16, 65536);
        assert!(large < small);
    }
}
