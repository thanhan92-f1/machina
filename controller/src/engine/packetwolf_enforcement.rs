// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Runtime enforcement bridge — dev fabric (Tetragon) vs production PacketWolf TC allowlist.

use serde_json::{json, Value};
use uuid::Uuid;

use crate::config::ControllerConfig;
use crate::engine::packetwolf_bridge::{
    dev_fabric_available, fabric_delete, fabric_get, fabric_patch, fabric_post,
    fabric_put, production_network_available,
};
use crate::engine::packetwolf_local;

async fn production_enforcement_mode(cfg: &ControllerConfig) -> bool {
    production_network_available(cfg).await && !dev_fabric_available(cfg).await
}

fn tc_rule_to_policy(rule: &Value) -> Value {
    let id = rule
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("tc-rule")
        .to_string();
    let proto = rule
        .get("proto")
        .or_else(|| rule.get("protocol"))
        .and_then(|v| v.as_str())
        .unwrap_or("tcp");
    let ip = rule
        .get("dstIpv4")
        .or_else(|| rule.get("dst_ipv4"))
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0.0");
    let port = rule
        .get("dstPort")
        .or_else(|| rule.get("dst_port"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let comment = rule
        .get("comment")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    json!({
        "id": id,
        "name": comment.unwrap_or(&id),
        "kind": "tc_allow",
        "match": format!("{ip}:{port}/{proto}"),
        "enabled": true,
        "scope": "fleet",
        "description": "PacketWolf TC egress allowlist rule (defaultDeny host enforcement)",
        "backend": "packetwolf-tc",
    })
}

fn normalize_production_status(raw: &Value) -> Value {
    let attached = raw.get("attached").and_then(|v| v.as_bool()).unwrap_or(false);
    let rule_count = raw
        .get("ruleCount")
        .or_else(|| raw.get("rules"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    let drops = raw.get("drops").and_then(|v| v.as_u64()).unwrap_or(0);
    let warning = raw
        .get("warning")
        .and_then(|v| v.as_str())
        .unwrap_or("Production PacketWolf TC egress enforcement");
    let tetragon_policies = packetwolf_local::list_enforcement_policies();
    let tetragon_count = tetragon_policies.len();
    let tetragon_enabled_count = tetragon_policies
        .iter()
        .filter(|p| p.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true))
        .count();
    json!({
        "mode": if attached { "enforce" } else { "observe" },
        "policies_total": rule_count + tetragon_count,
        "policies_enabled": rule_count + tetragon_enabled_count,
        "applied_hosts": [],
        "blocked_events": drops,
        "attached": attached,
        "default_deny": raw.get("defaultDeny").and_then(|v| v.as_bool()).unwrap_or(true),
        "iface": raw.get("iface"),
        "platform": raw.get("platform"),
        "api_mode": "production_tc",
        "summary": format!(
            "{warning} · {rule_count} TC allow rule(s) · {tetragon_count} Tetragon policy(ies) · {}",
            if attached { "BPF attached" } else { "BPF detached (observe)" }
        ),
    })
}

fn normalize_production_policies(raw: &Value) -> Value {
    let mut policies: Vec<Value> = raw
        .get("rules")
        .and_then(|v| v.as_array())
        .map(|rules| rules.iter().map(tc_rule_to_policy).collect())
        .unwrap_or_default();
    policies.extend(packetwolf_local::list_enforcement_policies());
    json!({
        "policies": policies,
        "default_deny": raw.get("defaultDeny").and_then(|v| v.as_bool()).unwrap_or(true),
        "api_mode": "production_tc",
    })
}

/// Parse a `"ip:port/proto"` TC allow-rule match string. The IP is REQUIRED: this
/// produces a host-level eBPF/TC egress ALLOW rule against a defaultDeny policy, so
/// a missing IP must never silently become a match-all wildcard (`0.0.0.0` in this
/// TC scheme matches any destination) — that would fail OPEN, punching an any-IP
/// hole through the allowlist for the given port. Reject (`None`) instead of
/// guessing, so the caller surfaces the mistake rather than installing an overly
/// broad rule.
fn parse_port_match(match_str: &str) -> Option<(String, u16, String)> {
    let (host_port, proto) = match_str.split_once('/')?;
    let proto = if proto.is_empty() { "tcp" } else { proto };
    let (ip, port) = host_port.rsplit_once(':')?;
    if ip.is_empty() {
        return None;
    }
    let port: u16 = port.parse().ok()?;
    Some((ip.to_string(), port, proto.to_string()))
}

fn machina_policy_to_tc_rule(id: &str, kind: &str, match_str: &str, name: &str) -> Option<Value> {
    if kind != "tc_allow" && kind != "allow_port" {
        return None;
    }
    let (ip, port, proto) = parse_port_match(match_str)?;
    Some(json!({
        "id": id,
        "proto": proto,
        "dstIpv4": ip,
        "dstPort": port,
        "comment": name,
    }))
}

pub async fn enforcement_status(cfg: &ControllerConfig) -> Value {
    if production_enforcement_mode(cfg).await {
        let raw = fabric_get(cfg, "/api/v1/runtime/enforcement/status").await;
        return normalize_production_status(&raw);
    }
    fabric_get(cfg, "/api/v1/enforcement/status").await
}

pub async fn enforcement_policies(cfg: &ControllerConfig) -> Value {
    if production_enforcement_mode(cfg).await {
        let raw = fabric_get(cfg, "/api/v1/runtime/enforcement/rules").await;
        return normalize_production_policies(&raw);
    }
    let raw = fabric_get(cfg, "/api/v1/enforcement/policies").await;
    if raw.get("policies").and_then(|v| v.as_array()).is_some() {
        return raw;
    }
    if production_network_available(cfg).await {
        let tc = fabric_get(cfg, "/api/v1/runtime/enforcement/rules").await;
        return normalize_production_policies(&tc);
    }
    json!({
        "policies": packetwolf_local::list_enforcement_policies(),
        "api_mode": "local_fabric",
    })
}

pub async fn create_enforcement_policy(cfg: &ControllerConfig, body: Value) -> Value {
    if !production_enforcement_mode(cfg).await {
        return fabric_post(cfg, "/api/v1/enforcement/policies", body).await;
    }
    let name = body
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("policy")
        .to_string();
    let kind = body
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or("deny_process")
        .to_string();
    let match_str = body
        .get("match")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let enabled = body
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let scope = body
        .get("scope")
        .and_then(|v| v.as_str())
        .unwrap_or("fleet")
        .to_string();
    let description = body
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let id = Uuid::new_v4().to_string();
    let tc_rule = machina_policy_to_tc_rule(&id, &kind, &match_str, &name);

    // A tc_allow/allow_port kind that fails to parse (missing/empty destination
    // IP, bad port, etc.) must be rejected outright — NOT silently redirected to
    // the Tetragon local-policy fallback below. That fallback is for genuinely
    // different policy kinds; routing a malformed TC allow rule through it would
    // silently install nothing on the TC allowlist while telling the admin the
    // policy was created, which is just as unsafe as fail-open (enforcement the
    // admin believes is active is quietly absent).
    if (kind == "tc_allow" || kind == "allow_port") && tc_rule.is_none() {
        return json!({
            "ok": false,
            "error": "invalid match: tc_allow/allow_port rules require an explicit destination IP, e.g. \"203.0.113.5:443/tcp\" — refusing to create a wildcard-IP allow rule",
            "api_mode": "production_tc",
        });
    }

    if let Some(tc_rule) = tc_rule {
        let current = fabric_get(cfg, "/api/v1/runtime/enforcement/rules").await;
        let mut rules = current
            .get("rules")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        rules.push(tc_rule.clone());
        let put_body = json!({
            "rules": rules,
            "defaultDeny": current.get("defaultDeny").and_then(|v| v.as_bool()).unwrap_or(true),
        });
        let pw = fabric_put(cfg, "/api/v1/runtime/enforcement/rules", put_body).await;
        return json!({
            "policy": tc_rule_to_policy(&tc_rule),
            "packetwolf": pw,
            "api_mode": "production_tc",
        });
    }

    let policy = packetwolf_local::upsert_enforcement_policy(
        &id,
        &name,
        &kind,
        &match_str,
        enabled,
        &scope,
        &description,
    );
    json!({
        "policy": policy,
        "api_mode": "production_tetragon",
        "note": "Tetragon policy stored on controller — apply to hosts to push TracingPolicy bundle",
    })
}

pub async fn apply_enforcement_policy(
    cfg: &ControllerConfig,
    policy_id: &str,
    host_ids: &[String],
) -> Value {
    if !production_enforcement_mode(cfg).await {
        let body = json!({ "host_ids": host_ids });
        return fabric_post(
            cfg,
            &format!("/api/v1/enforcement/policies/{policy_id}/apply"),
            body,
        )
        .await;
    }

    let sync_result = fabric_post(cfg, "/api/v1/runtime/enforcement/sync", json!({})).await;
    let mut results = vec![sync_result];
    if let Some(policy) = packetwolf_local::get_enforcement_policy(policy_id) {
        if policy.get("kind").and_then(|v| v.as_str()) == Some("tc_allow")
            || policy.get("backend").and_then(|v| v.as_str()) == Some("packetwolf-tc")
        {
            results.push(fabric_post(cfg, "/api/v1/runtime/enforcement/attach", json!({})).await);
        }
    } else {
        // TC rules from PacketWolf may not be in local store — still sync/attach BPF map.
        results.push(fabric_post(cfg, "/api/v1/runtime/enforcement/attach", json!({})).await);
    }
    // A fabric_post that couldn't reach/parse the upstream call returns the
    // sentinel `{"ok": false}` (see fabric_post/post_json) — surface that instead
    // of unconditionally claiming success, so a dead/unreachable PacketWolf
    // backend isn't reported as "enforcement applied" when nothing was actually
    // synced. Mark local state as applied only when the remote sync is confirmed
    // dispatched, so local and remote state stay in agreement.
    let synced = !results
        .iter()
        .any(|r| r.get("ok").and_then(|v| v.as_bool()) == Some(false));
    if synced {
        packetwolf_local::mark_policy_applied(policy_id, host_ids);
    }
    json!({
        "ok": synced,
        "api_mode": "production_tc",
        "host_ids": host_ids,
        "packetwolf": results,
        "summary": if synced {
            format!("Enforcement sync queued for {} host(s)", host_ids.len())
        } else {
            "Enforcement sync failed — PacketWolf backend unreachable or rejected the request"
                .to_string()
        },
    })
}

pub async fn patch_enforcement_policy(
    cfg: &ControllerConfig,
    policy_id: &str,
    body: Value,
) -> Value {
    if !production_enforcement_mode(cfg).await {
        return fabric_patch(
            cfg,
            &format!("/api/v1/enforcement/policies/{policy_id}"),
            body,
        )
        .await;
    }
    if packetwolf_local::get_enforcement_policy(policy_id).is_some() {
        let updated = packetwolf_local::patch_enforcement_policy(policy_id, &body);
        return json!({
            "policy": updated,
            "api_mode": "production_tetragon",
            "sync_hosts": updated
                .get("applied_hosts")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|h| h.as_str().map(String::from))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        });
    }
    fabric_patch(
        cfg,
        &format!("/api/v1/enforcement/policies/{policy_id}"),
        body,
    )
    .await
}

pub async fn delete_enforcement_policy(cfg: &ControllerConfig, policy_id: &str) -> Value {
    if !production_enforcement_mode(cfg).await {
        return fabric_delete(cfg, &format!("/api/v1/enforcement/policies/{policy_id}")).await;
    }
    if let Some(removed) = packetwolf_local::delete_enforcement_policy(policy_id) {
        let hosts = removed
            .get("applied_hosts")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|h| h.as_str().map(String::from))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        return json!({
            "ok": true,
            "api_mode": "production_tetragon",
            "removed_from_hosts": hosts,
        });
    }
    let current = fabric_get(cfg, "/api/v1/runtime/enforcement/rules").await;
    let rules: Vec<Value> = current
        .get("rules")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter(|r| r.get("id").and_then(|v| v.as_str()) != Some(policy_id))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let put_body = json!({
        "rules": rules,
        "defaultDeny": current.get("defaultDeny").and_then(|v| v.as_bool()).unwrap_or(true),
    });
    let pw = fabric_put(cfg, "/api/v1/runtime/enforcement/rules", put_body).await;
    // fabric_put falls back to the `{"ok": false}` sentinel when the upstream
    // PUT couldn't be reached/parsed — don't claim the TC allow rule was removed
    // when it may still be live on the host.
    let removed = pw.get("ok").and_then(|v| v.as_bool()) != Some(false);
    json!({
        "ok": removed,
        "api_mode": "production_tc",
        "packetwolf": pw,
    })
}

pub async fn enforcement_policy_tetragon(cfg: &ControllerConfig, policy_id: &str) -> Value {
    if !production_enforcement_mode(cfg).await {
        return fabric_get(
            cfg,
            &format!("/api/v1/enforcement/policies/{policy_id}/tetragon"),
        )
        .await;
    }
    if let Some(policy) = packetwolf_local::get_enforcement_policy(policy_id) {
        let tetragon = packetwolf_local::tetragon_policy_for(&policy);
        return json!({
            "tetragon_policy": tetragon,
            "tetragon_policy_name": format!("packetwolf-{policy_id}"),
            "api_mode": "production_tetragon",
        });
    }
    json!({
        "note": "TC allowlist rules use host BPF egress map — no TracingPolicy document",
        "policy_id": policy_id,
        "api_mode": "production_tc",
    })
}

pub async fn attach_enforcement(cfg: &ControllerConfig) -> Value {
    if production_enforcement_mode(cfg).await {
        let raw = fabric_post(cfg, "/api/v1/runtime/enforcement/attach", json!({})).await;
        return normalize_production_status(&raw);
    }
    json!({"ok": false, "note": "attach only available in production PacketWolf mode"})
}

pub async fn sync_enforcement(cfg: &ControllerConfig) -> Value {
    if production_enforcement_mode(cfg).await {
        let raw = fabric_post(cfg, "/api/v1/runtime/enforcement/sync", json!({})).await;
        return normalize_production_status(&raw);
    }
    json!({"ok": false, "note": "sync only available in production PacketWolf mode"})
}

pub async fn detach_enforcement(cfg: &ControllerConfig) -> Value {
    if production_enforcement_mode(cfg).await {
        let raw = fabric_post(cfg, "/api/v1/runtime/enforcement/detach", json!({})).await;
        return normalize_production_status(&raw);
    }
    json!({"ok": false, "note": "detach only available in production PacketWolf mode"})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tc_rule_maps_to_policy() {
        let rule = json!({"id":"e2e-dns","proto":"udp","dstIpv4":"8.8.8.8","dstPort":53});
        let pol = tc_rule_to_policy(&rule);
        assert_eq!(pol["kind"], "tc_allow");
        assert!(pol["match"].as_str().unwrap().contains("8.8.8.8"));
    }

    #[test]
    fn production_status_normalization() {
        let raw = json!({"attached":false,"ruleCount":2,"drops":5,"warning":"TC opt-in"});
        let norm = normalize_production_status(&raw);
        assert_eq!(norm["mode"], "observe");
        assert_eq!(norm["blocked_events"], 5);
    }

    #[test]
    fn parse_port_match_requires_explicit_ip() {
        // Fail-closed: a match string with no IP must be rejected, not silently
        // widened into a 0.0.0.0 (match-any) allow rule.
        assert_eq!(parse_port_match("443/tcp"), None);
        assert_eq!(parse_port_match("443"), None);
        assert_eq!(parse_port_match(":443/tcp"), None);
    }

    #[test]
    fn parse_port_match_accepts_explicit_ip() {
        assert_eq!(
            parse_port_match("203.0.113.5:443/tcp"),
            Some(("203.0.113.5".into(), 443, "tcp".into()))
        );
        // proto defaults to tcp when omitted.
        assert_eq!(
            parse_port_match("203.0.113.5:443/"),
            Some(("203.0.113.5".into(), 443, "tcp".into()))
        );
    }

    #[test]
    fn machina_policy_to_tc_rule_rejects_wildcard_ip() {
        assert!(machina_policy_to_tc_rule("id1", "tc_allow", "443/tcp", "rule").is_none());
    }
}
