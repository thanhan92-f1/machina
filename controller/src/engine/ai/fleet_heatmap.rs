// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

use serde::Serialize;
use sqlx::SqlitePool;

// Classification thresholds shared with fleet_power/fleet_rebalance callers,
// which key their recommendations off the "hot"/"cold" labels below.
const HOT_CPU_PERCENT: f32 = 85.0;
const HOT_MEMORY_PERCENT: f32 = 85.0;
const COLD_CPU_PERCENT: f32 = 15.0;
const COLD_MEMORY_PERCENT: f32 = 20.0;
const COLD_MAX_VM_COUNT: i32 = 1;

#[derive(Debug, Serialize)]
pub struct HostHeatCell {
    pub host_id: String,
    pub hostname: String,
    pub cpu_percent: f32,
    pub memory_percent: f32,
    pub vm_count: i32,
    pub classification: String,
}

#[derive(Debug, Serialize)]
pub struct FleetHeatmap {
    pub hosts: Vec<HostHeatCell>,
    pub hotspots: Vec<String>,
    pub cold_hosts: Vec<String>,
    pub power_waste_hosts: Vec<String>,
}

pub async fn heatmap(pool: &SqlitePool) -> anyhow::Result<FleetHeatmap> {
    let rows: Vec<(uuid::Uuid, String, f32, i64, i64, i32, String)> = sqlx::query_as(
        "SELECT id, hostname, cpu_percent, memory_used_mib, memory_total_mib, vm_count, state
         FROM hosts ORDER BY hostname",
    )
    .fetch_all(pool)
    .await?;

    let mut hosts = Vec::new();
    let mut hotspots = Vec::new();
    let mut cold_hosts = Vec::new();
    let mut power_waste_hosts = Vec::new();

    for (id, hostname, cpu, mem_used, mem_total, vm_count, state) in rows {
        let mem_pct = if mem_total > 0 {
            (mem_used as f32 / mem_total as f32) * 100.0
        } else {
            0.0
        };
        let classification = if state != "online" {
            "offline"
        } else if cpu >= HOT_CPU_PERCENT || mem_pct >= HOT_MEMORY_PERCENT {
            "hot"
        } else if cpu < COLD_CPU_PERCENT
            && mem_pct < COLD_MEMORY_PERCENT
            && vm_count <= COLD_MAX_VM_COUNT
        {
            "cold"
        } else {
            "balanced"
        };

        if classification == "hot" {
            hotspots.push(hostname.clone());
        }
        if classification == "cold" {
            cold_hosts.push(hostname.clone());
        }
        if classification == "cold" && vm_count == 0 {
            power_waste_hosts.push(hostname.clone());
        }

        hosts.push(HostHeatCell {
            host_id: id.to_string(),
            hostname,
            cpu_percent: cpu,
            memory_percent: mem_pct,
            vm_count,
            classification: classification.into(),
        });
    }

    Ok(FleetHeatmap {
        hosts,
        hotspots,
        cold_hosts,
        power_waste_hosts,
    })
}
