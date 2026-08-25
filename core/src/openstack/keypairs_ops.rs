// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

//! Nova keypair create, import, delete.

use osauth::services::COMPUTE;
use serde::{Deserialize, Serialize};

use crate::config::OpenStackConfig;
use crate::LibvirtError;

use super::auth::{connect_session, map_json_err, map_osauth_err};
use super::resources::OpenStackKeyPair;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateKeypairRequest {
    pub name: String,
    pub public_key: Option<String>,
}

pub async fn create_keypair(
    cfg: &OpenStackConfig,
    req: &CreateKeypairRequest,
) -> Result<OpenStackKeyPair, LibvirtError> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(LibvirtError::Invalid("keypair name is required".into()));
    }
    let session = connect_session(cfg).await?;
    let mut body = serde_json::json!({ "keypair": { "name": name } });
    if let Some(ref pk) = req.public_key {
        let key = pk.trim();
        if !key.is_empty() {
            body["keypair"]["public_key"] = serde_json::json!(key);
        }
    }
    #[derive(Deserialize)]
    struct Resp {
        keypair: KpJson,
    }
    #[derive(Deserialize)]
    struct KpJson {
        name: String,
        fingerprint: Option<String>,
        private_key: Option<String>,
    }
    let resp = session
        .post(COMPUTE, &["os-keypairs"])
        .json(&body)
        .send()
        .await
        .map_err(map_osauth_err)?;
    let parsed: Resp = resp.json().await.map_err(map_json_err)?;
    Ok(OpenStackKeyPair {
        name: parsed.keypair.name,
        fingerprint: parsed.keypair.fingerprint,
        private_key: parsed.keypair.private_key,
    })
}

pub async fn delete_keypair(cfg: &OpenStackConfig, name: &str) -> Result<(), LibvirtError> {
    let n = name.trim();
    if n.is_empty() {
        return Err(LibvirtError::Invalid("keypair name is required".into()));
    }
    let session = connect_session(cfg).await?;
    session
        .delete(COMPUTE, &["os-keypairs", n])
        .send()
        .await
        .map_err(map_osauth_err)?;
    Ok(())
}
