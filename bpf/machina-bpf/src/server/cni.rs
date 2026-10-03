// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! machina-cni datapath state: pod endpoints, identities, NetworkPolicy
//! entries and service frontends, reconciled into the CNI_* maps.

use std::collections::{BTreeSet, HashSet};
use std::net::Ipv4Addr;

use super::*;

const CNI_SOCK_PROGS: &[&str] = &["mn_cni_connect4", "mn_cni_sendmsg4", "mn_cni_recvmsg4"];

#[derive(Default)]
pub(super) struct CniRuntime {
    pub node_addr: Option<String>,
    pub uplink: Option<String>,
    pub endpoints: HashMap<String, CniEndpoint>,
    /// (addr, port, proto) → svc id, stable across syncs.
    svc_ids: HashMap<(String, u16, u8), u32>,
    next_svc: u32,
    /// Backend count last written per svc id (to clear stale indexes).
    svc_backends: HashMap<u32, u32>,
    pub last: CniState,
    pub last_sync: Option<String>,
}

fn ipv4(s: &str) -> Result<[u8; 4]> {
    s.trim()
        .parse::<Ipv4Addr>()
        .map(|a| a.octets())
        .map_err(|_| anyhow!("invalid IPv4 address `{s}`"))
}

fn mac(s: &str) -> Result<[u8; 6]> {
    let parts: Vec<u8> = s
        .split(':')
        .map(|p| u8::from_str_radix(p, 16))
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| anyhow!("invalid MAC `{s}`"))?;
    parts.try_into().map_err(|_| anyhow!("invalid MAC `{s}`"))
}

/// `"10.0.0.0/8"` → (network bytes, prefix bits).
pub(crate) fn cidr4(s: &str) -> Result<([u8; 4], u32)> {
    let (a, l) = s.split_once('/').unwrap_or((s, "32"));
    let bits: u32 = l
        .parse()
        .ok()
        .filter(|b| *b <= 32)
        .ok_or_else(|| anyhow!("invalid prefix in `{s}`"))?;
    let addr = u32::from_be_bytes(ipv4(a)?);
    let mask = if bits == 0 { 0 } else { u32::MAX << (32 - bits) };
    Ok(((addr & mask).to_be_bytes(), bits))
}

impl Engine {
    pub(super) fn cni_configure(&mut self, node_addr: &str, uplink: Option<&str>) -> Result<CniStatus> {
        let addr = ipv4(node_addr)?;
        self.dp.cni_set_node(NodeCfg {
            node_addr: addr,
            flags: 0,
        })?;
        if let Some(up) = uplink {
            if if_nametoindex(up).is_none() {
                return Err(anyhow!("uplink {up} not found"));
            }
            self.dp.attach_tc_one(up, "mn_cni_nodeport", true)?;
        }
        if self.features.cgroup2 {
            let root = Path::new(attribution::CGROUP_ROOT);
            if let Err(e) = self.dp.attach_cgroup_tagged(root, "cni", CNI_SOCK_PROGS) {
                self.dp.notes.push(format!("socket load balancing unavailable: {e:#}"));
            }
        }
        self.cni.node_addr = Some(node_addr.to_string());
        self.cni.uplink = uplink.map(String::from);
        Ok(self.cni_status())
    }

    pub(super) fn cni_add_endpoint(&mut self, ep: CniEndpoint) -> Result<()> {
        let ip = ipv4(&ep.ip)?;
        let idx = if_nametoindex(&ep.host_iface)
            .ok_or_else(|| anyhow!("interface {} not found", ep.host_iface))?;
        let (identity, flags) = self.cni_meta_for(&ep.ip);
        self.dp.cni_hash_insert(
            "CNI_ENDPOINTS",
            ip,
            Endpoint {
                host_ifindex: idx,
                identity,
                flags,
                _pad: 0,
                pod_mac: mac(&ep.pod_mac)?,
                host_mac: mac(&ep.host_mac)?,
                _pad2: 0,
            },
        )?;
        self.dp.attach_tc_pair(&ep.host_iface, "mn_cni_from_pod", "mn_cni_to_pod")?;
        self.cni.endpoints.insert(ep.ip.clone(), ep);
        Ok(())
    }

    pub(super) fn cni_del_endpoint(&mut self, ip: &str) -> Result<bool> {
        let Some(ep) = self.cni.endpoints.remove(ip) else {
            return Ok(false);
        };
        if let Ok(a) = ipv4(ip) {
            self.dp.cni_hash_remove::<[u8; 4], Endpoint>("CNI_ENDPOINTS", &a);
        }
        if if_nametoindex(&ep.host_iface).is_some() {
            self.dp.detach_tc(&ep.host_iface);
        } else {
            self.dp.forget_tc(&ep.host_iface);
        }
        Ok(true)
    }

    fn cni_meta_for(&self, ip: &str) -> (u32, u32) {
        self.cni
            .last
            .identities
            .iter()
            .find(|i| i.ip == ip)
            .map(|i| {
                let mut f = 0;
                if i.ingress_isolated {
                    f |= EP_INGRESS_ISOLATED;
                }
                if i.egress_isolated {
                    f |= EP_EGRESS_ISOLATED;
                }
                (i.identity, f)
            })
            .unwrap_or((0, 0))
    }

    /// Replace the desired state; only changed map entries are written.
    pub(super) fn cni_sync(&mut self, st: CniState) -> Result<CniStatus> {
        // Identities (cluster-wide pod IP → identity).
        let want: HashMap<[u8; 4], u32> = st
            .identities
            .iter()
            .filter_map(|i| ipv4(&i.ip).ok().map(|a| (a, i.identity)))
            .collect();
        for k in self.dp.cni_hash_keys::<[u8; 4], u32>("CNI_IDENTITIES")? {
            if !want.contains_key(&k) {
                self.dp.cni_hash_remove::<[u8; 4], u32>("CNI_IDENTITIES", &k);
            }
        }
        for (a, id) in &want {
            self.dp.cni_hash_insert("CNI_IDENTITIES", *a, *id)?;
        }

        // ipBlock CIDRs.
        self.dp.cni_cidr_clear()?;
        for (c, id) in &st.cidrs {
            let (a, bits) = cidr4(c)?;
            self.dp.cni_cidr_insert(a, bits, *id)?;
        }

        // Policy entries.
        let want_pol: HashSet<PolicyKey> = st
            .policy
            .iter()
            .map(|p| PolicyKey {
                subject_identity: p.subject,
                peer_identity: p.peer,
                direction: if p.egress { POLICY_EGRESS } else { POLICY_INGRESS },
                proto: p.proto,
                port: p.port.to_be_bytes(),
            })
            .collect();
        for k in self.dp.cni_hash_keys::<PolicyKey, u32>("CNI_POLICY")? {
            if !want_pol.contains(&k) {
                self.dp.cni_hash_remove::<PolicyKey, u32>("CNI_POLICY", &k);
            }
        }
        for k in &want_pol {
            self.dp.cni_hash_insert("CNI_POLICY", *k, 1u32)?;
        }

        self.cni_sync_services(&st.services)?;

        self.cni.last = st;
        self.cni.last_sync = Some(chrono::Utc::now().to_rfc3339());

        // Local endpoints pick up their identity / isolation.
        let eps: Vec<CniEndpoint> = self.cni.endpoints.values().cloned().collect();
        for ep in eps {
            if let Err(e) = self.cni_add_endpoint(ep.clone()) {
                tracing::debug!("cni endpoint {}: {e:#}", ep.ip);
            }
        }
        Ok(self.cni_status())
    }

    fn cni_sync_services(&mut self, services: &[CniService]) -> Result<()> {
        let mut live: BTreeSet<u32> = BTreeSet::new();
        let mut want_svc: HashSet<SvcKey> = HashSet::new();
        let mut want_np: HashSet<SvcKey> = HashSet::new();
        for s in services {
            let addr = ipv4(&s.addr)?;
            let key = SvcKey {
                addr,
                port: s.port.to_be_bytes(),
                proto: s.proto,
                _pad: 0,
            };
            let id = match self.cni.svc_ids.get(&(s.addr.clone(), s.port, s.proto)) {
                Some(id) => *id,
                None => {
                    self.cni.next_svc += 1;
                    let id = self.cni.next_svc;
                    self.cni.svc_ids.insert((s.addr.clone(), s.port, s.proto), id);
                    id
                }
            };
            live.insert(id);
            // Backends first so a frontend never points at missing indexes.
            let backends: Vec<Backend> = s
                .backends
                .iter()
                .filter_map(|b| {
                    ipv4(&b.addr).ok().map(|a| Backend {
                        addr: a,
                        port: b.port.to_be_bytes(),
                        _pad: 0,
                    })
                })
                .collect();
            for (i, b) in backends.iter().enumerate() {
                self.dp.cni_hash_insert(
                    "CNI_BACKENDS",
                    BackendKey {
                        svc_id: id,
                        index: i as u32,
                    },
                    *b,
                )?;
            }
            let val = SvcVal {
                svc_id: id,
                backend_count: backends.len() as u32,
            };
            if addr == [0; 4] {
                self.dp.cni_hash_insert("CNI_NODEPORTS", key, val)?;
                want_np.insert(key);
            } else {
                self.dp.cni_hash_insert("CNI_SERVICES", key, val)?;
                want_svc.insert(key);
            }
            let old = self.cni.svc_backends.insert(id, backends.len() as u32).unwrap_or(0);
            for i in backends.len() as u32..old {
                self.dp.cni_hash_remove::<BackendKey, Backend>(
                    "CNI_BACKENDS",
                    &BackendKey { svc_id: id, index: i },
                );
            }
        }
        for (map, want) in [("CNI_SERVICES", &want_svc), ("CNI_NODEPORTS", &want_np)] {
            for k in self.dp.cni_hash_keys::<SvcKey, SvcVal>(map)? {
                if !want.contains(&k) {
                    self.dp.cni_hash_remove::<SvcKey, SvcVal>(map, &k);
                }
            }
        }
        let stale: Vec<u32> = self.cni.svc_backends.keys().filter(|id| !live.contains(id)).copied().collect();
        for id in stale {
            let n = self.cni.svc_backends.remove(&id).unwrap_or(0);
            for i in 0..n {
                self.dp.cni_hash_remove::<BackendKey, Backend>(
                    "CNI_BACKENDS",
                    &BackendKey { svc_id: id, index: i },
                );
            }
        }
        self.cni.svc_ids.retain(|_, id| live.contains(id));
        Ok(())
    }

    pub(super) fn cni_status(&self) -> CniStatus {
        let mut endpoints: Vec<CniEndpoint> = self.cni.endpoints.values().cloned().collect();
        endpoints.sort_by(|a, b| a.ip.cmp(&b.ip));
        CniStatus {
            configured: self.cni.node_addr.is_some(),
            node_addr: self.cni.node_addr.clone(),
            uplink: self.cni.uplink.clone(),
            endpoints,
            identities: self.cni.last.identities.len(),
            policy_entries: self.cni.last.policy.len(),
            services: self.cni.last.services.len(),
            last_sync: self.cni.last_sync.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsers() {
        assert_eq!(cidr4("10.1.2.3/8").unwrap(), ([10, 0, 0, 0], 8));
        assert_eq!(cidr4("192.168.1.7").unwrap(), ([192, 168, 1, 7], 32));
        assert_eq!(cidr4("0.0.0.0/0").unwrap(), ([0, 0, 0, 0], 0));
        assert!(cidr4("10.0.0.0/40").is_err());
        assert_eq!(mac("aa:bb:cc:00:11:22").unwrap(), [0xaa, 0xbb, 0xcc, 0, 0x11, 0x22]);
        assert!(mac("aa:bb").is_err());
    }
}
