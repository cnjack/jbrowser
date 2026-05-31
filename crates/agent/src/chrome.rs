use std::time::Duration;

use anyhow::Context;
use jbrowser_shared::models::{BrowserConfig, BrowserTab, StealthLevel};
use tokio::process::{Child, Command};
use tracing::info;

use crate::globals::ACTIVE_TAB_TX;

pub(crate) async fn start_chrome(config: &BrowserConfig) -> anyhow::Result<Child> {
    info!("Starting Chrome headless …");

    let window_size = format!(
        "--window-size={},{}",
        config.fingerprint.viewport_width, config.fingerprint.viewport_height
    );
    let scale = format!(
        "--force-device-scale-factor={}",
        config.fingerprint.device_scale_factor
    );

    let mut cmd = Command::new("chromium");

    // Set timezone via environment variable
    if let Some(tz) = &config.fingerprint.timezone {
        cmd.env("TZ", tz);
    }

    let chrome = cmd
        .args([
            "--headless=new",
            "--no-sandbox",
            "--disable-dev-shm-usage",
            "--remote-debugging-port=9222",
            "--remote-debugging-address=0.0.0.0",
            "--disable-gpu",
            &window_size,
            &scale,
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

/// Build CDP commands to apply fingerprint emulation on a CDP session.
pub(crate) fn build_fingerprint_cdp_commands(config: &BrowserConfig, start_id: u64) -> Vec<String> {
    let mut cmds = Vec::new();
    let mut id = start_id;
    let fp = &config.fingerprint;

    // User-Agent override
    if let Some(ua) = &fp.user_agent {
        cmds.push(
            serde_json::json!({
                "id": id,
                "method": "Emulation.setUserAgentOverride",
                "params": { "userAgent": ua }
            })
            .to_string(),
        );
        id += 1;
    }

    // Device metrics (viewport + DPR)
    cmds.push(
        serde_json::json!({
            "id": id,
            "method": "Emulation.setDeviceMetricsOverride",
            "params": {
                "width": fp.viewport_width,
                "height": fp.viewport_height,
                "deviceScaleFactor": fp.device_scale_factor,
                "mobile": false
            }
        })
        .to_string(),
    );
    id += 1;

    // Timezone override
    if let Some(tz) = &fp.timezone {
        cmds.push(
            serde_json::json!({
                "id": id,
                "method": "Emulation.setTimezoneOverride",
                "params": { "timezoneId": tz }
            })
            .to_string(),
        );
        id += 1;
    }

    // Locale override
    if let Some(locale) = &fp.locale {
        cmds.push(
            serde_json::json!({
                "id": id,
                "method": "Emulation.setLocaleOverride",
                "params": { "locale": locale }
            })
            .to_string(),
        );
        id += 1;
    }

    // Stealth scripts
    if config.stealth == StealthLevel::Basic {
        cmds.push(
            serde_json::json!({
                "id": id,
                "method": "Page.addScriptToEvaluateOnNewDocument",
                "params": { "source": STEALTH_BASIC_SCRIPT }
            })
            .to_string(),
        );
    }

    cmds
}

/// Basic stealth: hide obvious automation fingerprints
const STEALTH_BASIC_SCRIPT: &str = r#"
// 1. Remove navigator.webdriver
Object.defineProperty(navigator, 'webdriver', { get: () => undefined });

// 2. Fix Chrome detection point
window.chrome = { runtime: {} };

// 3. Fix permissions query
const originalQuery = window.navigator.permissions.query;
window.navigator.permissions.query = (parameters) => (
    parameters.name === 'notifications' ?
        Promise.resolve({ state: Notification.permission }) :
        originalQuery(parameters)
);

// 4. Fix plugins (headless defaults to empty)
Object.defineProperty(navigator, 'plugins', {
    get: () => [1, 2, 3, 4, 5],
});

// 5. Fix languages
Object.defineProperty(navigator, 'languages', {
    get: () => ['en-US', 'en'],
});
"#;
