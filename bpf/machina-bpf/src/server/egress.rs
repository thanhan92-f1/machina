// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! Per-project egress IPs: the `ip`/`ip6 machina_egress` nftables tables. They are
//! address translation, not a security control, so it stays installed while
//! bpfd is stopped and is re-applied from the persisted config on start.

use std::io::Write as _;
use std::process::{Command, Stdio};

use crate::netpol::snat;

use super::*;

#[derive(Default)]
pub(super) struct EgressRuntime {
    pub config: VmEgressSnat,
    skipped: Vec<String>,
    error: Option<String>,
    active: bool,
}

fn nft(script: &str) -> Result<()> {
    let mut child = Command::new("nft")
        .args(["-f", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("run nft")?;
    child
        .stdin
        .take()
        .context("nft stdin")?
        .write_all(script.as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "nft: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

fn local_addresses() -> Vec<std::net::IpAddr> {
    Command::new("ip")
        .args(["-o", "addr", "show"])
        .output()
        .map(|o| snat::local_addrs(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

impl Engine {
    pub(super) fn vm_egress_set(&mut self, cfg: VmEgressSnat) -> Result<VmEgressSnatStatus> {
        snat::validate(&cfg).map_err(|e| anyhow!(e))?;
        let (script, skipped) = snat::render(&cfg, &local_addresses());
        let idle = script.is_none() && !self.egress.active && self.egress.error.is_none();
        let res = if idle {
            Ok(())
        } else {
            nft(&snat::transaction(script.as_deref()))
        };
        if let Err(e) = &res {
            tracing::warn!("egress SNAT: {e:#}");
        } else if !idle {
            tracing::info!(
                rules = cfg.rules.len(),
                skipped = skipped.len(),
                "egress SNAT applied"
            );
        }
        self.egress = EgressRuntime {
            active: script.is_some() && res.is_ok(),
            error: res.err().map(|e| format!("{e:#}")),
            config: cfg,
            skipped,
        };
        Ok(self.vm_egress_status())
    }

    pub(super) fn vm_egress_status(&self) -> VmEgressSnatStatus {
        VmEgressSnatStatus {
            rules: self.egress.config.rules.clone(),
            exclude: snat::excludes(&self.egress.config),
            active: self.egress.active,
            skipped: self.egress.skipped.clone(),
            error: self.egress.error.clone(),
            hostname: None,
        }
    }
}
