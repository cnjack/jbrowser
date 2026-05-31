use std::time::Duration;

use anyhow::Context;
use jbrowser_shared::models::BrowserTab;
use tokio::process::{Child, Command};
use tracing::info;

use crate::globals::ACTIVE_TAB_TX;

pub(crate) async fn start_chrome() -> anyhow::Result<Child> {
    info!("Starting Chrome headless …");
    let chrome = Command::new("chromium")
        .args([
            "--headless=new",
            "--no-sandbox",
            "--disable-dev-shm-usage",
            "--remote-debugging-port=9222",
            "--remote-debugging-address=0.0.0.0",
            "--disable-gpu",
            "--window-size=1280,720",
            "--force-device-scale-factor=1",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-extensions",
            "--disable-background-networking",
            "--disable-client-side-phishing-detection",
            "--disable-sync",
            "--disable-features=GCMDriver",
            "--disable-notifications",
            "--no-service-autorun",
            "--password-store=basic",
            "about:blank",
        ])
        .spawn()
        .context("failed to spawn chromium")?;

    let http = reqwest::Client::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        if let Ok(r) = http.get("http://localhost:9222/json/version").send().await {
            if r.status().is_success() {
                info!("Chrome CDP ready");
                ensure_page_target().await?;
                break;
            }
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("Chrome CDP did not become available within 15 s");
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    Ok(chrome)
}

pub(crate) async fn cdp_get_targets() -> anyhow::Result<Vec<serde_json::Value>> {
    reqwest::Client::new()
        .get("http://localhost:9222/json/list")
        .send()
        .await?
        .error_for_status()?
        .json::<Vec<serde_json::Value>>()
        .await
        .context("failed to parse /json/list")
}

pub(crate) async fn ensure_page_target() -> anyhow::Result<serde_json::Value> {
    let targets = cdp_get_targets().await?;

    if let Some(tx) = ACTIVE_TAB_TX.get() {
        let active_id = tx.borrow().clone();
        if let Some(id) = &active_id {
            if let Some(target) = targets.iter().find(|t| {
                t["id"].as_str() == Some(id.as_str()) && t["type"].as_str() == Some("page")
            }) {
                return Ok(target.clone());
            }
        }
    }

    if let Some(target) = targets
        .into_iter()
        .find(|t| t["type"].as_str() == Some("page"))
    {
        return Ok(target);
    }

    let client = reqwest::Client::new();
    let created = client
        .put("http://localhost:9222/json/new?about:blank")
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    if created["type"].as_str() == Some("page") {
        Ok(created)
    } else {
        cdp_get_targets()
            .await?
            .into_iter()
            .find(|t| t["type"].as_str() == Some("page"))
            .context("no page target available after creating about:blank")
    }
}

pub(crate) fn cdp_to_browser_tabs(targets: &[serde_json::Value]) -> Vec<BrowserTab> {
    let active_tab_id = ACTIVE_TAB_TX.get().and_then(|tx| tx.borrow().clone());

    targets
        .iter()
        .filter(|t| t["type"].as_str() == Some("page"))
        .enumerate()
        .map(|(i, t)| {
            let id = t["id"].as_str().unwrap_or("").to_string();
            let is_active = match &active_tab_id {
                Some(active_id) => id == *active_id,
                None => i == 0,
            };
            BrowserTab {
                id,
                title: t["title"].as_str().unwrap_or("").to_string(),
                url: t["url"].as_str().unwrap_or("").to_string(),
                active: is_active,
            }
        })
        .collect()
}
