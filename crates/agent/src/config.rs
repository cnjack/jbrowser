use std::{env, path::PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub(crate) struct AgentConfig {
    pub control_http_url: String,
    pub control_ws_url: String,
    pub registration_token: String,
    pub data_dir: PathBuf,
    pub agent_name: String,
    pub browser_type: String,
    pub browser_version: String,
}

impl AgentConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let control_http_url = env::var("CONTROL_PLANE_HTTP_URL")
            .unwrap_or_else(|_| "http://localhost:8080".to_string());
        let control_ws_url = env::var("CONTROL_PLANE_WS_URL").unwrap_or_else(|_| {
            control_http_url
                .replace("https://", "wss://")
                .replace("http://", "ws://")
        });
        Ok(Self {
            control_http_url,
            control_ws_url,
            registration_token: env::var("REGISTRATION_TOKEN")
                .context("missing REGISTRATION_TOKEN")?,
            data_dir: env::var("AGENT_DATA_DIR")
                .unwrap_or_else(|_| "/var/lib/jbrowser-agent".to_string())
                .into(),
            agent_name: env::var("AGENT_NAME").unwrap_or_else(|_| "chromium-agent".to_string()),
            browser_type: env::var("BROWSER_TYPE").unwrap_or_else(|_| "chromium".to_string()),
            browser_version: env::var("BROWSER_VERSION").unwrap_or_else(|_| "unknown".to_string()),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentIdentity {
    pub agent_id: String,
    pub browser_instance_id: String,
    pub agent_runtime_token: String,
}

pub(crate) async fn load_or_register(config: &AgentConfig) -> anyhow::Result<AgentIdentity> {
    let path = config.data_dir.join("identity.json");
    if let Ok(raw) = fs::read_to_string(&path).await {
        if let Ok(existing) = serde_json::from_str::<AgentIdentity>(&raw) {
            let client = reqwest::Client::new();
            match client
                .post(format!(
                    "{}/api/v1/agents/register",
                    config.control_http_url
                ))
                .json(&serde_json::json!({
                    "registration_token": config.registration_token,
                    "name": config.agent_name,
                    "browser_type": config.browser_type,
                    "browser_version": config.browser_version,
                    "agent_id": existing.agent_id,
                    "browser_instance_id": existing.browser_instance_id,
                }))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    let refreshed = resp.json::<AgentIdentity>().await?;
                    info!(agent_id = %refreshed.agent_id, "re-registered with stable identity");
                    return Ok(refreshed);
                }
                Ok(resp) => {
                    warn!(status = %resp.status(), "re-registration failed, will try fresh registration");
                }
                Err(e) => {
                    warn!(err = %e, "re-registration request failed, will retry as new agent");
                }
            }
        }
    }
    let client = reqwest::Client::new();
    let identity = client
        .post(format!(
            "{}/api/v1/agents/register",
            config.control_http_url
        ))
        .json(&serde_json::json!({
            "registration_token": config.registration_token,
            "name": config.agent_name,
            "browser_type": config.browser_type,
            "browser_version": config.browser_version
        }))
        .send()
        .await?
        .error_for_status()?
        .json::<AgentIdentity>()
        .await?;
    Ok(identity)
}

pub(crate) async fn save_identity(
    config: &AgentConfig,
    identity: &AgentIdentity,
) -> anyhow::Result<()> {
    let path = config.data_dir.join("identity.json");
    fs::write(path, serde_json::to_vec_pretty(identity)?).await?;
    Ok(())
}
