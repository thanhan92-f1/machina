// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! Host-local IPv4 IPAM: one file per allocated address, created with
//! `O_EXCL` so concurrent CNI invocations never hand out the same IP.

use std::fs::OpenOptions;
use std::io::Write;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subnet {
    pub net: u32,
    pub bits: u32,
}

impl Subnet {
    pub fn parse(s: &str) -> Result<Self> {
        let (a, l) = s.split_once('/').ok_or_else(|| anyhow!("subnet `{s}` needs a prefix length"))?;
        let bits: u32 = l.parse().ok().filter(|b| (8..=30).contains(b)).ok_or_else(|| anyhow!("unsupported prefix in `{s}` (8..30)"))?;
        let ip: Ipv4Addr = a.parse().map_err(|_| anyhow!("invalid subnet `{s}`"))?;
        let mask = u32::MAX << (32 - bits);
        Ok(Self { net: u32::from(ip) & mask, bits })
    }

    /// Usable pod addresses: skips network, the `.1` gateway slot and broadcast.
    pub fn hosts(&self) -> impl Iterator<Item = Ipv4Addr> {
        let size = 1u32 << (32 - self.bits);
        let net = self.net;
        (2..size - 1).map(move |i| Ipv4Addr::from(net + i))
    }

    #[cfg(test)]
    pub fn contains(&self, ip: Ipv4Addr) -> bool {
        let mask = u32::MAX << (32 - self.bits);
        u32::from(ip) & mask == self.net
    }

    pub fn dir_name(&self) -> String {
        format!("{}-{}", Ipv4Addr::from(self.net), self.bits)
    }
}

pub struct Ipam {
    dir: PathBuf,
    subnet: Subnet,
}

fn owner(container_id: &str, ifname: &str) -> String {
    format!("{container_id}\n{ifname}\n")
}

impl Ipam {
    pub fn new(root: &Path, subnet: Subnet) -> Result<Self> {
        let dir = root.join(subnet.dir_name());
        std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        Ok(Self { dir, subnet })
    }

    /// Existing allocation for this container/interface, if any (CNI ADD is retried).
    pub fn find(&self, container_id: &str, ifname: &str) -> Option<Ipv4Addr> {
        let want = owner(container_id, ifname);
        std::fs::read_dir(&self.dir).ok()?.flatten().find_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let ip: Ipv4Addr = name.parse().ok()?;
            (std::fs::read_to_string(e.path()).ok()? == want).then_some(ip)
        })
    }

    pub fn allocate(&self, container_id: &str, ifname: &str) -> Result<Ipv4Addr> {
        if let Some(ip) = self.find(container_id, ifname) {
            return Ok(ip);
        }
        // Start after the most recent allocation so freed IPs are not reused at once.
        let last_path = self.dir.join("last");
        let last: Option<Ipv4Addr> = std::fs::read_to_string(&last_path).ok().and_then(|s| s.trim().parse().ok());
        let hosts: Vec<Ipv4Addr> = self.subnet.hosts().collect();
        let start = last
            .and_then(|l| hosts.iter().position(|h| *h == l))
            .map(|p| p + 1)
            .unwrap_or(0);
        for i in 0..hosts.len() {
            let ip = hosts[(start + i) % hosts.len()];
            let path = self.dir.join(ip.to_string());
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut f) => {
                    f.write_all(owner(container_id, ifname).as_bytes())?;
                    let _ = std::fs::write(&last_path, ip.to_string());
                    return Ok(ip);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e).with_context(|| format!("reserve {}", path.display())),
            }
        }
        Err(anyhow!("no free addresses in {}/{}", Ipv4Addr::from(self.subnet.net), self.subnet.bits))
    }

    /// Release this container's address. Missing allocations are not an error.
    pub fn release(&self, container_id: &str, ifname: &str) -> Option<Ipv4Addr> {
        let ip = self.find(container_id, ifname)?;
        let _ = std::fs::remove_file(self.dir.join(ip.to_string()));
        Some(ip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subnet_hosts() {
        let s = Subnet::parse("10.42.1.0/30").unwrap();
        assert_eq!(s.hosts().collect::<Vec<_>>(), vec![Ipv4Addr::new(10, 42, 1, 2)]);
        let s = Subnet::parse("10.42.1.77/24").unwrap();
        assert_eq!(s.net, u32::from(Ipv4Addr::new(10, 42, 1, 0)));
        assert_eq!(s.hosts().count(), 253);
        assert!(s.contains(Ipv4Addr::new(10, 42, 1, 200)));
        assert!(!s.contains(Ipv4Addr::new(10, 42, 2, 1)));
        assert!(Subnet::parse("10.0.0.0").is_err());
        assert!(Subnet::parse("10.0.0.0/31").is_err());
    }

    #[test]
    fn allocate_release_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let ipam = Ipam::new(tmp.path(), Subnet::parse("10.42.0.0/29").unwrap()).unwrap();
        let a = ipam.allocate("c1", "eth0").unwrap();
        assert_eq!(a, Ipv4Addr::new(10, 42, 0, 2));
        assert_eq!(ipam.allocate("c1", "eth0").unwrap(), a, "retry returns the same IP");
        let b = ipam.allocate("c2", "eth0").unwrap();
        assert_ne!(a, b);
        assert_eq!(ipam.release("c1", "eth0"), Some(a));
        assert_eq!(ipam.release("c1", "eth0"), None);
        // .2–.6 usable in a /29; c2 holds one, so four more fit, then exhaustion.
        for i in 0..4 {
            ipam.allocate(&format!("x{i}"), "eth0").unwrap();
        }
        assert!(ipam.allocate("full", "eth0").is_err());
    }
}
