// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! Async client for machina-bpfd (used by machina-daemon, machina-agent, machina-cni).

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc;

use crate::api::{Request, Response, StreamEvent, DEFAULT_SOCKET, SOCKET_ENV};

#[derive(Debug, Clone)]
pub struct BpfdClient {
    path: PathBuf,
    timeout: Duration,
}

impl Default for BpfdClient {
    fn default() -> Self {
        Self::from_env()
    }
}

impl BpfdClient {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            timeout: Duration::from_secs(15),
        }
    }

    pub fn from_env() -> Self {
        Self::new(std::env::var(SOCKET_ENV).unwrap_or_else(|_| DEFAULT_SOCKET.into()))
    }

    pub fn socket_path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn reachable(&self) -> bool {
        self.path.exists()
    }

    async fn connect(&self) -> Result<UnixStream> {
        tokio::time::timeout(Duration::from_secs(3), UnixStream::connect(&self.path))
            .await
            .map_err(|_| anyhow!("timed out connecting to machina-bpfd at {}", self.path.display()))?
            .with_context(|| {
                format!(
                    "machina-bpfd not reachable at {} (is machina-bpfd.service running?)",
                    self.path.display()
                )
            })
    }

    /// Send one request; returns `data` on success or the daemon's error.
    pub async fn call(&self, req: &Request) -> Result<Value> {
        let fut = async {
            let mut s = self.connect().await?;
            let mut line = serde_json::to_vec(req)?;
            line.push(b'\n');
            s.write_all(&line).await?;
            let mut reader = BufReader::new(s);
            let mut buf = String::new();
            reader.read_line(&mut buf).await?;
            if buf.is_empty() {
                return Err(anyhow!("machina-bpfd closed the connection"));
            }
            let resp: Response = serde_json::from_str(&buf).context("invalid machina-bpfd response")?;
            if resp.ok {
                Ok(resp.data)
            } else {
                Err(anyhow!(resp.error.unwrap_or_else(|| "machina-bpfd error".into())))
            }
        };
        tokio::time::timeout(self.timeout, fut)
            .await
            .map_err(|_| anyhow!("machina-bpfd request timed out"))?
    }

    pub async fn call_as<T: DeserializeOwned>(&self, req: &Request) -> Result<T> {
        Ok(serde_json::from_value(self.call(req).await?)?)
    }

    /// Subscribe to event topics; events arrive on the returned channel until
    /// it is dropped or the daemon disconnects.
    pub async fn subscribe(&self, topics: &[&str]) -> Result<mpsc::Receiver<StreamEvent>> {
        let mut s = self.connect().await?;
        let req = Request::Subscribe {
            topics: topics.iter().map(|t| t.to_string()).collect(),
        };
        let mut line = serde_json::to_vec(&req)?;
        line.push(b'\n');
        s.write_all(&line).await?;
        let (tx, rx) = mpsc::channel(1024);
        tokio::spawn(async move {
            let mut lines = BufReader::new(s).lines();
            // First line is the subscribe acknowledgement.
            let _ = lines.next_line().await;
            while let Ok(Some(l)) = lines.next_line().await {
                let Ok(ev) = serde_json::from_str::<StreamEvent>(&l) else {
                    continue;
                };
                if tx.send(ev).await.is_err() {
                    break;
                }
            }
        });
        Ok(rx)
    }
}
