// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! Real forecasting. A leader-only recorder samples VM memory/CPU and storage-pool fill every five minutes into
//! `metric_samples`; `time_to_threshold` fits a straight line to the recent samples and says when the metric
//! crosses a limit, with a confidence derived from how well the line fits and how much history there is.
//! Too little history means "no forecast", never a made-up number.

use std::time::Duration;

use sqlx::SqlitePool;

use crate::state::AppState;

const SAMPLE_EVERY: Duration = Duration::from_secs(300);
const KEEP_SECS: i64 = 14 * 24 * 3600;
/// Fewest samples, and shortest time span, before a trend is worth showing.
pub const MIN_SAMPLES: usize = 12;
pub const MIN_SPAN_HOURS: f64 = 1.0;
/// Forecasts further out than this are not actionable.
pub const MAX_HORIZON_HOURS: f64 = 24.0 * 30.0;

pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(SAMPLE_EVERY).await;
            if !state.leader.is_leader() {
                continue;
            }
            if let Err(e) = record_samples(&state.pool).await {
                tracing::warn!("forecast sample recording failed: {e:#}");
            }
        }
    });
}

pub async fn record_samples(pool: &SqlitePool) -> anyhow::Result<()> {
    let now = chrono::Utc::now().timestamp();
    sqlx::query(
        "INSERT OR IGNORE INTO metric_samples (subject, metric, ts, value)
         SELECT v.id, 'mem_ratio', ?, m.memory_used_mib * 1.0 / v.memory_mib
         FROM vms v JOIN vm_metrics m ON m.vm_id = v.id
         WHERE v.observed_state = 'running' AND v.memory_mib > 0",
    )
    .bind(now)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT OR IGNORE INTO metric_samples (subject, metric, ts, value)
         SELECT v.id, 'cpu_percent', ?, m.cpu_percent
         FROM vms v JOIN vm_metrics m ON m.vm_id = v.id
         WHERE v.observed_state = 'running'",
    )
    .bind(now)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT OR IGNORE INTO metric_samples (subject, metric, ts, value)
         SELECT 'pool:' || id, 'pool_used_ratio', ?, used_gib * 1.0 / capacity_gib
         FROM storage_pools WHERE capacity_gib > 0",
    )
    .bind(now)
    .execute(pool)
    .await?;
    sqlx::query("DELETE FROM metric_samples WHERE ts < ?")
        .bind(now - KEEP_SECS)
        .execute(pool)
        .await?;
    Ok(())
}

/// Recent samples for one subject+metric as (epoch seconds, value), oldest first.
pub async fn series(
    pool: &SqlitePool,
    subject: &str,
    metric: &str,
    window_hours: i64,
) -> anyhow::Result<Vec<(i64, f64)>> {
    let since = chrono::Utc::now().timestamp() - window_hours * 3600;
    Ok(sqlx::query_as(
        "SELECT ts, value FROM metric_samples WHERE subject = ? AND metric = ? AND ts >= ? ORDER BY ts",
    )
    .bind(subject)
    .bind(metric)
    .bind(since)
    .fetch_all(pool)
    .await?)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trend {
    /// Change per hour.
    pub slope: f64,
    pub intercept: f64,
    pub r2: f64,
}

/// Least-squares line through (hours, value) points. None for fewer than 2 points or no spread in time.
pub fn linreg(points: &[(f64, f64)]) -> Option<Trend> {
    let n = points.len() as f64;
    if points.len() < 2 {
        return None;
    }
    let (sx, sy) = points
        .iter()
        .fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
    let (mx, my) = (sx / n, sy / n);
    let sxx: f64 = points.iter().map(|p| (p.0 - mx).powi(2)).sum();
    if sxx < 1e-12 {
        return None;
    }
    let sxy: f64 = points.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let slope = sxy / sxx;
    let intercept = my - slope * mx;
    let ss_tot: f64 = points.iter().map(|p| (p.1 - my).powi(2)).sum();
    let ss_res: f64 = points
        .iter()
        .map(|p| (p.1 - (slope * p.0 + intercept)).powi(2))
        .sum();
    let r2 = if ss_tot < 1e-12 {
        0.0
    } else {
        (1.0 - ss_res / ss_tot).clamp(0.0, 1.0)
    };
    Some(Trend {
        slope,
        intercept,
        r2,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Crossing {
    pub hours: f64,
    pub confidence: f64,
    /// Fitted rise per day, in the metric's own unit.
    pub per_day: f64,
}

/// When will the metric reach `threshold`? `samples` are (epoch seconds, value). None when there is too little
/// history, the trend is flat or falling, the fit is poor, or the crossing is beyond the useful horizon.
pub fn time_to_threshold(samples: &[(i64, f64)], threshold: f64) -> Option<Crossing> {
    if samples.len() < MIN_SAMPLES {
        return None;
    }
    let t0 = samples.first()?.0;
    let pts: Vec<(f64, f64)> = samples
        .iter()
        .map(|(t, v)| ((*t - t0) as f64 / 3600.0, *v))
        .collect();
    let span = pts.last()?.0;
    if span < MIN_SPAN_HOURS {
        return None;
    }
    let tr = linreg(&pts)?;
    if tr.slope <= 1e-9 || tr.r2 < 0.5 {
        return None;
    }
    let now_fit = tr.slope * span + tr.intercept;
    if now_fit >= threshold {
        return Some(Crossing {
            hours: 0.0,
            confidence: tr.r2,
            per_day: tr.slope * 24.0,
        });
    }
    let hours = (threshold - now_fit) / tr.slope;
    if hours > MAX_HORIZON_HOURS {
        return None;
    }
    // Fit quality, discounted when there is little history to fit.
    let history = (samples.len() as f64 / 48.0).min(1.0);
    let confidence = (tr.r2 * (0.6 + 0.4 * history) * 100.0).round() / 100.0;
    Some(Crossing {
        hours,
        confidence,
        per_day: tr.slope * 24.0,
    })
}

/// "~6 days" / "~14 hours" / "under an hour".
pub fn humanize_hours(h: f64) -> String {
    if h < 1.0 {
        "under an hour".into()
    } else if h < 48.0 {
        format!("~{:.0} hours", h)
    } else {
        format!("~{:.0} days", h / 24.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(n: usize, step_secs: i64, start: f64, per_hour: f64) -> Vec<(i64, f64)> {
        (0..n)
            .map(|i| {
                let t = i as i64 * step_secs;
                (t, start + per_hour * (t as f64 / 3600.0))
            })
            .collect()
    }

    #[test]
    fn linreg_recovers_slope_and_perfect_fit() {
        let pts: Vec<(f64, f64)> = (0..10).map(|i| (i as f64, 2.0 * i as f64 + 1.0)).collect();
        let t = linreg(&pts).unwrap();
        assert!((t.slope - 2.0).abs() < 1e-9 && (t.intercept - 1.0).abs() < 1e-9 && t.r2 > 0.999);
    }

    #[test]
    fn rising_series_gives_hours_to_the_limit() {
        // 0.50 now, +0.01/hour, over 24h => 0.74 at the end; 0.95 is 21 hours later.
        let s = line(48, 1800, 0.50, 0.01);
        let c = time_to_threshold(&s, 0.95).unwrap();
        assert!((c.hours - 21.0).abs() < 1.0, "{}", c.hours);
        assert!(c.confidence > 0.9);
    }

    #[test]
    fn too_little_history_gives_no_forecast() {
        assert!(time_to_threshold(&line(5, 300, 0.5, 0.01), 0.95).is_none());
        // plenty of samples but only 55 minutes of history
        assert!(time_to_threshold(&line(12, 300, 0.5, 0.01), 0.95).is_none());
    }

    #[test]
    fn flat_or_falling_never_crosses() {
        assert!(time_to_threshold(&line(48, 1800, 0.5, 0.0), 0.95).is_none());
        assert!(time_to_threshold(&line(48, 1800, 0.8, -0.01), 0.95).is_none());
    }

    #[test]
    fn noisy_data_is_rejected_not_extrapolated() {
        let s: Vec<(i64, f64)> = (0..48)
            .map(|i| (i * 1800, if i % 2 == 0 { 0.2 } else { 0.9 }))
            .collect();
        assert!(time_to_threshold(&s, 0.95).is_none());
    }

    #[test]
    fn already_past_the_limit_is_zero_hours() {
        let s = line(48, 1800, 0.90, 0.01);
        assert_eq!(time_to_threshold(&s, 0.95).unwrap().hours, 0.0);
    }

    #[test]
    fn far_future_crossings_are_dropped() {
        // +0.0001/hour needs ~months to reach the limit
        assert!(time_to_threshold(&line(48, 1800, 0.10, 0.0001), 0.95).is_none());
    }

    #[test]
    fn humanize() {
        assert_eq!(humanize_hours(0.2), "under an hour");
        assert_eq!(humanize_hours(14.0), "~14 hours");
        assert_eq!(humanize_hours(150.0), "~6 days");
    }
}
