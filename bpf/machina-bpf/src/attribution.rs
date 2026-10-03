// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! Workload attribution without libvirt bindings: tap → VM from libvirt's
//! live status XML, and cgroup path → VM / systemd unit / container.

use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VmIface {
    pub vm: String,
    pub uuid: Option<String>,
    pub mac: Option<String>,
}

fn attr(tag: &str, name: &str) -> Option<String> {
    for q in ['\'', '"'] {
        let pat = format!("{name}={q}");
        if let Some(i) = tag.find(&pat) {
            let rest = &tag[i + pat.len()..];
            if let Some(j) = rest.find(q) {
                return Some(rest[..j].to_string());
            }
        }
    }
    None
}

fn element_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let i = xml.find(&open)? + open.len();
    let j = xml[i..].find(&format!("</{tag}>"))?;
    Some(xml[i..i + j].trim().to_string())
}

/// Parse one libvirt domain (or domstatus) XML document into tap → VM entries.
pub fn parse_domain_xml(xml: &str) -> HashMap<String, VmIface> {
    let mut out = HashMap::new();
    let dom = match xml.find("<domain") {
        Some(i) => &xml[i..],
        None => return out,
    };
    let Some(name) = element_text(dom, "name") else {
        return out;
    };
    let uuid = element_text(dom, "uuid");
    let mut rest = dom;
    while let Some(i) = rest.find("<interface") {
        let block_start = &rest[i..];
        let end = block_start.find("</interface>").unwrap_or(block_start.len());
        let block = &block_start[..end];
        let target = block
            .find("<target")
            .and_then(|t| attr(&block[t..block[t..].find('>').map(|e| t + e).unwrap_or(block.len())], "dev"));
        let mac = block
            .find("<mac")
            .and_then(|t| attr(&block[t..block[t..].find('>').map(|e| t + e).unwrap_or(block.len())], "address"));
        if let Some(dev) = target {
            out.insert(
                dev,
                VmIface {
                    vm: name.clone(),
                    uuid: uuid.clone(),
                    mac,
                },
            );
        }
        rest = &block_start[end.min(block_start.len())..];
        if end == block_start.len() {
            break;
        }
    }
    out
}

/// Scan libvirt's runtime directories for running domains.
pub fn scan_libvirt() -> HashMap<String, VmIface> {
    let mut out = HashMap::new();
    for dir in ["/run/libvirt/qemu", "/var/run/libvirt/qemu"] {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("xml") {
                continue;
            }
            if let Ok(xml) = std::fs::read_to_string(&p) {
                out.extend(parse_domain_xml(&xml));
            }
        }
        if !out.is_empty() {
            break;
        }
    }
    out
}

/// systemd escapes '-' as `\x2d` in unit names.
fn unescape_systemd(s: &str) -> String {
    s.replace("\\x2d", "-").replace("\\x5c", "\\")
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CgroupInfo {
    pub vm: Option<String>,
    pub unit: Option<String>,
    pub container: Option<String>,
}

/// Classify a cgroup v2 path (relative, e.g. `machine.slice/machine-qemu\x2d3\x2dweb.scope/libvirt/emulator`).
pub fn classify_cgroup(path: &str) -> CgroupInfo {
    let mut info = CgroupInfo::default();
    for seg in path.split('/') {
        if let Some(rest) = seg.strip_prefix("machine-qemu\\x2d") {
            let rest = rest.trim_end_matches(".scope");
            // "<id>\x2d<name>"
            let name = rest.split_once("\\x2d").map(|(_, n)| n).unwrap_or(rest);
            info.vm = Some(unescape_systemd(name));
        } else if let Some(id) = seg
            .strip_prefix("libpod-")
            .or_else(|| seg.strip_prefix("docker-"))
            .or_else(|| seg.strip_prefix("cri-containerd-"))
            .or_else(|| seg.strip_prefix("crio-"))
        {
            let id = id.trim_end_matches(".scope");
            if !id.starts_with("conmon") {
                info.container = Some(id.chars().take(12).collect());
            }
        } else if seg.ends_with(".service") && info.unit.is_none() {
            info.unit = Some(unescape_systemd(seg));
        }
    }
    info
}

/// cgroup v2 id (== inode of the cgroup directory) → relative path cache.
#[derive(Default)]
pub struct CgroupCache {
    map: HashMap<u64, String>,
    last_scan: Option<std::time::Instant>,
}

pub const CGROUP_ROOT: &str = "/sys/fs/cgroup";

#[cfg(unix)]
pub fn cgroup_id(path: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).ok().map(|m| m.ino())
}

#[cfg(not(unix))]
pub fn cgroup_id(_path: &Path) -> Option<u64> {
    None
}

impl CgroupCache {
    pub fn lookup(&mut self, id: u64) -> Option<String> {
        if let Some(p) = self.map.get(&id) {
            return Some(p.clone());
        }
        let stale = self
            .last_scan
            .map(|t| t.elapsed() > std::time::Duration::from_secs(5))
            .unwrap_or(true);
        if stale {
            self.rescan();
        }
        self.map.get(&id).cloned()
    }

    fn rescan(&mut self) {
        self.last_scan = Some(std::time::Instant::now());
        self.map.clear();
        let root = Path::new(CGROUP_ROOT);
        let mut stack = vec![(root.to_path_buf(), 0usize)];
        while let Some((dir, depth)) = stack.pop() {
            if let Some(id) = cgroup_id(&dir) {
                let rel = dir
                    .strip_prefix(root)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.map.insert(id, rel);
            }
            if depth >= 10 || self.map.len() > 50_000 {
                continue;
            }
            if let Ok(rd) = std::fs::read_dir(&dir) {
                for e in rd.flatten() {
                    if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        stack.push((e.path(), depth + 1));
                    }
                }
            }
        }
    }
}

/// `/proc/<pid>/stat` ppid and `/proc/<pid>/cmdline`.
pub fn proc_details(pid: u32) -> (Option<u32>, Option<String>) {
    let ppid = std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|s| {
            let after = &s[s.rfind(')')? + 1..];
            after.split_whitespace().nth(1)?.parse().ok()
        });
    let cmdline = std::fs::read(format!("/proc/{pid}/cmdline"))
        .ok()
        .filter(|b| !b.is_empty())
        .map(|b| {
            b.split(|c| *c == 0)
                .filter(|s| !s.is_empty())
                .map(|s| String::from_utf8_lossy(s).into_owned())
                .collect::<Vec<_>>()
                .join(" ")
        });
    (ppid, cmdline)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_xml() {
        let xml = r#"<domstatus state='running'><monitor path='/x'/><domain type='kvm' id='3'>
          <name>web-01</name><uuid>abcd</uuid>
          <devices>
            <interface type='network'><mac address='52:54:00:aa:bb:cc'/><source network='default'/><target dev='vnet3'/></interface>
            <interface type="bridge"><mac address="52:54:00:11:22:33"/><target dev="vnet4"/></interface>
          </devices></domain></domstatus>"#;
        let m = parse_domain_xml(xml);
        assert_eq!(m.len(), 2);
        assert_eq!(m["vnet3"].vm, "web-01");
        assert_eq!(m["vnet3"].mac.as_deref(), Some("52:54:00:aa:bb:cc"));
        assert_eq!(m["vnet4"].uuid.as_deref(), Some("abcd"));
    }

    #[test]
    fn cgroups() {
        let c = classify_cgroup("machine.slice/machine-qemu\\x2d3\\x2dweb\\x2d01.scope/libvirt/emulator");
        assert_eq!(c.vm.as_deref(), Some("web-01"));
        let c = classify_cgroup("system.slice/machina\\x2ddaemon.service");
        assert_eq!(c.unit.as_deref(), Some("machina-daemon.service"));
        let c = classify_cgroup("machine.slice/libpod-0123456789abcdef.scope/container");
        assert_eq!(c.container.as_deref(), Some("0123456789ab"));
    }
}
