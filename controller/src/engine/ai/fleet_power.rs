// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

use serde::Serialize;
use sqlx::SqlitePool;

// Fallback FinOps rates when no cluster row carries pricing yet (approx.
// on-demand cloud vCPU/GiB pricing at the time these defaults were chosen).
const DEFAULT_VCPU_HOUR_USD: f64 = 0.02;
const DEFAULT_GIB_HOUR_USD: f64 = 0.005;
// Assumed idle-host shape used only to size the savings estimate: a
// mid-range 16 vCPU / 64 GiB node running nearly full-time in a month.
const IDLE_HOST_VCPUS: f64 = 16.0;
const IDLE_HOST_GIB: f64 = 64.0;
const HOURS_PER_MONTH: f64 = 730.0;
// Estimated fraction of a cold host's cost that is recoverable by
// consolidating/powering it down (rest is fixed overhead that persists).
const IDLE_HOST_SAVINGS_FRACTION: f64 = 0.15;
// Hotspot rebalancing recovers a smaller share since VMs are migrated, not
// powered off — the host keeps running for whatever remains.
const HOTSPOT_REBALANCE_SAVINGS_FRACTION: f64 = 0.4;
// Only the top-N hotspots get a rebalance recommendation to avoid flooding
// the report when many hosts are mildly over-utilized.
const TOP_HOTSPOTS_LIMIT: usize = 3;

#[derive(Debug, Serialize)]
pub struct PowerOptimization {
    pub host: String,
    pub action: String,
    pub estimated_savings_usd_month: f64,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct FleetPowerReport {
    pub optimizations: Vec<PowerOptimization>,
    pub total_savings_usd_month: f64,
    pub summary: String,
}

pub async fn optimize(pool: &SqlitePool) -> anyhow::Result<FleetPowerReport> {
    let heat = super::fleet_heatmap::heatmap(pool).await?;
    let rates: (f64, f64) = sqlx::query_as(
        "SELECT finops_vcpu_hour_usd, finops_gib_hour_usd FROM clusters ORDER BY created_at LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .unwrap_or((DEFAULT_VCPU_HOUR_USD, DEFAULT_GIB_HOUR_USD));

    let idle_host_monthly = (IDLE_HOST_VCPUS * rates.0 + IDLE_HOST_GIB * rates.1)
        * HOURS_PER_MONTH
        * IDLE_HOST_SAVINGS_FRACTION;

    let mut optimizations = Vec::new();
    for host in &heat.power_waste_hosts {
        optimizations.push(PowerOptimization {
            host: host.clone(),
            action: "consolidate_or_power_down".into(),
            estimated_savings_usd_month: idle_host_monthly,
            reason: "Cold host with zero VMs — candidate for consolidation or power-down.".into(),
        });
    }

    for host in heat.hotspots.iter().take(TOP_HOTSPOTS_LIMIT) {
        optimizations.push(PowerOptimization {
            host: host.clone(),
            action: "rebalance_vms".into(),
            estimated_savings_usd_month: idle_host_monthly * HOTSPOT_REBALANCE_SAVINGS_FRACTION,
            reason: "Hotspot host — live-migrate VMs to cold nodes.".into(),
        });
    }

    let total_savings_usd_month = optimizations
        .iter()
        .map(|o| o.estimated_savings_usd_month)
        .sum();

    let summary = if optimizations.is_empty() {
        "Fleet power profile balanced — no waste optimizations.".into()
    } else {
        format!(
            "{} optimization(s) · ~${:.0}/mo estimated savings",
            optimizations.len(),
            total_savings_usd_month
        )
    };

    Ok(FleetPowerReport {
        optimizations,
        total_savings_usd_month,
        summary,
    })
}
