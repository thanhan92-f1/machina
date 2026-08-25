// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

//! Short-lived console URL tokens for same-origin embedding.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::config::OpenStackConfig;
use crate::LibvirtError;

use super::instance_ops::get_remote_console;

const TTL: Duration = Duration::from_secs(300);

struct Entry {
    url: String,
    expires: Instant,
}

fn store() -> &'static Mutex<HashMap<String, Entry>> {
    static STORE: OnceLock<Mutex<HashMap<String, Entry>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn prune(map: &mut HashMap<String, Entry>) {
    let now = Instant::now();
    map.retain(|_, e| e.expires > now);
}

pub fn issue_console_token(instance_id: &str, console_url: &str) -> String {
    let token = format!("{}-{}", instance_id, random_token_suffix());
    let mut map = store().lock().unwrap_or_else(|e| e.into_inner());
    prune(&mut map);
    map.insert(
        token.clone(),
        Entry {
            url: console_url.to_string(),
            expires: Instant::now() + TTL,
        },
    );
    token
}

pub fn resolve_console_token(token: &str) -> Option<String> {
    let mut map = store().lock().unwrap_or_else(|e| e.into_inner());
    prune(&mut map);
    map.get(token).map(|e| e.url.clone())
}

/// Cryptographically random token suffix. The previous implementation used the
/// current nanosecond timestamp, which is not secret: an attacker who can guess
/// roughly when a console session was opened (e.g. from a UI action, or by
/// watching request timing) has only a small search space to brute-force within
/// the 300s TTL, and `instance_id` — the other half of the token — is routinely
/// visible to any authenticated user who can list instances. That let anyone who
/// could enumerate instance IDs guess a live console token and hijack another
/// tenant's VNC/SPICE session through the proxy.
fn random_token_suffix() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub async fn remote_console_with_tunnel(
    cfg: &OpenStackConfig,
    id: &str,
    console_type: &str,
) -> Result<(String, String), LibvirtError> {
    let c = get_remote_console(cfg, id, console_type).await?;
    let token = issue_console_token(id, &c.url);
    let proxy_path = format!("/api/v1/openstack/console-tunnel/{token}");
    Ok((c.url, proxy_path))
}
