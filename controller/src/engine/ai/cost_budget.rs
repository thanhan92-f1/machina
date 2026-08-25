// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

use serde::Serialize;
use sqlx::SqlitePool;

// A synthetic monthly budget is derived from current spend since there is no
// explicit budget input yet: current spend plus 15% headroom, with a $1000
// floor so a near-empty fleet doesn't get a near-zero budget (and therefore
// a misleadingly high utilization percentage).
const BUDGET_HEADROOM_MULTIPLIER: f64 = 1.15;
const MIN_MONTHLY_BUDGET_USD: f64 = 1000.0;
const UTILIZATION_CRITICAL_PCT: f32 = 95.0;
const UTILIZATION_WARNING_PCT: f32 = 80.0;
const IDLE_VM_ALERT_THRESHOLD: i64 = 2;
// Flag when a meaningful share of spend has no team/project tag attached.
const UNATTRIBUTED_SPEND_FRACTION: f64 = 0.3;
// Firewall exposure overlapping with a noticeable slice of infra spend is
// worth surfacing as a FinOps x Security cross-cutting alert.
const EXPOSURE_INFO_FRACTION: f64 = 0.05;
const EXPOSURE_WARNING_FRACTION: f64 = 0.1;
const IDLE_PORT_WASTE_ALERT_USD: f64 = 50.0;

#[derive(Debug, Serialize)]
pub struct BudgetAlert {
    pub id: String,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct CostBudgetReport {
    pub monthly_budget_usd: f64,
    pub current_spend_usd: f64,
    pub predicted_spend_usd: f64,
    pub utilization_pct: f32,
    pub status: String,
    pub alerts: Vec<BudgetAlert>,
    pub summary: String,
}

pub async fn analyze(pool: &SqlitePool) -> anyhow::Result<CostBudgetReport> {
    let cost = super::cost::analyze(pool).await?;
    let attribution = super::cost_attribution::attribute(pool).await?;

    let monthly_budget_usd =
        (cost.estimated_monthly_usd * BUDGET_HEADROOM_MULTIPLIER).max(MIN_MONTHLY_BUDGET_USD);
    let current_spend_usd = cost.estimated_monthly_usd;
    let predicted_spend_usd = cost.predicted_next_month_usd;
    let utilization_pct = if monthly_budget_usd > 0.0 {
        (current_spend_usd / monthly_budget_usd * 100.0) as f32
    } else {
        0.0
    };

    let mut alerts = Vec::new();
    if utilization_pct >= UTILIZATION_CRITICAL_PCT {
        alerts.push(BudgetAlert {
            id: "budget-critical".into(),
            severity: "critical".into(),
            message: format!("Spend at {:.0}% of monthly budget", utilization_pct),
        });
    } else if utilization_pct >= UTILIZATION_WARNING_PCT {
        alerts.push(BudgetAlert {
            id: "budget-warning".into(),
            severity: "warning".into(),
            message: format!("Spend at {:.0}% of monthly budget", utilization_pct),
        });
    }

    if cost.idle_vm_count > IDLE_VM_ALERT_THRESHOLD {
        alerts.push(BudgetAlert {
            id: "idle-waste".into(),
            severity: "info".into(),
            message: format!("{} idle VMs contributing to waste", cost.idle_vm_count),
        });
    }

    if attribution.unattributed_monthly_usd > current_spend_usd * UNATTRIBUTED_SPEND_FRACTION {
        alerts.push(BudgetAlert {
            id: "unattributed".into(),
            severity: "info".into(),
            message: "Over 30% spend lacks team/project attribution tags".into(),
        });
    }

    let cfg = crate::config::ControllerConfig::default();
    if let Ok(exp) = crate::engine::zeus_firewall::finops::exposure_rollup(pool, &cfg).await {
        if exp.fleet_exposure_monthly_usd > current_spend_usd * EXPOSURE_INFO_FRACTION {
            alerts.push(BudgetAlert {
                id: "firewall-exposure-overlap".into(),
                severity: if exp.fleet_exposure_monthly_usd
                    > current_spend_usd * EXPOSURE_WARNING_FRACTION
                {
                    "warning".into()
                } else {
                    "info".into()
                },
                message: format!(
                    "Firewall exposure est ${:.0}/mo ({:.0}% of infra spend) — FinOps × Security overlap",
                    exp.fleet_exposure_monthly_usd,
                    if current_spend_usd > 0.0 {
                        exp.fleet_exposure_monthly_usd / current_spend_usd * 100.0
                    } else {
                        0.0
                    }
                ),
            });
        }
        if exp.idle_port_waste_usd > IDLE_PORT_WASTE_ALERT_USD {
            alerts.push(BudgetAlert {
                id: "idle-port-waste".into(),
                severity: "info".into(),
                message: format!(
                    "${:.0}/mo idle open port waste across fleet",
                    exp.idle_port_waste_usd
                ),
            });
        }
    }

    let status = if alerts.iter().any(|a| a.severity == "critical") {
        "over_budget"
    } else if alerts.iter().any(|a| a.severity == "warning") {
        "watch"
    } else {
        "on_track"
    };

    let summary = format!(
        "${:.0} / ${:.0} budget ({:.0}%) · predicted ${:.0} next month",
        current_spend_usd, monthly_budget_usd, utilization_pct, predicted_spend_usd
    );

    Ok(CostBudgetReport {
        monthly_budget_usd,
        current_spend_usd,
        predicted_spend_usd,
        utilization_pct,
        status: status.into(),
        alerts,
        summary,
    })
}
