// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

use super::types::{
    ExposureRisk, FirewallPosture, FirewallRule, FirewallScore, OpenPort, ScoreBreakdownItem,
    ScoreRecommendation,
};

// Point deductions for the 0-100 firewall score, ordered roughly by severity.
// Each is also the number of points its matching recommendation offers back.
const PENALTY_FIREWALL_DISABLED: i32 = 25;
const PENALTY_DEFAULT_ALLOW_INBOUND: i32 = 10;
const PENALTY_SSH_PUBLIC: i32 = 8;
const PENALTY_DATABASE_PUBLIC: i32 = 20;
/// Per-port penalty for other critical-risk open ports (only applied when
/// `PENALTY_DATABASE_PUBLIC` didn't already cover the exposure).
const PENALTY_PER_CRITICAL_PORT: i32 = 5;
const PENALTY_DRIFT_DETECTED: i32 = 15;
const RECOMMENDATION_ENABLE_STEALTH_POINTS: i32 = 5;

pub fn compute_firewall_score(
    posture: &FirewallPosture,
    rules: &[FirewallRule],
    ports: &[OpenPort],
) -> FirewallScore {
    let mut score: i32 = 100;
    let mut breakdown = Vec::new();
    let mut recommendations = Vec::new();

    if !posture.enabled {
        score -= PENALTY_FIREWALL_DISABLED;
        breakdown.push(ScoreBreakdownItem {
            category: "firewall_enabled".into(),
            status: "critical".into(),
            points: -PENALTY_FIREWALL_DISABLED,
            detail: "Host firewall is disabled".into(),
        });
        recommendations.push(ScoreRecommendation {
            label: "Enable host firewall".into(),
            points: PENALTY_FIREWALL_DISABLED,
            action: "enable_firewall".into(),
        });
    } else {
        breakdown.push(ScoreBreakdownItem {
            category: "firewall_enabled".into(),
            status: "good".into(),
            points: 0,
            detail: "Firewall is active".into(),
        });
    }

    let default_deny = posture.default_inbound.as_deref() == Some("deny")
        || posture.default_inbound.as_deref() == Some("DROP");
    if posture.enabled && !default_deny {
        score -= PENALTY_DEFAULT_ALLOW_INBOUND;
        breakdown.push(ScoreBreakdownItem {
            category: "default_inbound".into(),
            status: "warning".into(),
            points: -PENALTY_DEFAULT_ALLOW_INBOUND,
            detail: "Default inbound policy is not deny".into(),
        });
        recommendations.push(ScoreRecommendation {
            label: "Set default inbound to deny".into(),
            points: PENALTY_DEFAULT_ALLOW_INBOUND,
            action: "default_deny_inbound".into(),
        });
    }

    let ssh_public = rules.iter().any(|r| {
        r.action == "allow"
            && (r.ports == "22" || r.ports.contains("22"))
            && r.sources
                .iter()
                .any(|s| s == "any" || s == "0.0.0.0/0" || s == "Anywhere")
    }) || ports
        .iter()
        .any(|p| p.port == 22 && p.risk == ExposureRisk::Critical);
    if ssh_public {
        score -= PENALTY_SSH_PUBLIC;
        breakdown.push(ScoreBreakdownItem {
            category: "ssh_exposure".into(),
            status: "warning".into(),
            points: -PENALTY_SSH_PUBLIC,
            detail: "SSH is allowed from anywhere".into(),
        });
        recommendations.push(ScoreRecommendation {
            label: "Restrict SSH to admin subnet".into(),
            points: PENALTY_SSH_PUBLIC,
            action: "restrict_ssh".into(),
        });
    }

    // MySQL, PostgreSQL, Redis, MongoDB default ports.
    let db_public = ports
        .iter()
        .any(|p| matches!(p.port, 3306 | 5432 | 6379 | 27017) && p.risk == ExposureRisk::Critical);
    if db_public {
        score -= PENALTY_DATABASE_PUBLIC;
        breakdown.push(ScoreBreakdownItem {
            category: "database_exposure".into(),
            status: "critical".into(),
            points: -PENALTY_DATABASE_PUBLIC,
            detail: "Database port exposed publicly".into(),
        });
        recommendations.push(ScoreRecommendation {
            label: "Restrict database to app servers only".into(),
            points: PENALTY_DATABASE_PUBLIC,
            action: "restrict_database".into(),
        });
    }

    let critical_ports = ports
        .iter()
        .filter(|p| p.risk == ExposureRisk::Critical)
        .count();
    if critical_ports > 0 && !db_public {
        score -= (critical_ports as i32) * PENALTY_PER_CRITICAL_PORT;
        breakdown.push(ScoreBreakdownItem {
            category: "open_ports".into(),
            status: "warning".into(),
            points: -((critical_ports as i32) * PENALTY_PER_CRITICAL_PORT),
            detail: format!("{critical_ports} critical port exposures"),
        });
    } else if critical_ports == 0 {
        breakdown.push(ScoreBreakdownItem {
            category: "open_ports".into(),
            status: "good".into(),
            points: 0,
            detail: "No critical public port exposures".into(),
        });
    }

    if posture.drift_detected {
        score -= PENALTY_DRIFT_DETECTED;
        breakdown.push(ScoreBreakdownItem {
            category: "drift".into(),
            status: "warning".into(),
            points: -PENALTY_DRIFT_DETECTED,
            detail: "Firewall drift detected outside Zeus".into(),
        });
    }

    if posture.stealth_level == super::types::StealthLevel::Off && posture.enabled {
        recommendations.push(ScoreRecommendation {
            label: "Enable Stealth Mode".into(),
            points: RECOMMENDATION_ENABLE_STEALTH_POINTS,
            action: "enable_stealth".into(),
        });
    }

    FirewallScore {
        score: score.clamp(0, 100) as u32,
        breakdown,
        recommendations,
    }
}
