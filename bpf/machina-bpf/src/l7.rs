// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! First-payload L7 classification: TLS ClientHello (SNI, ALPN) and HTTP/1.x
//! request heads. Pure parsing of a possibly truncated buffer, never panics.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tls {
    pub sni: Option<String>,
    pub alpn: Vec<String>,
    /// Legacy record version from the ClientHello, e.g. "1.2".
    pub version: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Http {
    pub method: String,
    pub path: String,
    pub version: String,
    pub host: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum L7 {
    Tls(Tls),
    Http(Http),
    Ssh { banner: String },
}

const HTTP_METHODS: &[&str] = &[
    "GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS", "PATCH", "CONNECT", "TRACE",
];

pub fn classify(b: &[u8]) -> Option<L7> {
    if let Some(t) = parse_client_hello(b) {
        return Some(L7::Tls(t));
    }
    if let Some(h) = parse_http(b) {
        return Some(L7::Http(h));
    }
    if b.starts_with(b"SSH-") {
        let end = b.iter().position(|c| *c == b'\r' || *c == b'\n').unwrap_or(b.len()).min(128);
        return Some(L7::Ssh {
            banner: String::from_utf8_lossy(&b[..end]).into_owned(),
        });
    }
    None
}

struct Rd<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Rd<'a> {
    fn u8(&mut self) -> Option<u8> {
        let v = *self.b.get(self.p)?;
        self.p += 1;
        Some(v)
    }
    fn u16(&mut self) -> Option<u16> {
        Some(((self.u8()? as u16) << 8) | self.u8()? as u16)
    }
    fn u24(&mut self) -> Option<usize> {
        Some(((self.u8()? as usize) << 16) | ((self.u8()? as usize) << 8) | self.u8()? as usize)
    }
    fn skip(&mut self, n: usize) -> Option<()> {
        (self.p + n <= self.b.len()).then(|| self.p += n)
    }
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.b.get(self.p..self.p + n)?;
        self.p += n;
        Some(s)
    }
}

fn tls_version(major: u8, minor: u8) -> String {
    match (major, minor) {
        (3, 0) => "ssl3".into(),
        (3, n) => format!("1.{}", n - 1),
        _ => format!("{major}.{minor}"),
    }
}

/// Extensions may be cut off by the snap length: whatever was readable is kept.
pub fn parse_client_hello(b: &[u8]) -> Option<Tls> {
    let mut r = Rd { b, p: 0 };
    if r.u8()? != 0x16 {
        return None;
    }
    if r.u8()? != 3 {
        return None;
    }
    r.skip(3)?; // record minor version + length
    if r.u8()? != 0x01 {
        return None;
    }
    r.u24()?;
    let (hmaj, hmin) = (r.u8()?, r.u8()?);
    let mut out = Tls {
        version: tls_version(hmaj, hmin),
        ..Tls::default()
    };
    r.skip(32)?; // random
    let sid = r.u8()? as usize;
    r.skip(sid)?;
    let cs = r.u16()? as usize;
    r.skip(cs)?;
    let comp = r.u8()? as usize;
    r.skip(comp)?;
    let Some(ext_len) = r.u16() else {
        return Some(out);
    };
    let end = (r.p + ext_len as usize).min(b.len());
    while r.p + 4 <= end {
        let ty = r.u16()?;
        let len = r.u16()? as usize;
        let Some(body) = r.take(len) else {
            break;
        };
        let mut e = Rd { b: body, p: 0 };
        match ty {
            0x0000 => {
                // server_name list → first host_name entry
                let _list = e.u16();
                while let (Some(kind), Some(n)) = (e.u8(), e.u16()) {
                    let Some(name) = e.take(n as usize) else { break };
                    if kind == 0 {
                        if let Ok(s) = std::str::from_utf8(name) {
                            if !s.is_empty() && s.bytes().all(|c| c.is_ascii_graphic()) {
                                out.sni = Some(s.to_ascii_lowercase());
                            }
                        }
                        break;
                    }
                }
            }
            0x0010 => {
                let _list = e.u16();
                while let Some(n) = e.u8() {
                    let Some(p) = e.take(n as usize) else { break };
                    out.alpn.push(String::from_utf8_lossy(p).into_owned());
                }
            }
            0x002b => {
                // supported_versions: highest listed (ignoring GREASE) wins
                if let Some(n) = e.u8() {
                    let mut best: Option<(u8, u8)> = None;
                    for _ in 0..n / 2 {
                        let (Some(a), Some(b)) = (e.u8(), e.u8()) else { break };
                        if a & 0x0f == 0x0a && b & 0x0f == 0x0a {
                            continue;
                        }
                        if best.is_none_or(|x| (a, b) > x) {
                            best = Some((a, b));
                        }
                    }
                    if let Some((a, b)) = best {
                        out.version = tls_version(a, b);
                    }
                }
            }
            _ => {}
        }
    }
    Some(out)
}

pub fn parse_http(b: &[u8]) -> Option<Http> {
    let sp = b.iter().take(8).position(|c| *c == b' ')?;
    let method = std::str::from_utf8(&b[..sp]).ok()?;
    if !HTTP_METHODS.contains(&method) {
        return None;
    }
    let line_end = b.iter().position(|c| *c == b'\n').unwrap_or(b.len());
    let line = String::from_utf8_lossy(&b[..line_end]);
    let mut parts = line.trim_end_matches('\r').split(' ');
    parts.next();
    let path = parts.next()?.chars().take(512).collect::<String>();
    let version = parts.next().unwrap_or("").to_string();
    if !version.is_empty() && !version.starts_with("HTTP/") {
        return None;
    }
    let mut out = Http {
        method: method.to_string(),
        path,
        version,
        ..Http::default()
    };
    let rest = b.get(line_end + 1..).unwrap_or(&[]);
    for raw in rest.split(|c| *c == b'\n') {
        let l = String::from_utf8_lossy(raw);
        let l = l.trim_end_matches('\r');
        if l.is_empty() {
            break;
        }
        let Some((k, v)) = l.split_once(':') else { continue };
        let v = v.trim().chars().take(256).collect::<String>();
        match k.trim().to_ascii_lowercase().as_str() {
            "host" => out.host = Some(v),
            "user-agent" => out.user_agent = Some(v),
            _ => {}
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ext(ty: u16, body: &[u8]) -> Vec<u8> {
        let mut v = ty.to_be_bytes().to_vec();
        v.extend((body.len() as u16).to_be_bytes());
        v.extend(body);
        v
    }

    fn client_hello(sni: &str) -> Vec<u8> {
        let mut sn = Vec::new();
        sn.extend(((sni.len() + 3) as u16).to_be_bytes());
        sn.push(0);
        sn.extend((sni.len() as u16).to_be_bytes());
        sn.extend(sni.as_bytes());
        let mut alpn = Vec::new();
        alpn.extend(12u16.to_be_bytes());
        alpn.push(2);
        alpn.extend(b"h2");
        alpn.push(8);
        alpn.extend(b"http/1.1");
        let versions = [4u8, 0x0a, 0x0a, 3, 4];
        let mut exts = ext(0x0000, &sn);
        exts.extend(ext(0x0010, &alpn));
        exts.extend(ext(0x002b, &versions));

        let mut hs = vec![3, 3];
        hs.extend([0u8; 32]);
        hs.push(0); // session id
        hs.extend(4u16.to_be_bytes());
        hs.extend([0x13, 0x01, 0x13, 0x02]);
        hs.extend([1, 0]);
        hs.extend((exts.len() as u16).to_be_bytes());
        hs.extend(exts);

        let mut msg = vec![1];
        msg.extend(&(hs.len() as u32).to_be_bytes()[1..]);
        msg.extend(hs);
        let mut rec = vec![0x16, 3, 1];
        rec.extend((msg.len() as u16).to_be_bytes());
        rec.extend(msg);
        rec
    }

    #[test]
    fn tls_sni_alpn_version() {
        let t = parse_client_hello(&client_hello("API.Example.com")).unwrap();
        assert_eq!(t.sni.as_deref(), Some("api.example.com"));
        assert_eq!(t.alpn, vec!["h2", "http/1.1"]);
        assert_eq!(t.version, "1.3");
    }

    #[test]
    fn tls_truncated_never_panics() {
        let full = client_hello("example.org");
        for n in 0..full.len() {
            let _ = classify(&full[..n]);
        }
        assert!(parse_client_hello(&full[..60]).is_some_and(|t| t.sni.is_none()));
    }

    #[test]
    fn http_request() {
        let h = parse_http(b"GET /index.html?q=1 HTTP/1.1\r\nHost: Example.com\r\nUser-Agent: curl/8\r\n\r\n").unwrap();
        assert_eq!(h.method, "GET");
        assert_eq!(h.path, "/index.html?q=1");
        assert_eq!(h.version, "HTTP/1.1");
        assert_eq!(h.host.as_deref(), Some("Example.com"));
        assert_eq!(h.user_agent.as_deref(), Some("curl/8"));
        assert!(parse_http(b"HELLO world").is_none());
        assert!(parse_http(b"GET / FOO/1").is_none());
    }

    #[test]
    fn ssh_banner() {
        assert_eq!(
            classify(b"SSH-2.0-OpenSSH_9.6\r\n"),
            Some(L7::Ssh { banner: "SSH-2.0-OpenSSH_9.6".into() })
        );
        assert_eq!(classify(b"\x00\x01binary"), None);
    }
}
