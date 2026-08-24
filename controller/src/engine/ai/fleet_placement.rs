// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

use serde::Serialize;
use sqlx::SqlitePool;
use uuid::Uuid;

// Heuristic scoring weights for GPU placement — tuned so a GPU-tagged host
// with light load clearly outranks a busy general-purpose host, without
// letting CPU/memory alone push a non-GPU host above a GPU one.
const BASE_SCORE: f32 = 50.0;
const GPU_TAG_BONUS: f32 = 35.0;
const CPU_HEADROOM_WEIGHT: f32 = 0.2;
const MEM_HEADROOM_WEIGHT: f32 = 0.15;
const PER_VM_PENALTY: f32 = 1.5;
// Cap the headroom credit so an idle host doesn't dominate purely on being
// empty — utilization below this floor all scores the same.
const HEADROOM_UTILIZATION_FLOOR: f32 = 95.0;
// Only the top-N candidates are surfaced in the report.
const MAX_CANDIDATES: usize = 8;

#[derive(Debug, Serialize)]
pub struct GpuHostCandidate {
    pub host_id: String,
    pub hostname: String,
    pub gpu_capable: bool,
    pub numa_hint: String,
    pub score: f32,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct GpuPlacementReport {
    pub workload: String,
    pub candidates: Vec<GpuHostCandidate>,
    pub summary: String,
}

pub async fn advise_gpu(pool: &SqlitePool, workload: &str) -> anyhow::Result<GpuPlacementReport> {
    let rows: Vec<(Uuid, String, f32, i64, i64, i32, sqlx::types::Json<Vec<String>>)> = sqlx::query_as(
        "SELECT id, hostname, cpu_percent, memory_used_mib, memory_total_mib, vm_count,
                COALESCE(tags, '[]') AS tags
         FROM hosts WHERE state = 'online' AND maintenance_mode = FALSE ORDER BY hostname",
    )
    .fetch_all(pool)
    .await?;

    let mut candidates = Vec::new();
    for (id, hostname, cpu, mem_used, mem_total, vm_count, tags) in rows {
        let gpu_capable = tags.iter().any(|t| {
            let tl = t.to_lowercase();
            tl.contains("gpu") || tl == "nvidia" || tl == "inference"
        });
        let mem_pct = if mem_total > 0 {
            mem_used as f32 / mem_total as f32 * 100.0
        } else {
            0.0
        };
        let mut score = BASE_SCORE;
        if gpu_capable {
            score += GPU_TAG_BONUS;
        }
        score += (100.0 - cpu.min(HEADROOM_UTILIZATION_FLOOR)) * CPU_HEADROOM_WEIGHT;
        score += (100.0 - mem_pct.min(HEADROOM_UTILIZATION_FLOOR)) * MEM_HEADROOM_WEIGHT;
        score -= vm_count as f32 * PER_VM_PENALTY;

        let numa_hint = if gpu_capable {
            "Prefer local NUMA node for GPU passthrough / vGPU".into()
        } else {
            "No GPU tag — use for CPU inference fallback only".into()
        };

        let reason = if gpu_capable {
            format!("GPU-tagged host · CPU {cpu:.0}% · {vm_count} VMs")
        } else {
            format!("General compute · CPU {cpu:.0}% · mem {mem_pct:.0}%")
        };

        candidates.push(GpuHostCandidate {
            host_id: id.to_string(),
            hostname,
            gpu_capable,
            numa_hint,
            score,
            reason,
        });
    }

    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let gpu_count = candidates.iter().filter(|c| c.gpu_capable).count();
    let summary = if gpu_count > 0 {
        format!(
            "{} GPU-capable host(s) for '{workload}' — top: {}",
            gpu_count,
            candidates
                .first()
                .map(|c| c.hostname.as_str())
                .unwrap_or("—")
        )
    } else {
        format!(
            "No GPU-tagged hosts — tag hosts with gpu/nvidia for affinity. Workload: {workload}"
        )
    };

    Ok(GpuPlacementReport {
        workload: workload.into(),
        candidates: candidates.into_iter().take(MAX_CANDIDATES).collect(),
        summary,
    })
}
