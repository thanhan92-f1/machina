// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! VM network policy: the CiliumNetworkPolicy schema applied to VMs.
//!
//! VMs are the endpoints and VM labels the endpoint labels, so a
//! `cilium.io/v2` CiliumNetworkPolicy / CiliumClusterwideNetworkPolicy
//! applies unchanged; the native kind is `machina.io/v1` `VmNetworkPolicy`
//! (alias `VmClusterwideNetworkPolicy`). There are no namespaces: every
//! policy is fleet-wide.
//!
//! Every VM also carries `machina.io/vm-name`, `machina.io/host` and
//! (when known) `machina.io/project`, so selectors can pin one VM, one
//! hypervisor or one project.
//!
//! [`compile`] turns policies + VM inventory into a [`crate::api::VmEdgeState`]
//! for one host's bpfd; [`trace`] evaluates a connection the same way the
//! datapath does.

mod compile;
mod flow;
pub mod fqdn;
mod trace;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub use compile::{cidr_identity, compile, vm_identity, Compiled, EndpointInfo, Inputs, NetpolVm, SelectorInfo, IDENTITY_REMOTE_NODE};
pub use flow::FlowFilter;
pub use trace::{trace, TraceEndpoint, TraceQuery, TraceResult, TraceSide};

/// Cilium on this host (`cilium_host` link, CNI config or a running
/// cilium-agent). libvirt taps are never Cilium endpoints, so the VM edge
/// stays Machina's either way; this is reported for KubeVirt delegation.
fn cni_config_name(name: &str) -> bool {
    [".conf", ".conflist", ".json"].iter().any(|ext| name.ends_with(ext))
}

pub fn cilium_present() -> Option<String> {
    if std::path::Path::new("/sys/class/net/cilium_host").exists() {
        return Some("cilium_host interface".into());
    }
    if let Ok(rd) = std::fs::read_dir("/etc/cni/net.d") {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            // Only files the CNI runtime loads; Cilium renames configs it
            // displaces to `*.cilium_bak`, which outlive an uninstall.
            if name.contains("cilium") && cni_config_name(&name) {
                return Some(format!("/etc/cni/net.d/{name}"));
            }
        }
    }
    if let Ok(rd) = std::fs::read_dir("/proc") {
        for e in rd.flatten() {
            if let Ok(c) = std::fs::read_to_string(e.path().join("comm")) {
                if c.trim() == "cilium-agent" {
                    return Some("cilium-agent process".into());
                }
            }
        }
    }
    None
}

pub const API_VERSION: &str = "machina.io/v1";
pub const KIND: &str = "VmNetworkPolicy";
const KINDS: [&str; 4] =
    ["VmNetworkPolicy", "VmClusterwideNetworkPolicy", "CiliumNetworkPolicy", "CiliumClusterwideNetworkPolicy"];
const API_VERSIONS: [&str; 2] = ["machina.io/v1", "cilium.io/v2"];

pub const LABEL_VM_NAME: &str = "machina.io/vm-name";
pub const LABEL_HOST: &str = "machina.io/host";
pub const LABEL_PROJECT: &str = "machina.io/project";
/// `machina.io/port.<name>: "<number>"` names a port on a VM for `toPorts`.
pub const LABEL_PORT_PREFIX: &str = "machina.io/port.";

pub const MAX_PORT_RANGE: u32 = 256;

/// (subject identity, peer identity, egress, proto, port) → (deny, source).
pub type RuleIndex = std::collections::HashMap<(u32, u32, bool, u8, u16), (bool, String)>;

/// One stored policy. `specs` holds the Cilium rule objects (one per
/// `spec` / `specs` entry) exactly as written.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VmNetworkPolicy {
    pub name: String,
    /// Kind as written (`VmNetworkPolicy`, `CiliumNetworkPolicy`, ...).
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, String>,
    pub specs: Vec<Value>,
}

fn default_kind() -> String {
    KIND.into()
}

/// A validation problem with a Cilium-style field path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Issue {
    pub path: String,
    pub message: String,
}

impl std::fmt::Display for Issue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Validation {
    pub errors: Vec<Issue>,
    /// Accepted but not enforced natively yet (L7, groups, ...).
    pub warnings: Vec<Issue>,
}

impl Validation {
    fn err(&mut self, path: impl Into<String>, msg: impl Into<String>) {
        self.errors.push(Issue { path: path.into(), message: msg.into() });
    }
    fn warn(&mut self, path: impl Into<String>, msg: impl Into<String>) {
        self.warnings.push(Issue { path: path.into(), message: msg.into() });
    }
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

impl VmNetworkPolicy {
    /// The policy as a document (`spec` when it has one rule, else `specs`).
    pub fn to_document(&self) -> Value {
        let api = if self.kind.starts_with("Cilium") { "cilium.io/v2" } else { API_VERSION };
        let mut meta = Map::new();
        meta.insert("name".into(), Value::String(self.name.clone()));
        if !self.labels.is_empty() {
            meta.insert("labels".into(), serde_json::to_value(&self.labels).unwrap_or_default());
        }
        if !self.annotations.is_empty() {
            meta.insert("annotations".into(), serde_json::to_value(&self.annotations).unwrap_or_default());
        }
        let mut doc = Map::new();
        doc.insert("apiVersion".into(), Value::String(api.into()));
        doc.insert("kind".into(), Value::String(self.kind.clone()));
        doc.insert("metadata".into(), Value::Object(meta));
        match self.specs.as_slice() {
            [one] => doc.insert("spec".into(), one.clone()),
            many => doc.insert("specs".into(), Value::Array(many.to_vec())),
        };
        Value::Object(doc)
    }

    pub fn to_yaml(&self) -> String {
        serde_yaml::to_string(&self.to_document()).unwrap_or_default()
    }

    pub fn description(&self) -> Option<String> {
        self.specs.iter().find_map(|s| s["description"].as_str().map(String::from))
    }
}

/// Parse YAML or JSON: one or more `---` documents, a top-level array, or a
/// `kind: List` with `items`. Returns the policies plus every issue found.
pub fn parse_documents(text: &str) -> (Vec<VmNetworkPolicy>, Validation) {
    let mut out = Vec::new();
    let mut v = Validation::default();
    let mut docs = Vec::new();
    for (i, de) in serde_yaml::Deserializer::from_str(text).enumerate() {
        match serde_yaml::Value::deserialize(de) {
            Ok(serde_yaml::Value::Null) => {}
            Ok(y) => match serde_json::to_value(y) {
                Ok(j) => docs.push((i, j)),
                Err(e) => v.err(format!("document[{i}]"), format!("not representable as JSON: {e}")),
            },
            Err(e) => v.err(format!("document[{i}]"), format!("invalid YAML: {e}")),
        }
    }
    let mut flat = Vec::new();
    for (i, d) in docs {
        match d {
            Value::Array(items) => flat.extend(items.into_iter().enumerate().map(|(j, x)| (format!("document[{i}][{j}]"), x))),
            Value::Object(ref m) if m.get("kind").and_then(Value::as_str) == Some("List") => {
                for (j, x) in m.get("items").and_then(Value::as_array).cloned().unwrap_or_default().into_iter().enumerate() {
                    flat.push((format!("document[{i}].items[{j}]"), x));
                }
            }
            other => flat.push((format!("document[{i}]"), other)),
        }
    }
    let mut names = std::collections::BTreeSet::new();
    for (path, d) in flat {
        if let Some(p) = from_value(&d, &path, &mut v) {
            if !names.insert(p.name.clone()) {
                v.err(format!("{path}.metadata.name"), format!("duplicate policy `{}` in input", p.name));
                continue;
            }
            out.push(p);
        }
    }
    if out.is_empty() && v.errors.is_empty() {
        v.err("document", "no policy documents found");
    }
    (out, v)
}

fn valid_name(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 253
        && n.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
        && n.as_bytes()[0].is_ascii_alphanumeric()
        && n.as_bytes()[n.len() - 1].is_ascii_alphanumeric()
}

fn string_map(v: &Value) -> BTreeMap<String, String> {
    v.as_object()
        .map(|m| m.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect())
        .unwrap_or_default()
}

/// One document → policy, recording issues under `path`.
pub fn from_value(d: &Value, path: &str, v: &mut Validation) -> Option<VmNetworkPolicy> {
    let Some(obj) = d.as_object() else {
        v.err(path, "expected a mapping");
        return None;
    };
    let before = v.errors.len();
    let api = obj.get("apiVersion").and_then(Value::as_str).unwrap_or("");
    if !API_VERSIONS.contains(&api) {
        v.err(format!("{path}.apiVersion"), format!("`{api}`: expected one of {}", API_VERSIONS.join(", ")));
    }
    let kind = obj.get("kind").and_then(Value::as_str).unwrap_or("");
    if !KINDS.contains(&kind) {
        v.err(format!("{path}.kind"), format!("`{kind}`: expected one of {}", KINDS.join(", ")));
    }
    for k in obj.keys() {
        if !matches!(k.as_str(), "apiVersion" | "kind" | "metadata" | "spec" | "specs" | "status") {
            v.err(format!("{path}.{k}"), "unknown field");
        }
    }
    let meta = &obj.get("metadata").cloned().unwrap_or(Value::Null);
    let name = meta["name"].as_str().unwrap_or("");
    if !valid_name(name) {
        v.err(format!("{path}.metadata.name"), "required: lowercase letters, digits, `-` and `.` (max 253)");
    }
    if meta.get("namespace").is_some() {
        v.warn(format!("{path}.metadata.namespace"), "ignored: VM policies are fleet-wide");
    }
    let mut specs = Vec::new();
    if let Some(s) = obj.get("spec").filter(|s| !s.is_null()) {
        specs.push(s.clone());
    }
    if let Some(list) = obj.get("specs").filter(|s| !s.is_null()) {
        match list.as_array() {
            Some(a) => specs.extend(a.iter().cloned()),
            None => v.err(format!("{path}.specs"), "expected a list"),
        }
    }
    if specs.is_empty() {
        v.err(path, "spec or specs is required");
    }
    for (i, s) in specs.iter().enumerate() {
        let sp = if obj.get("spec").is_some_and(|s| !s.is_null()) && i == 0 {
            format!("{path}.spec")
        } else {
            let off = usize::from(obj.get("spec").is_some_and(|s| !s.is_null()));
            format!("{path}.specs[{}]", i - off)
        };
        validate_spec(s, &sp, v);
    }
    if v.errors.len() > before {
        return None;
    }
    let mut annotations = string_map(&meta["annotations"]);
    if kind.starts_with("Cilium") {
        annotations.insert("machina.io/source-kind".into(), kind.into());
    }
    Some(VmNetworkPolicy { name: name.into(), kind: kind.into(), labels: string_map(&meta["labels"]), annotations, specs })
}

/// Validate one policy already in storage form (API JSON bodies).
pub fn validate_policy(p: &VmNetworkPolicy) -> Validation {
    let mut v = Validation::default();
    from_value(&p.to_document(), &p.name, &mut v);
    v
}

const ENTITIES: [&str; 13] = [
    "all", "world", "world-ipv4", "world-ipv6", "unmanaged", "host", "remote-node", "cluster", "fleet", "health",
    "init", "kube-apiserver", "ingress",
];
pub(crate) const UNSUPPORTED_ENTITIES: [&str; 4] = ["health", "init", "kube-apiserver", "ingress"];

fn keys_only(v: &Value, path: &str, allowed: &[&str], val: &mut Validation) -> bool {
    let Some(m) = v.as_object() else {
        val.err(path, "expected a mapping");
        return false;
    };
    for k in m.keys() {
        if !allowed.contains(&k.as_str()) {
            val.err(format!("{path}.{k}"), "unknown field");
        }
    }
    true
}

fn list<'a>(v: &'a Value, key: &str, path: &str, val: &mut Validation) -> &'a [Value] {
    match v.get(key) {
        None | Some(Value::Null) => &[],
        Some(Value::Array(a)) => a,
        Some(_) => {
            val.err(format!("{path}.{key}"), "expected a list");
            &[]
        }
    }
}

pub(crate) fn validate_selector(sel: &Value, path: &str, v: &mut Validation) {
    if !keys_only(sel, path, &["matchLabels", "matchExpressions"], v) {
        return;
    }
    if let Some(ml) = sel.get("matchLabels") {
        match ml.as_object() {
            Some(m) => {
                for (k, x) in m {
                    if !x.is_string() {
                        v.err(format!("{path}.matchLabels.{k}"), "label values are strings");
                    }
                }
            }
            None => v.err(format!("{path}.matchLabels"), "expected a mapping"),
        }
    }
    for (i, e) in list(sel, "matchExpressions", path, v).iter().enumerate() {
        let p = format!("{path}.matchExpressions[{i}]");
        if !keys_only(e, &p, &["key", "operator", "values"], v) {
            continue;
        }
        if e["key"].as_str().is_none_or(str::is_empty) {
            v.err(format!("{p}.key"), "required");
        }
        let op = e["operator"].as_str().unwrap_or("");
        let n = e["values"].as_array().map_or(0, Vec::len);
        match op {
            "In" | "NotIn" if n == 0 => v.err(format!("{p}.values"), format!("{op} needs at least one value")),
            "Exists" | "DoesNotExist" if n > 0 => v.err(format!("{p}.values"), format!("{op} takes no values")),
            "In" | "NotIn" | "Exists" | "DoesNotExist" => {}
            _ => v.err(format!("{p}.operator"), format!("`{op}`: expected In, NotIn, Exists or DoesNotExist")),
        }
    }
}

fn validate_cidr(c: &Value, path: &str, v: &mut Validation) {
    match c.as_str() {
        Some(s) if s.contains('/') && crate::policy::parse_prefix(s).is_ok() => {}
        Some(s) => v.err(path, format!("`{s}` is not a CIDR (address/prefix)")),
        None => v.err(path, "expected a string"),
    }
}

pub(crate) fn icmp_type_num(t: &Value, v6: bool) -> Option<u8> {
    if let Some(n) = t.as_u64() {
        return u8::try_from(n).ok();
    }
    let s = t.as_str()?;
    if let Ok(n) = s.parse::<u8>() {
        return Some(n);
    }
    let v4 = [
        ("EchoReply", 0),
        ("DestinationUnreachable", 3),
        ("Redirect", 5),
        ("Echo", 8),
        ("EchoRequest", 8),
        ("RouterAdvertisement", 9),
        ("RouterSelection", 10),
        ("TimeExceeded", 11),
        ("ParameterProblem", 12),
        ("Timestamp", 13),
        ("TimestampReply", 14),
        ("Photuris", 40),
        ("ExtendedEchoRequest", 42),
        ("ExtendedEchoReply", 43),
    ];
    let v6t = [
        ("DestinationUnreachable", 1),
        ("PacketTooBig", 2),
        ("TimeExceeded", 3),
        ("ParameterProblem", 4),
        ("EchoRequest", 128),
        ("EchoReply", 129),
        ("MulticastListenerQuery", 130),
        ("MulticastListenerReport", 131),
        ("MulticastListenerDone", 132),
        ("RouterSolicitation", 133),
        ("RouterAdvertisement", 134),
        ("NeighborSolicitation", 135),
        ("NeighborAdvertisement", 136),
        ("RedirectMessage", 137),
        ("RouterRenumbering", 138),
        ("ICMPNodeInformationQuery", 139),
        ("ICMPNodeInformationResponse", 140),
        ("InverseNeighborDiscoverySolicitation", 141),
        ("InverseNeighborDiscoveryAdvertisement", 142),
        ("HomeAgentAddressDiscoveryRequest", 144),
        ("HomeAgentAddressDiscoveryReply", 145),
        ("MobilePrefixSolicitation", 146),
        ("MobilePrefixAdvertisement", 147),
        ("DuplicateAddressRequestCodeSuffix", 157),
        ("DuplicateAddressConfirmationCodeSuffix", 158),
        ("ExtendedEchoRequest", 160),
        ("ExtendedEchoReply", 161),
    ];
    let table: &[(&str, u8)] = if v6 { &v6t } else { &v4 };
    table.iter().find(|(n, _)| *n == s).map(|(_, t)| *t)
}

fn validate_ports(tp: &Value, path: &str, deny: bool, v: &mut Validation) {
    let allowed: &[&str] = if deny {
        &["ports"]
    } else {
        &["ports", "rules", "terminatingTLS", "originatingTLS", "serverNames", "listener"]
    };
    if !keys_only(tp, path, allowed, v) {
        return;
    }
    for (i, p) in list(tp, "ports", path, v).iter().enumerate() {
        let pp = format!("{path}.ports[{i}]");
        if !keys_only(p, &pp, &["port", "endPort", "protocol"], v) {
            continue;
        }
        let proto = p["protocol"].as_str().unwrap_or("ANY");
        if !matches!(proto, "TCP" | "UDP" | "SCTP" | "ANY") {
            v.err(format!("{pp}.protocol"), format!("`{proto}`: expected TCP, UDP, SCTP or ANY"));
        }
        let port = match &p["port"] {
            Value::Null => "0".to_string(),
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => {
                v.err(format!("{pp}.port"), "expected a port number or name");
                continue;
            }
        };
        match port.parse::<u32>() {
            Ok(n) if n > 65535 => v.err(format!("{pp}.port"), "port above 65535"),
            Ok(n) => {
                if let Some(end) = p.get("endPort").filter(|e| !e.is_null()) {
                    match end.as_u64() {
                        Some(_) if n == 0 => v.err(format!("{pp}.endPort"), "endPort needs a start port"),
                        Some(e) if (e as u32) < n || e > 65535 => {
                            v.err(format!("{pp}.endPort"), format!("must be between {n} and 65535"))
                        }
                        Some(e) if e as u32 - n >= MAX_PORT_RANGE => {
                            v.err(format!("{pp}.endPort"), format!("range wider than {MAX_PORT_RANGE} ports"))
                        }
                        Some(_) => {}
                        None => v.err(format!("{pp}.endPort"), "expected a number"),
                    }
                }
            }
            Err(_) => {
                let ok = !port.is_empty()
                    && port.len() <= 15
                    && port.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
                if !ok {
                    v.err(format!("{pp}.port"), format!("`{port}` is neither a number nor a port name"));
                }
            }
        }
    }
    if !deny {
        if tp.get("rules").is_some_and(|r| !r.is_null()) {
            v.warn(format!("{path}.rules"), "L7 rules are not enforced natively yet; the rule enforces as L4");
        }
        for k in ["terminatingTLS", "originatingTLS", "serverNames", "listener"] {
            if tp.get(k).is_some_and(|r| !r.is_null()) {
                v.warn(format!("{path}.{k}"), "not enforced natively yet");
            }
        }
    }
}

fn validate_rule(r: &Value, path: &str, egress: bool, deny: bool, v: &mut Validation) {
    let pre = if egress { "to" } else { "from" };
    let mut allowed: Vec<String> = ["Endpoints", "Requires", "CIDR", "CIDRSet", "Entities", "Groups", "Nodes"]
        .iter()
        .map(|s| format!("{pre}{s}"))
        .collect();
    allowed.extend(["toPorts", "icmps"].map(String::from));
    if egress {
        allowed.push("toServices".into());
    }
    if egress && !deny {
        allowed.push("toFQDNs".into());
    }
    if !deny {
        allowed.push("authentication".into());
    }
    let refs: Vec<&str> = allowed.iter().map(String::as_str).collect();
    if !keys_only(r, path, &refs, v) {
        return;
    }
    for key in ["Endpoints", "Requires"] {
        let k = format!("{pre}{key}");
        for (i, s) in list(r, &k, path, v).iter().enumerate() {
            validate_selector(s, &format!("{path}.{k}[{i}]"), v);
        }
    }
    let k = format!("{pre}CIDR");
    for (i, c) in list(r, &k, path, v).iter().enumerate() {
        validate_cidr(c, &format!("{path}.{k}[{i}]"), v);
    }
    let k = format!("{pre}CIDRSet");
    for (i, set) in list(r, &k, path, v).iter().enumerate() {
        let sp = format!("{path}.{k}[{i}]");
        if !keys_only(set, &sp, &["cidr", "except", "cidrGroupRef"], v) {
            continue;
        }
        if set.get("cidrGroupRef").is_some() {
            v.warn(format!("{sp}.cidrGroupRef"), "CiliumCIDRGroup references are not supported; use cidr");
        }
        match set.get("cidr") {
            Some(c) => validate_cidr(c, &format!("{sp}.cidr"), v),
            None if set.get("cidrGroupRef").is_none() => v.err(format!("{sp}.cidr"), "required"),
            None => {}
        }
        for (j, e) in list(set, "except", &sp, v).iter().enumerate() {
            validate_cidr(e, &format!("{sp}.except[{j}]"), v);
        }
    }
    let k = format!("{pre}Entities");
    for (i, e) in list(r, &k, path, v).iter().enumerate() {
        let ep = format!("{path}.{k}[{i}]");
        match e.as_str() {
            Some(s) if UNSUPPORTED_ENTITIES.contains(&s) => v.warn(ep, format!("entity `{s}` has no VM equivalent; it matches nothing")),
            Some(s) if ENTITIES.contains(&s) => {}
            Some(s) => v.err(ep, format!("unknown entity `{s}`")),
            None => v.err(ep, "expected a string"),
        }
    }
    for key in ["Groups", "Nodes"] {
        let k = format!("{pre}{key}");
        if !list(r, &k, path, v).is_empty() {
            v.warn(format!("{path}.{k}"), "not enforced natively yet; matches nothing");
        }
    }
    if egress {
        if !list(r, "toServices", path, v).is_empty() {
            v.warn(format!("{path}.toServices"), "not enforced natively yet; matches nothing (audited in observe mode)");
        }
        for (i, s) in list(r, "toFQDNs", path, v).iter().enumerate() {
            let sp = format!("{path}.toFQDNs[{i}]");
            if !keys_only(s, &sp, &["matchName", "matchPattern"], v) {
                continue;
            }
            match (s.get("matchName"), s.get("matchPattern")) {
                (Some(_), Some(_)) | (None, None) => v.err(sp, "set exactly one of matchName / matchPattern"),
                (Some(n), None) | (None, Some(n)) => {
                    let pattern = s.get("matchPattern").is_some();
                    let key = if pattern { "matchPattern" } else { "matchName" };
                    match n.as_str() {
                        Some(t) => {
                            if let Some(why) = fqdn::invalid(t, pattern) {
                                v.err(format!("{sp}.{key}"), why);
                            }
                        }
                        None => v.err(format!("{sp}.{key}"), "expected a string"),
                    }
                }
            }
        }
    }
    if r.get("authentication").is_some_and(|a| !a.is_null()) {
        v.warn(format!("{path}.authentication"), "mutual authentication is not enforced natively yet");
    }
    for (i, tp) in list(r, "toPorts", path, v).iter().enumerate() {
        validate_ports(tp, &format!("{path}.toPorts[{i}]"), deny, v);
    }
    for (i, ic) in list(r, "icmps", path, v).iter().enumerate() {
        let ip = format!("{path}.icmps[{i}]");
        if !keys_only(ic, &ip, &["fields"], v) {
            continue;
        }
        for (j, f) in list(ic, "fields", &ip, v).iter().enumerate() {
            let fp = format!("{ip}.fields[{j}]");
            if !keys_only(f, &fp, &["type", "family"], v) {
                continue;
            }
            let fam = f["family"].as_str().unwrap_or("IPv4");
            if !matches!(fam, "IPv4" | "IPv6") {
                v.err(format!("{fp}.family"), format!("`{fam}`: expected IPv4 or IPv6"));
            }
            if icmp_type_num(&f["type"], fam == "IPv6").is_none() {
                v.err(format!("{fp}.type"), "unknown ICMP type");
            }
        }
    }
}

fn validate_spec(s: &Value, path: &str, v: &mut Validation) {
    let allowed = [
        "endpointSelector", "nodeSelector", "ingress", "egress", "ingressDeny", "egressDeny", "enableDefaultDeny",
        "description", "labels",
    ];
    if !keys_only(s, path, &allowed, v) {
        return;
    }
    match (s.get("endpointSelector"), s.get("nodeSelector")) {
        (Some(_), Some(_)) => v.err(path, "endpointSelector and nodeSelector are mutually exclusive"),
        (None, None) => v.err(format!("{path}.endpointSelector"), "required"),
        (Some(sel), None) => validate_selector(sel, &format!("{path}.endpointSelector"), v),
        (None, Some(sel)) => {
            validate_selector(sel, &format!("{path}.nodeSelector"), v);
            v.warn(format!("{path}.nodeSelector"), "host policies are not enforced on the VM edge");
        }
    }
    if let Some(dd) = s.get("enableDefaultDeny") {
        if keys_only(dd, &format!("{path}.enableDefaultDeny"), &["ingress", "egress"], v) {
            for k in ["ingress", "egress"] {
                if dd.get(k).is_some_and(|x| !x.is_boolean()) {
                    v.err(format!("{path}.enableDefaultDeny.{k}"), "expected true or false");
                }
            }
        }
    }
    for (key, egress, deny) in
        [("ingress", false, false), ("egress", true, false), ("ingressDeny", false, true), ("egressDeny", true, true)]
    {
        for (i, r) in list(s, key, path, v).iter().enumerate() {
            validate_rule(r, &format!("{path}.{key}[{i}]"), egress, deny, v);
        }
    }
}

/// Labels a selector sees for one VM: user labels plus the reserved
/// `machina.io/*` ones.
pub fn effective_labels(vm: &NetpolVm) -> BTreeMap<String, String> {
    let mut l = vm.labels.clone();
    l.insert(LABEL_VM_NAME.into(), vm.name.clone());
    if let Some(h) = vm.host.as_deref().filter(|h| !h.is_empty()) {
        l.insert(LABEL_HOST.into(), h.into());
    }
    if let Some(p) = vm.project.as_deref().filter(|p| !p.is_empty()) {
        l.insert(LABEL_PROJECT.into(), p.into());
    }
    l
}

fn strip_source(k: &str) -> &str {
    ["k8s:", "any:", "machina:"].iter().find_map(|p| k.strip_prefix(p)).unwrap_or(k)
}

/// Kubernetes/Cilium label selector; `k8s:`/`any:`/`machina:` key prefixes
/// are ignored. `{}` matches everything.
pub fn selector_matches(sel: &Value, labels: &BTreeMap<String, String>) -> bool {
    if let Some(ml) = sel.get("matchLabels").and_then(Value::as_object) {
        for (k, v) in ml {
            if labels.get(strip_source(k)).map(String::as_str) != v.as_str() {
                return false;
            }
        }
    }
    for e in sel.get("matchExpressions").and_then(Value::as_array).into_iter().flatten() {
        let key = strip_source(e["key"].as_str().unwrap_or(""));
        let values: Vec<&str> = e["values"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        let have = labels.get(key).map(String::as_str);
        let ok = match e["operator"].as_str().unwrap_or("") {
            "In" => have.is_some_and(|h| values.contains(&h)),
            "NotIn" => have.is_none_or(|h| !values.contains(&h)),
            "Exists" => have.is_some(),
            "DoesNotExist" => have.is_none(),
            _ => false,
        };
        if !ok {
            return false;
        }
    }
    true
}

/// `reserved:<entity>` selectors (`matchLabels: {reserved:host: ""}`) name
/// an entity instead of VMs.
pub(crate) fn reserved_entity(sel: &Value) -> Option<String> {
    let ml = sel.get("matchLabels")?.as_object()?;
    ml.keys().find_map(|k| k.strip_prefix("reserved:").map(String::from))
}

/// `app=web,env in (prod)` style text for a selector.
pub fn selector_string(sel: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(ml) = sel.get("matchLabels").and_then(Value::as_object) {
        for (k, v) in ml {
            parts.push(format!("{k}={}", v.as_str().unwrap_or("")));
        }
    }
    for e in sel.get("matchExpressions").and_then(Value::as_array).into_iter().flatten() {
        let key = e["key"].as_str().unwrap_or("");
        let vals: Vec<&str> = e["values"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        parts.push(match e["operator"].as_str().unwrap_or("") {
            "In" => format!("{key} in ({})", vals.join(",")),
            "NotIn" => format!("{key} notin ({})", vals.join(",")),
            "Exists" => key.to_string(),
            "DoesNotExist" => format!("!{key}"),
            op => format!("{key} {op}"),
        });
    }
    if parts.is_empty() {
        "<all>".into()
    } else {
        parts.join(",")
    }
}
