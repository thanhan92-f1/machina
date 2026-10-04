// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! L7 rules of `toPorts` (`rules.http`, `rules.kafka`, `rules.dns`,
//! `serverNames`): the rule model, the request parsers and the matcher.
//! Shared by bpfd (live verdicts) and the policy tracer.
//!
//! Matching follows Cilium: HTTP `method` / `path` / `host` are anchored
//! regular expressions, `headers` are `Name` (present) or `Name: value`
//! (exact); Kafka `role` expands to its API keys and every topic of a
//! request must be allowed; DNS and `serverNames` use `toFQDNs` patterns.
//! A request is allowed when any rule of the union matches it.

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::fqdn;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HttpRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub header_matches: Vec<HeaderMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeaderMatch {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// `LOG`: a mismatch is logged, the request still matches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mismatch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KafkaRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
}

/// One `toPorts` entry's L7 section.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct L7Rules {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub http: Vec<HttpRule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kafka: Vec<KafkaRule>,
    /// Normalized `matchName` / `matchPattern` selectors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dns: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub server_names: Vec<String>,
}

impl L7Rules {
    pub fn is_empty(&self) -> bool {
        self.http.is_empty()
            && self.kafka.is_empty()
            && self.dns.is_empty()
            && self.server_names.is_empty()
    }

    /// `http`, `kafka`, `dns` or `tls`.
    pub fn kind(&self) -> &'static str {
        if !self.server_names.is_empty() {
            "tls"
        } else if !self.kafka.is_empty() {
            "kafka"
        } else if !self.dns.is_empty() {
            "dns"
        } else {
            "http"
        }
    }
}

fn opt_str(v: &Value, k: &str) -> Option<String> {
    v.get(k)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

/// The L7 part of a `toPorts` entry; `None` when it has none.
pub fn from_to_ports(tp: &Value) -> Option<L7Rules> {
    let r = tp.get("rules").filter(|r| !r.is_null());
    let arr = |k: &str| {
        r.and_then(|r| r.get(k))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let mut out = L7Rules::default();
    let http = arr("http");
    let has_http = r.is_some_and(|r| r.get("http").is_some_and(|h| !h.is_null()));
    for h in &http {
        out.http.push(HttpRule {
            method: opt_str(h, "method"),
            path: opt_str(h, "path"),
            host: opt_str(h, "host"),
            headers: h
                .get("headers")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect(),
            header_matches: h
                .get("headerMatches")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|m| {
                    Some(HeaderMatch {
                        name: m.get("name")?.as_str()?.to_string(),
                        value: opt_str(m, "value"),
                        mismatch: opt_str(m, "mismatch"),
                    })
                })
                .collect(),
        });
    }
    if has_http && out.http.is_empty() {
        out.http.push(HttpRule::default());
    }
    for k in arr("kafka") {
        out.kafka.push(KafkaRule {
            role: opt_str(&k, "role").map(|s| s.to_ascii_lowercase()),
            api_key: opt_str(&k, "apiKey").map(|s| s.to_ascii_lowercase()),
            api_version: opt_str(&k, "apiVersion"),
            client_id: opt_str(&k, "clientID"),
            topic: opt_str(&k, "topic"),
        });
    }
    out.dns = fqdn::selectors(&arr("dns"));
    out.server_names = tp
        .get("serverNames")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(fqdn::normalize)
        .filter(|s| !s.is_empty())
        .collect();
    (!out.is_empty()).then_some(out)
}

// ---- Kafka API keys ------------------------------------------------------------

pub const KAFKA_API_KEYS: [&str; 34] = [
    "produce",
    "fetch",
    "offsets",
    "metadata",
    "leaderandisr",
    "stopreplica",
    "updatemetadata",
    "controlledshutdown",
    "offsetcommit",
    "offsetfetch",
    "findcoordinator",
    "joingroup",
    "heartbeat",
    "leavegroup",
    "syncgroup",
    "describegroups",
    "listgroups",
    "saslhandshake",
    "apiversions",
    "createtopics",
    "deletetopics",
    "deleterecords",
    "initproducerid",
    "offsetforleaderepoch",
    "addpartitionstotxn",
    "addoffsetstotxn",
    "endtxn",
    "writetxnmarkers",
    "txnoffsetcommit",
    "describeacls",
    "createacls",
    "deleteacls",
    "describeconfigs",
    "alterconfigs",
];
const KAFKA_PRODUCE_KEYS: [i16; 3] = [0, 3, 18];
const KAFKA_CONSUME_KEYS: [i16; 11] = [1, 2, 3, 8, 9, 10, 11, 12, 13, 14, 18];

pub fn kafka_api_key(name: &str) -> Option<i16> {
    if let Ok(n) = name.parse::<i16>() {
        return Some(n);
    }
    KAFKA_API_KEYS
        .iter()
        .position(|k| *k == name.to_ascii_lowercase())
        .map(|i| i as i16)
}

// ---- Validation ------------------------------------------------------------------

/// Problems with one `toPorts` L7 section: (relative path, message).
pub fn validate(tp: &Value, egress: bool) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut err = |p: String, m: &str| out.push((p, m.to_string()));
    if let Some(r) = tp.get("rules").filter(|r| !r.is_null()) {
        let Some(m) = r.as_object() else {
            err("rules".into(), "expected a mapping");
            return out;
        };
        let kinds: Vec<&String> = m.keys().filter(|k| !m[k.as_str()].is_null()).collect();
        if kinds.len() > 1 {
            err(
                "rules".into(),
                "use one of http, kafka or dns per toPorts entry",
            );
        }
        for k in m.keys() {
            match k.as_str() {
                "http" | "kafka" | "dns" => {}
                "l7proto" | "l7" => err(
                    format!("rules.{k}"),
                    "generic L7 parsers are not supported; use http, kafka or dns",
                ),
                _ => err(format!("rules.{k}"), "unknown field"),
            }
        }
        for (i, h) in r
            .get("http")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            let p = format!("rules.http[{i}]");
            let Some(o) = h.as_object() else {
                err(p, "expected a mapping");
                continue;
            };
            for (k, x) in o {
                match k.as_str() {
                    "method" | "path" | "host" => match x.as_str() {
                        Some(s) if Regex::new(&format!("^(?:{s})$")).is_err() => {
                            err(format!("{p}.{k}"), "invalid regular expression")
                        }
                        Some(_) => {}
                        None => err(format!("{p}.{k}"), "expected a string"),
                    },
                    "headers" => {
                        if !x.as_array().is_some_and(|a| a.iter().all(Value::is_string)) {
                            err(
                                format!("{p}.headers"),
                                "expected a list of `Name` or `Name: value`",
                            );
                        }
                    }
                    "headerMatches" => {
                        for (j, hm) in x.as_array().into_iter().flatten().enumerate() {
                            if hm
                                .get("name")
                                .and_then(Value::as_str)
                                .is_none_or(str::is_empty)
                            {
                                err(format!("{p}.headerMatches[{j}].name"), "required");
                            }
                            if hm.get("secret").is_some() {
                                err(
                                    format!("{p}.headerMatches[{j}].secret"),
                                    "secret references are not supported; use value",
                                );
                            }
                            match hm.get("mismatch").and_then(Value::as_str) {
                                None | Some("LOG") => {}
                                Some("ADD" | "DELETE" | "REPLACE") => err(
                                    format!("{p}.headerMatches[{j}].mismatch"),
                                    "header rewriting needs a terminating proxy; only LOG is supported",
                                ),
                                Some(_) => err(format!("{p}.headerMatches[{j}].mismatch"), "expected LOG, ADD, DELETE or REPLACE"),
                            }
                        }
                    }
                    _ => err(format!("{p}.{k}"), "unknown field"),
                }
            }
        }
        for (i, k) in r
            .get("kafka")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            let p = format!("rules.kafka[{i}]");
            let Some(o) = k.as_object() else {
                err(p, "expected a mapping");
                continue;
            };
            for key in o.keys() {
                if !matches!(
                    key.as_str(),
                    "role" | "apiKey" | "apiVersion" | "clientID" | "topic"
                ) {
                    err(format!("{p}.{key}"), "unknown field");
                }
            }
            if o.contains_key("role") && o.contains_key("apiKey") {
                err(p.clone(), "role and apiKey are mutually exclusive");
            }
            if let Some(role) = k.get("role").and_then(Value::as_str) {
                if !matches!(role.to_ascii_lowercase().as_str(), "produce" | "consume") {
                    err(format!("{p}.role"), "expected produce or consume");
                }
            }
            if let Some(key) = k.get("apiKey").and_then(Value::as_str) {
                if kafka_api_key(key).is_none() {
                    err(format!("{p}.apiKey"), "unknown Kafka API key");
                }
            }
            if let Some(v) = k.get("apiVersion").and_then(Value::as_str) {
                if v.parse::<i16>().is_err() {
                    err(format!("{p}.apiVersion"), "expected a number");
                }
            }
            if let Some(t) = k.get("topic").and_then(Value::as_str) {
                if t.is_empty()
                    || t.len() > 255
                    || !t
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                {
                    err(
                        format!("{p}.topic"),
                        "Kafka topic names use letters, digits, `.`, `_` and `-` (max 255)",
                    );
                }
            }
        }
        for (i, d) in r
            .get("dns")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            let p = format!("rules.dns[{i}]");
            if !egress {
                err(p.clone(), "DNS rules apply to egress only");
            }
            match (
                d.get("matchName").and_then(Value::as_str),
                d.get("matchPattern").and_then(Value::as_str),
            ) {
                (Some(n), None) => {
                    if let Some(why) = fqdn::invalid(n, false) {
                        err(format!("{p}.matchName"), why);
                    }
                }
                (None, Some(n)) => {
                    if let Some(why) = fqdn::invalid(n, true) {
                        err(format!("{p}.matchPattern"), why);
                    }
                }
                _ => err(p, "set exactly one of matchName / matchPattern"),
            }
        }
    }
    for (i, s) in tp
        .get("serverNames")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        match s.as_str() {
            Some(n) => {
                if let Some(why) = fqdn::invalid(n, true) {
                    err(format!("serverNames[{i}]"), why);
                }
            }
            None => err(format!("serverNames[{i}]"), "expected a string"),
        }
    }
    out
}

// ---- Requests ----------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub host: String,
    /// (lower-case name, value)
    #[serde(default)]
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KafkaRequest {
    pub api_key: i16,
    pub api_version: i16,
    #[serde(default)]
    pub client_id: String,
    /// `None` when the request carries topics this parser cannot read.
    #[serde(default)]
    pub topics: Option<Vec<String>>,
}

/// One parsed request (or TLS ClientHello / DNS query).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Request {
    Http(HttpRequest),
    Kafka(KafkaRequest),
    Tls { server_name: String },
    Dns { name: String },
}

impl Request {
    /// `GET example.com/api`, `kafka produce topic=orders`, ...
    pub fn summary(&self) -> String {
        match self {
            Request::Http(h) => format!("{} {}{}", h.method, h.host, h.path),
            Request::Kafka(k) => {
                let key = KAFKA_API_KEYS
                    .get(k.api_key as usize)
                    .copied()
                    .unwrap_or("unknown");
                match &k.topics {
                    Some(t) if !t.is_empty() => {
                        format!("kafka {key} v{} topic={}", k.api_version, t.join(","))
                    }
                    _ => format!("kafka {key} v{}", k.api_version),
                }
            }
            Request::Tls { server_name } => format!("tls sni={server_name}"),
            Request::Dns { name } => format!("dns {name}"),
        }
    }
}

/// HTTP/1.x methods a request start begins with.
pub const HTTP_METHODS: [&str; 9] = [
    "GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS", "PATCH", "CONNECT", "TRACE",
];

/// One HTTP/1.x request head at the start of `b`: (request, bytes the
/// request occupies, or `None` for a chunked body that runs to the end of
/// the connection).
pub fn parse_http(b: &[u8]) -> Result<(HttpRequest, Option<usize>), &'static str> {
    let end = b
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("request head incomplete in this segment")?;
    let head = std::str::from_utf8(&b[..end]).map_err(|_| "request head is not UTF-8")?;
    let mut lines = head.split("\r\n");
    let line = lines.next().ok_or("empty request")?;
    let mut parts = line.split(' ');
    let (method, target, version) = (
        parts.next().unwrap_or(""),
        parts.next().unwrap_or(""),
        parts.next().unwrap_or(""),
    );
    if !HTTP_METHODS.contains(&method)
        || target.is_empty()
        || !version.starts_with("HTTP/1.")
        || parts.next().is_some()
    {
        return Err("not an HTTP/1.x request");
    }
    let mut req = HttpRequest {
        method: method.into(),
        path: target.into(),
        ..Default::default()
    };
    let mut len: usize = 0;
    let mut chunked = false;
    for l in lines {
        let (n, v) = l.split_once(':').ok_or("malformed header")?;
        let (n, v) = (n.trim().to_ascii_lowercase(), v.trim().to_string());
        match n.as_str() {
            "host" => req.host = v.clone(),
            "content-length" => len = v.parse().map_err(|_| "bad Content-Length")?,
            "transfer-encoding" if v.to_ascii_lowercase().contains("chunked") => chunked = true,
            _ => {}
        }
        req.headers.push((n, v));
    }
    if let Some(rest) = req
        .path
        .strip_prefix("http://")
        .or_else(|| req.path.strip_prefix("https://"))
    {
        let (h, p) = rest
            .split_once('/')
            .map_or((rest, "/".to_string()), |(h, p)| (h, format!("/{p}")));
        if req.host.is_empty() {
            req.host = h.into();
        }
        req.path = p;
    }
    Ok((req, (!chunked).then_some(end + 4 + len)))
}

struct Rd<'a> {
    b: &'a [u8],
    i: usize,
}

impl Rd<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.b.get(self.i..self.i.checked_add(n)?)?;
        self.i += n;
        Some(s)
    }
    fn i8(&mut self) -> Option<i8> {
        self.take(1).map(|s| s[0] as i8)
    }
    fn i16(&mut self) -> Option<i16> {
        self.take(2).map(|s| i16::from_be_bytes([s[0], s[1]]))
    }
    fn i32(&mut self) -> Option<i32> {
        self.take(4)
            .map(|s| i32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn skip(&mut self, n: usize) -> Option<()> {
        self.take(n).map(|_| ())
    }
    /// Nullable string (i16 length, -1 = null).
    fn string(&mut self) -> Option<String> {
        let n = self.i16()?;
        if n < 0 {
            return Some(String::new());
        }
        self.take(n as usize)
            .map(|s| String::from_utf8_lossy(s).into_owned())
    }
    fn bytes(&mut self) -> Option<()> {
        let n = self.i32()?;
        if n > 0 {
            self.skip(n as usize)?;
        }
        Some(())
    }
    fn count(&mut self) -> Option<usize> {
        let n = self.i32()?;
        if n < 0 {
            return Some(0);
        }
        (n <= 100_000).then_some(n as usize)
    }
}

/// Topic names of the request body for the API keys / versions read here.
fn kafka_topics(key: i16, ver: i16, r: &mut Rd) -> Option<Vec<String>> {
    let mut topics = Vec::new();
    let mut each = |r: &mut Rd, part: &mut dyn FnMut(&mut Rd) -> Option<()>| -> Option<()> {
        for _ in 0..r.count()? {
            topics.push(r.string()?);
            for _ in 0..r.count()? {
                part(r)?;
            }
        }
        Some(())
    };
    match (key, ver) {
        (0, 0..=8) => {
            if ver >= 3 {
                r.string()?;
            }
            r.skip(6)?;
            each(r, &mut |r| {
                r.skip(4)?;
                r.bytes()
            })?;
        }
        (1, 0..=11) => {
            r.skip(12)?;
            if ver >= 3 {
                r.skip(4)?;
            }
            if ver >= 4 {
                r.skip(1)?;
            }
            if ver >= 7 {
                r.skip(8)?;
            }
            let n = match ver {
                0..=4 => 16,
                5..=8 => 24,
                _ => 28,
            };
            each(r, &mut |r| r.skip(n))?;
        }
        (2, 0..=5) => {
            r.skip(4)?;
            if ver >= 2 {
                r.skip(1)?;
            }
            let n = match ver {
                0 => 16,
                1..=3 => 12,
                _ => 16,
            };
            each(r, &mut |r| r.skip(n))?;
        }
        (3, 0..=8) => {
            let n = r.i32()?;
            if n < 0 {
                return None;
            }
            for _ in 0..n.min(100_000) {
                topics.push(r.string()?);
            }
        }
        (8, 0..=7) => {
            r.string()?;
            if ver >= 1 {
                r.skip(4)?;
                r.string()?;
            }
            if ver >= 7 {
                r.string()?;
            }
            if (2..=4).contains(&ver) {
                r.skip(8)?;
            }
            each(r, &mut |r| {
                r.skip(12)?;
                if ver == 1 {
                    r.skip(8)?;
                }
                if ver >= 6 {
                    r.skip(4)?;
                }
                r.string().map(|_| ())
            })?;
        }
        (9, 0..=5) => {
            r.string()?;
            each(r, &mut |r| r.skip(4))?;
        }
        _ => return None,
    }
    Some(topics)
}

/// Kafka requests at the start of `b`: each with its total size (length
/// prefix included). Stops at a request that runs past the segment.
pub fn parse_kafka(b: &[u8]) -> Result<Vec<(KafkaRequest, usize)>, &'static str> {
    let mut out = Vec::new();
    let mut off = 0;
    while off < b.len() {
        let rest = &b[off..];
        if rest.len() < 14 {
            return if out.is_empty() {
                Err("Kafka request header incomplete")
            } else {
                Ok(out)
            };
        }
        let size = i32::from_be_bytes([rest[0], rest[1], rest[2], rest[3]]);
        if !(10..=100 << 20).contains(&size) {
            return Err("not a Kafka request");
        }
        let total = size as usize + 4;
        let mut r = Rd {
            b: &rest[4..rest.len().min(total)],
            i: 0,
        };
        let api_key = r.i16().ok_or("short")?;
        let api_version = r.i16().ok_or("short")?;
        if !(0..=100).contains(&api_key) || !(0..=100).contains(&api_version) {
            return Err("not a Kafka request");
        }
        r.skip(4).ok_or("short")?;
        let client_id = r.string().ok_or("Kafka client id incomplete")?;
        let topics = kafka_topics(api_key, api_version, &mut r);
        out.push((
            KafkaRequest {
                api_key,
                api_version,
                client_id,
                topics,
            },
            total,
        ));
        off += total;
    }
    Ok(out)
}

/// SNI of a TLS ClientHello at the start of `b`.
pub fn parse_sni(b: &[u8]) -> Result<String, &'static str> {
    let mut r = Rd { b, i: 0 };
    let ct = r.i8().ok_or("short")?;
    if ct != 22 {
        return Err("not a TLS handshake");
    }
    r.skip(4).ok_or("short")?;
    if r.i8().ok_or("short")? != 1 {
        return Err("not a ClientHello");
    }
    r.skip(3 + 2 + 32)
        .ok_or("ClientHello incomplete in this segment")?;
    let sid = r.i8().ok_or("short")? as u8;
    r.skip(sid as usize).ok_or("short")?;
    let cs = r.i16().ok_or("short")? as u16;
    r.skip(cs as usize).ok_or("short")?;
    let cm = r.i8().ok_or("short")? as u8;
    r.skip(cm as usize).ok_or("short")?;
    let ext_len = r.i16().ok_or("no extensions")? as u16 as usize;
    let end = (r.i + ext_len).min(b.len());
    while r.i + 4 <= end {
        let ty = r.i16().ok_or("short")? as u16;
        let len = r.i16().ok_or("short")? as u16 as usize;
        if ty == 0 {
            let mut s = Rd {
                b: r.take(len).ok_or("SNI incomplete")?,
                i: 0,
            };
            s.skip(2).ok_or("short")?;
            while s.i < s.b.len() {
                let nt = s.i8().ok_or("short")?;
                let n = s.i16().ok_or("short")? as u16 as usize;
                let name = s.take(n).ok_or("short")?;
                if nt == 0 {
                    return Ok(String::from_utf8_lossy(name).to_ascii_lowercase());
                }
            }
            return Err("no host name in SNI");
        }
        r.skip(len).ok_or("short")?;
    }
    Err("ClientHello has no SNI")
}

// ---- Matching -------------------------------------------------------------------------

/// Rules with their regular expressions compiled, ready to match.
#[derive(Debug, Default)]
pub struct Matcher {
    http: Vec<(HttpRule, [Option<Regex>; 3])>,
    kafka: Vec<KafkaRule>,
    dns: Vec<String>,
    server_names: Vec<String>,
}

fn anchored(s: &Option<String>) -> Option<Regex> {
    s.as_ref()
        .and_then(|s| Regex::new(&format!("^(?:{s})$")).ok())
}

impl Matcher {
    pub fn new<'a>(rules: impl IntoIterator<Item = &'a L7Rules>) -> Self {
        let mut m = Matcher::default();
        for r in rules {
            for h in &r.http {
                m.http.push((
                    h.clone(),
                    [anchored(&h.method), anchored(&h.path), anchored(&h.host)],
                ));
            }
            m.kafka.extend(r.kafka.iter().cloned());
            m.dns.extend(r.dns.iter().cloned());
            m.server_names.extend(r.server_names.iter().cloned());
        }
        m
    }

    /// Allowed?, plus a note (`header X mismatch logged`, why denied).
    pub fn check(&self, req: &Request) -> (bool, Option<String>) {
        match req {
            Request::Http(h) => {
                let mut note = None;
                for (rule, [m, p, host]) in &self.http {
                    let re_ok = |re: &Option<Regex>, src: &Option<String>, v: &str| match (re, src)
                    {
                        (Some(re), _) => re.is_match(v),
                        (None, Some(_)) => false,
                        (None, None) => true,
                    };
                    if !re_ok(m, &rule.method, &h.method) || !re_ok(p, &rule.path, &h.path) {
                        continue;
                    }
                    let host_only = h
                        .host
                        .rsplit_once(':')
                        .filter(|(_, port)| port.bytes().all(|c| c.is_ascii_digit()))
                        .map_or(h.host.as_str(), |(a, _)| a);
                    if !(re_ok(host, &rule.host, &h.host) || re_ok(host, &rule.host, host_only)) {
                        continue;
                    }
                    let has = |n: &str, v: Option<&str>| {
                        let n = n.trim().to_ascii_lowercase();
                        h.headers
                            .iter()
                            .any(|(hn, hv)| *hn == n && v.is_none_or(|v| hv == v))
                    };
                    let headers_ok = rule.headers.iter().all(|e| match e.split_once(':') {
                        Some((n, v)) => has(n, Some(v.trim())),
                        None => has(e, None),
                    });
                    if !headers_ok {
                        continue;
                    }
                    let mut ok = true;
                    for hm in &rule.header_matches {
                        if !has(&hm.name, hm.value.as_deref()) {
                            if hm.mismatch.as_deref() == Some("LOG") {
                                note = Some(format!("header {} mismatch logged", hm.name));
                            } else {
                                ok = false;
                            }
                        }
                    }
                    if ok {
                        return (true, note);
                    }
                }
                (false, Some("no http rule matches".into()))
            }
            Request::Kafka(k) => {
                let cands: Vec<&KafkaRule> = self
                    .kafka
                    .iter()
                    .filter(|r| match (&r.role, &r.api_key) {
                        (Some(role), _) => {
                            let keys: &[i16] = if role == "produce" {
                                &KAFKA_PRODUCE_KEYS
                            } else {
                                &KAFKA_CONSUME_KEYS
                            };
                            keys.contains(&k.api_key)
                        }
                        (None, Some(key)) => kafka_api_key(key) == Some(k.api_key),
                        (None, None) => true,
                    })
                    .filter(|r| {
                        r.api_version
                            .as_deref()
                            .is_none_or(|v| v.parse::<i16>().ok() == Some(k.api_version))
                    })
                    .filter(|r| r.client_id.as_deref().is_none_or(|c| c == k.client_id))
                    .collect();
                if cands.iter().any(|r| r.topic.is_none()) {
                    return (true, None);
                }
                if cands.is_empty() {
                    return (
                        false,
                        Some("no kafka rule matches the API key / version / client".into()),
                    );
                }
                match &k.topics {
                    Some(t)
                        if !t.is_empty()
                            && t.iter()
                                .all(|t| cands.iter().any(|r| r.topic.as_deref() == Some(t))) =>
                    {
                        (true, None)
                    }
                    Some(t) if !t.is_empty() => {
                        (false, Some(format!("topic not allowed: {}", t.join(","))))
                    }
                    _ => (
                        false,
                        Some("request topics unknown; topic rules cannot allow it".into()),
                    ),
                }
            }
            Request::Tls { server_name } => {
                if self
                    .server_names
                    .iter()
                    .any(|p| fqdn::matches(p, server_name))
                {
                    (true, None)
                } else {
                    (
                        false,
                        Some(format!("server name {server_name} not allowed")),
                    )
                }
            }
            Request::Dns { name } => {
                if self.dns.iter().any(|p| fqdn::matches(p, name)) {
                    (true, None)
                } else {
                    (false, Some(format!("DNS name {name} not allowed")))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn http_rules() {
        let r = from_to_ports(&json!({"ports": [{"port": "80"}], "rules": {"http": [
            {"method": "GET", "path": "/api/v1/.*"},
            {"method": "POST", "path": "/login", "headers": ["X-Token: s3cret"], "host": "app\\.example\\.com"}
        ]}}))
        .unwrap();
        let m = Matcher::new([&r]);
        let req = |s: &str| Request::Http(parse_http(s.as_bytes()).unwrap().0);
        assert!(
            m.check(&req("GET /api/v1/users HTTP/1.1\r\nHost: x\r\n\r\n"))
                .0
        );
        assert!(!m.check(&req("GET /admin HTTP/1.1\r\nHost: x\r\n\r\n")).0);
        assert!(!m.check(&req("DELETE /api/v1/users HTTP/1.1\r\n\r\n")).0);
        assert!(
            m.check(&req(
                "POST /login HTTP/1.1\r\nHost: app.example.com:8080\r\nX-Token: s3cret\r\n\r\n"
            ))
            .0
        );
        assert!(
            !m.check(&req(
                "POST /login HTTP/1.1\r\nHost: app.example.com\r\nX-Token: no\r\n\r\n"
            ))
            .0
        );
        let (_, used) = parse_http(b"POST /x HTTP/1.1\r\nContent-Length: 5\r\n\r\nhello").unwrap();
        assert_eq!(used, Some(44));
        assert_eq!(
            parse_http(b"POST /x HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n")
                .unwrap()
                .1,
            None
        );
        assert!(parse_http(b"GET /x HTTP/1.1\r\nHost: a").is_err());
        assert!(parse_http(b"\x16\x03\x01\x00").is_err());
        let all = Matcher::new([&from_to_ports(&json!({"rules": {"http": [{}]}})).unwrap()]);
        assert!(all.check(&req("PUT /anything HTTP/1.0\r\n\r\n")).0);
    }

    fn kafka_req(key: i16, ver: i16, client: &str, body: &[u8]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend(key.to_be_bytes());
        b.extend(ver.to_be_bytes());
        b.extend(7i32.to_be_bytes());
        b.extend((client.len() as i16).to_be_bytes());
        b.extend(client.as_bytes());
        b.extend(body);
        let mut out = (b.len() as i32).to_be_bytes().to_vec();
        out.extend(b);
        out
    }

    fn kstr(s: &str) -> Vec<u8> {
        let mut v = (s.len() as i16).to_be_bytes().to_vec();
        v.extend(s.as_bytes());
        v
    }

    #[test]
    fn kafka_rules() {
        // Produce v2: acks, timeout, [topic, [partition, records]]
        let mut body = vec![0, 1, 0, 0, 0x75, 0x30];
        body.extend(1i32.to_be_bytes());
        body.extend(kstr("orders"));
        body.extend(1i32.to_be_bytes());
        body.extend(0i32.to_be_bytes());
        body.extend(3i32.to_be_bytes());
        body.extend([1, 2, 3]);
        let produce = kafka_req(0, 2, "svc", &body);
        let mut meta = 1i32.to_be_bytes().to_vec();
        meta.extend(kstr("payments"));
        let metadata = kafka_req(3, 1, "svc", &meta);
        let mut both = produce.clone();
        both.extend(&metadata);
        let reqs = parse_kafka(&both).unwrap();
        assert_eq!(reqs.len(), 2);
        assert_eq!(
            reqs[0].0.topics.as_deref(),
            Some(&["orders".to_string()][..])
        );
        assert_eq!(reqs[0].1, produce.len());
        let r = from_to_ports(&json!({"rules": {"kafka": [{"role": "produce", "topic": "orders"}, {"apiKey": "apiversions"}]}})).unwrap();
        let m = Matcher::new([&r]);
        assert!(m.check(&Request::Kafka(reqs[0].0.clone())).0);
        assert!(
            !m.check(&Request::Kafka(reqs[1].0.clone())).0,
            "metadata for another topic"
        );
        let apiv = parse_kafka(&kafka_req(18, 3, "svc", &[0])).unwrap();
        assert!(m.check(&Request::Kafka(apiv[0].0.clone())).0);
        let fetch = parse_kafka(&kafka_req(1, 4, "svc", &[0; 13])).unwrap();
        assert!(
            !m.check(&Request::Kafka(fetch[0].0.clone())).0,
            "consume not allowed"
        );
    }

    #[test]
    fn sni_and_dns() {
        let name = b"api.example.com";
        let mut ext = vec![0, 0];
        let sni_list: Vec<u8> = [
            vec![0, (name.len() + 3) as u8, 0],
            (name.len() as u16).to_be_bytes().to_vec(),
            name.to_vec(),
        ]
        .concat();
        ext.extend((sni_list.len() as u16).to_be_bytes());
        ext.extend(&sni_list);
        let mut hello = vec![3, 3];
        hello.extend([0u8; 32]);
        hello.extend([0, 0, 2, 0x13, 0x01, 1, 0]);
        hello.extend((ext.len() as u16).to_be_bytes());
        hello.extend(&ext);
        let mut hs = vec![1, 0, (hello.len() >> 8) as u8, hello.len() as u8];
        hs.extend(&hello);
        let mut rec = vec![22, 3, 1];
        rec.extend((hs.len() as u16).to_be_bytes());
        rec.extend(&hs);
        assert_eq!(parse_sni(&rec).unwrap(), "api.example.com");
        let r = from_to_ports(&json!({"serverNames": ["*.example.com"]})).unwrap();
        assert_eq!(r.kind(), "tls");
        assert!(
            Matcher::new([&r])
                .check(&Request::Tls {
                    server_name: "api.example.com".into()
                })
                .0
        );
        let d = from_to_ports(&json!({"rules": {"dns": [{"matchPattern": "*.corp.internal"}]}}))
            .unwrap();
        let m = Matcher::new([&d]);
        assert!(
            m.check(&Request::Dns {
                name: "git.corp.internal".into()
            })
            .0
        );
        assert!(
            !m.check(&Request::Dns {
                name: "evil.com".into()
            })
            .0
        );
    }

    #[test]
    fn validation_paths() {
        let v = validate(
            &json!({"rules": {"http": [{"method": "(", "bogus": 1}], "kafka": [{"role": "x"}]}}),
            false,
        );
        let paths: Vec<&str> = v.iter().map(|(p, _)| p.as_str()).collect();
        assert!(paths.contains(&"rules"), "{paths:?}");
        assert!(paths.contains(&"rules.http[0].method"));
        assert!(paths.contains(&"rules.http[0].bogus"));
        assert!(paths.contains(&"rules.kafka[0].role"));
        let v = validate(&json!({"rules": {"dns": [{"matchName": "a.b"}]}}), false);
        assert_eq!(v[0].0, "rules.dns[0]");
    }
}
