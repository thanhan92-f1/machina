// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! aya loader for the embedded `machina-bpf-ebpf` object: program attach.
//! Map helpers live in `loader_maps.rs`.

use std::collections::{HashMap as StdHashMap, HashSet};
use std::fs::File;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use aya::programs::{
    cgroup_skb::CgroupSkbLinkId, cgroup_sock_addr::CgroupSockAddrLinkId, tc::SchedClassifierLinkId,
    xdp::XdpLinkId, CgroupAttachMode, CgroupSkb, CgroupSkbAttachType, CgroupSockAddr, KProbe,
    SchedClassifier, TcAttachType, TracePoint, Xdp, XdpMode,
};
use aya::util::KernelVersion;
use aya::{Ebpf, EbpfLoader, VerifierLogLevel};

use crate::api::KernelFeatures;

static BPF_OBJECT: &[u8] = aya::include_bytes_aligned!(concat!(env!("OUT_DIR"), "/machina-bpf.o"));

/// True when the build staged a real eBPF object (nightly + bpf-linker were available).
pub fn programs_compiled() -> bool {
    !BPF_OBJECT.is_empty()
}

pub fn kernel_features() -> KernelFeatures {
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let tcx = KernelVersion::current()
        .map(|v| v.code() >= KernelVersion::new(6, 6, 0).code())
        .unwrap_or(false);
    let lsm_bpf = std::fs::read_to_string("/sys/kernel/security/lsm")
        .map(|s| s.split(',').any(|l| l.trim() == "bpf"))
        .unwrap_or(false);
    KernelFeatures {
        kernel,
        btf: Path::new("/sys/kernel/btf/vmlinux").exists(),
        tcx,
        lsm_bpf,
        tracefs: crate::tracefs::tracefs_root().map(|p| p.display().to_string()),
        cgroup2: Path::new("/sys/fs/cgroup/cgroup.controllers").exists(),
    }
}

/// Raise RLIMIT_MEMLOCK for pre-5.11 kernels (memcg accounting makes it moot later).
pub fn bump_memlock() {
    let rlim = libc::rlimit {
        rlim_cur: libc::RLIM_INFINITY,
        rlim_max: libc::RLIM_INFINITY,
    };
    unsafe {
        libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlim);
    }
}

/// CLOCK_MONOTONIC in ns (same clock as `bpf_ktime_get_ns`).
pub fn monotonic_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe {
        libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts);
    }
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

/// Convert a `bpf_ktime_get_ns` timestamp to Unix epoch microseconds.
pub fn mono_to_epoch_us(mono_ns: u64) -> u64 {
    let now_mono = monotonic_ns();
    let now_wall = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    now_wall.saturating_sub(now_mono.saturating_sub(mono_ns)) / 1000
}

pub fn mono_to_rfc3339(mono_ns: u64) -> String {
    let us = mono_to_epoch_us(mono_ns) as i64;
    chrono::DateTime::from_timestamp_micros(us)
        .unwrap_or_default()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

struct TcLinks {
    ingress: SchedClassifierLinkId,
    egress: SchedClassifierLinkId,
}

#[derive(Default)]
struct CgroupLinks {
    sock: Vec<(String, CgroupSockAddrLinkId)>,
    skb: Vec<(String, CgroupSkbLinkId)>,
}

pub struct Datapath {
    pub(crate) ebpf: Ebpf,
    loaded: HashSet<String>,
    tc: StdHashMap<String, (String, TcLinks)>,
    uplink: StdHashMap<String, (String, SchedClassifierLinkId)>,
    xdp: StdHashMap<String, (String, XdpLinkId)>,
    cgroups: StdHashMap<String, CgroupLinks>,
    pub tracepoints: Vec<String>,
    pub notes: Vec<String>,
    tcx: bool,
}

impl Datapath {
    pub fn load() -> Result<Self> {
        if !programs_compiled() {
            bail!(
                "machina-bpf was built without eBPF programs (install nightly + bpf-linker and rebuild)"
            );
        }
        bump_memlock();
        let mut loader = EbpfLoader::new();
        if std::env::var_os("MACHINA_BPF_VERIFIER_LOG").is_some() {
            loader.verifier_log_level(VerifierLogLevel::VERBOSE | VerifierLogLevel::STATS);
        }
        let ebpf = loader
            .load(BPF_OBJECT)
            .context("load machina-bpf eBPF object")?;
        Ok(Self {
            ebpf,
            loaded: HashSet::new(),
            tc: StdHashMap::new(),
            uplink: StdHashMap::new(),
            xdp: StdHashMap::new(),
            cgroups: StdHashMap::new(),
            tracepoints: Vec::new(),
            notes: Vec::new(),
            tcx: kernel_features().tcx,
        })
    }

    fn classifier(&mut self, name: &str) -> Result<&mut SchedClassifier> {
        let first = !self.loaded.contains(name);
        let p: &mut SchedClassifier = self
            .ebpf
            .program_mut(name)
            .ok_or_else(|| anyhow!("program {name} missing"))?
            .try_into()?;
        if first {
            p.load().with_context(|| format!("verifier rejected {name}"))?;
            self.loaded.insert(name.to_string());
        }
        Ok(p)
    }

    fn ensure_clsact(&self, iface: &str) {
        if !self.tcx {
            let _ = aya::programs::tc::qdisc_add_clsact(iface);
        }
    }

    /// Attach a TCX ingress+egress program pair to `iface`.
    pub fn attach_tc_pair(&mut self, iface: &str, ingress: &str, egress: &str) -> Result<()> {
        if self.tc.contains_key(iface) {
            return Ok(());
        }
        self.ensure_clsact(iface);
        let i = self
            .classifier(ingress)?
            .attach(iface, TcAttachType::Ingress)
            .with_context(|| format!("attach {ingress} to {iface}"))?;
        let e = match self.classifier(egress)?.attach(iface, TcAttachType::Egress) {
            Ok(e) => e,
            Err(err) => {
                let _ = self.classifier(ingress)?.detach(i);
                return Err(anyhow!(err).context(format!("attach {egress} to {iface}")));
            }
        };
        self.tc.insert(
            iface.to_string(),
            (format!("{ingress}|{egress}"), TcLinks { ingress: i, egress: e }),
        );
        Ok(())
    }

    /// Host datapath on a workload tap.
    pub fn attach_tc(&mut self, iface: &str) -> Result<()> {
        self.attach_tc_pair(iface, "mn_tc_ingress", "mn_tc_egress")
    }

    pub fn detach_tc(&mut self, iface: &str) {
        let Some((names, links)) = self.tc.remove(iface) else {
            return;
        };
        let (ing, eg) = names.split_once('|').unwrap_or(("mn_tc_ingress", "mn_tc_egress"));
        let (ing, eg) = (ing.to_string(), eg.to_string());
        if let Ok(p) = self.classifier(&ing) {
            let _ = p.detach(links.ingress);
        }
        if let Ok(p) = self.classifier(&eg) {
            let _ = p.detach(links.egress);
        }
    }

    /// Forget links for an interface that disappeared (kernel already dropped them).
    pub fn forget_tc(&mut self, iface: &str) {
        self.tc.remove(iface);
        self.uplink.remove(iface);
        self.xdp.remove(iface);
    }

    pub fn tc_attached(&self) -> Vec<String> {
        self.tc.keys().cloned().collect()
    }

    /// Single-direction classifier (e.g. CNI NodePort on the uplink).
    pub fn attach_tc_one(&mut self, iface: &str, prog: &str, ingress: bool) -> Result<()> {
        let key = format!("{iface}:{prog}");
        if self.uplink.contains_key(&key) {
            return Ok(());
        }
        self.ensure_clsact(iface);
        let dir = if ingress { TcAttachType::Ingress } else { TcAttachType::Egress };
        let id = self
            .classifier(prog)?
            .attach(iface, dir)
            .with_context(|| format!("attach {prog} to {iface}"))?;
        self.uplink.insert(key, (prog.to_string(), id));
        Ok(())
    }

    pub fn attach_xdp(&mut self, iface: &str) -> Result<()> {
        self.attach_xdp_prog(iface, "mn_xdp_deny")
    }

    pub(crate) fn xdp_program(&mut self, name: &str) -> Result<&mut Xdp> {
        let first = !self.loaded.contains(name);
        let p: &mut Xdp = self
            .ebpf
            .program_mut(name)
            .ok_or_else(|| anyhow!("program {name} missing"))?
            .try_into()?;
        if first {
            p.load().with_context(|| format!("verifier rejected {name}"))?;
            self.loaded.insert(name.to_string());
        }
        Ok(p)
    }

    /// Attach an XDP program (native, falling back to generic/skb mode).
    /// One XDP program per interface; a different one replaces it.
    pub fn attach_xdp_prog(&mut self, iface: &str, prog: &str) -> Result<()> {
        match self.xdp.get(iface) {
            Some((p, _)) if p == prog => return Ok(()),
            Some(_) => self.detach_xdp(iface),
            None => {}
        }
        let p = self.xdp_program(prog)?;
        let id = p
            .attach(iface, XdpMode::default())
            .or_else(|_| p.attach(iface, XdpMode::Skb))
            .with_context(|| format!("attach {prog} to {iface}"))?;
        self.xdp.insert(iface.to_string(), (prog.to_string(), id));
        Ok(())
    }

    pub fn xdp_attached(&self, iface: &str) -> Option<&str> {
        self.xdp.get(iface).map(|(p, _)| p.as_str())
    }

    pub fn detach_xdp(&mut self, iface: &str) {
        if let Some((prog, id)) = self.xdp.remove(iface) {
            if let Some(p) = self.ebpf.program_mut(&prog) {
                if let Ok(p) = <&mut Xdp>::try_from(p) {
                    let _ = p.detach(id);
                }
            }
        }
    }

    /// Attach sock_addr + cgroup_skb programs to a cgroup v2 directory.
    pub fn attach_cgroup(&mut self, cg_path: &Path, sock_progs: &[&str], skb: bool) -> Result<()> {
        let key = cg_path.display().to_string();
        if self.cgroups.contains_key(&key) {
            return Ok(());
        }
        let mut links = CgroupLinks::default();
        for name in sock_progs {
            let first = !self.loaded.contains(*name);
            let p: &mut CgroupSockAddr = self
                .ebpf
                .program_mut(name)
                .ok_or_else(|| anyhow!("program {name} missing"))?
                .try_into()?;
            if first {
                p.load().with_context(|| format!("verifier rejected {name}"))?;
                self.loaded.insert(name.to_string());
            }
            let f = File::open(cg_path).with_context(|| format!("open {}", cg_path.display()))?;
            let id = p.attach(f, CgroupAttachMode::default())?;
            links.sock.push((name.to_string(), id));
        }
        if skb {
            for (name, ty) in [
                ("mn_cg_skb_ingress", CgroupSkbAttachType::Ingress),
                ("mn_cg_skb_egress", CgroupSkbAttachType::Egress),
            ] {
                let first = !self.loaded.contains(name);
                let p: &mut CgroupSkb = self
                    .ebpf
                    .program_mut(name)
                    .ok_or_else(|| anyhow!("program {name} missing"))?
                    .try_into()?;
                if first {
                    p.load().with_context(|| format!("verifier rejected {name}"))?;
                    self.loaded.insert(name.to_string());
                }
                let f = File::open(cg_path)?;
                let id = p.attach(f, ty, CgroupAttachMode::default())?;
                links.skb.push((name.to_string(), id));
            }
        }
        self.cgroups.insert(key, links);
        Ok(())
    }

    /// Attach sock_addr programs next to whatever else runs on `cg_path`,
    /// tracked under `<path>#<tag>`. The default mode uses a bpf_link, which
    /// the kernel always attaches multi-mode; passing `AllowMultiple` sets
    /// BPF_F_ALLOW_MULTI on link_create and fails with EINVAL.
    pub fn attach_cgroup_tagged(&mut self, cg_path: &Path, tag: &str, sock_progs: &[&str]) -> Result<()> {
        let key = format!("{}#{tag}", cg_path.display());
        if self.cgroups.contains_key(&key) {
            return Ok(());
        }
        let mut links = CgroupLinks::default();
        for name in sock_progs {
            let first = !self.loaded.contains(*name);
            let p: &mut CgroupSockAddr = self
                .ebpf
                .program_mut(name)
                .ok_or_else(|| anyhow!("program {name} missing"))?
                .try_into()?;
            if first {
                p.load().with_context(|| format!("verifier rejected {name}"))?;
                self.loaded.insert(name.to_string());
            }
            let f = File::open(cg_path).with_context(|| format!("open {}", cg_path.display()))?;
            let id = p.attach(f, CgroupAttachMode::default())?;
            links.sock.push((name.to_string(), id));
        }
        self.cgroups.insert(key, links);
        Ok(())
    }

    pub fn detach_cgroup(&mut self, cg_path: &Path) {
        let Some(links) = self.cgroups.remove(&cg_path.display().to_string()) else {
            return;
        };
        for (name, id) in links.sock {
            if let Some(p) = self.ebpf.program_mut(&name) {
                if let Ok(p) = <&mut CgroupSockAddr>::try_from(p) {
                    let _ = p.detach(id);
                }
            }
        }
        for (name, id) in links.skb {
            if let Some(p) = self.ebpf.program_mut(&name) {
                if let Ok(p) = <&mut CgroupSkb>::try_from(p) {
                    let _ = p.detach(id);
                }
            }
        }
    }

    pub fn cgroups(&self) -> Vec<String> {
        self.cgroups.keys().cloned().collect()
    }

    /// Attach telemetry tracepoints and the capability kprobe. Failures are
    /// recorded in `notes` (a missing tracepoint disables only that feature).
    pub fn attach_telemetry(&mut self) {
        for (prog, cat, ev) in crate::tracefs::TRACEPOINTS {
            let res: Result<()> = (|| {
                let p: &mut TracePoint = self
                    .ebpf
                    .program_mut(prog)
                    .ok_or_else(|| anyhow!("program {prog} missing"))?
                    .try_into()?;
                p.load()?;
                p.attach(cat, ev)?;
                Ok(())
            })();
            match res {
                Ok(()) => self.tracepoints.push(format!("{cat}/{ev}")),
                Err(e) => self.notes.push(format!("tracepoint {cat}/{ev} unavailable: {e:#}")),
            }
        }
        let res: Result<()> = (|| {
            let p: &mut KProbe = self
                .ebpf
                .program_mut("mn_kp_cap_capable")
                .ok_or_else(|| anyhow!("program mn_kp_cap_capable missing"))?
                .try_into()?;
            p.load()?;
            p.attach("cap_capable", 0)?;
            Ok(())
        })();
        match res {
            Ok(()) => self.tracepoints.push("kprobe/cap_capable".into()),
            Err(e) => self.notes.push(format!("kprobe cap_capable unavailable (deny_cap disabled): {e:#}")),
        }
    }
}
