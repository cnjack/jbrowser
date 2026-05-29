use std::{env, path::PathBuf, sync::OnceLock, time::Duration};

use anyhow::Context;
use base64::Engine;
use bytes::Bytes;
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use jbrowser_shared::{
    models::BrowserTab,
    protocol::{encode_video_frame, ControlToAgentMessage, VideoFrame, VideoFrameType},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::{
    fs,
    process::{Child, Command},
    sync::{broadcast, mpsc, Mutex},
    time::interval,
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

// ── Global persistent input channel ────────────────────────────────────────
static INPUT_TX: OnceLock<mpsc::Sender<serde_json::Value>> = OnceLock::new();

fn input_tx() -> Option<&'static mpsc::Sender<serde_json::Value>> {
    INPUT_TX.get()
}

// ── Entry point ─────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,jbrowser_agent=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AgentConfig::from_env()?;
    fs::create_dir_all(&config.data_dir).await?;
    let identity = load_or_register(&config).await?;
    save_identity(&config, &identity).await?;

    // Start Chrome headless; keep handle alive for the whole process.
    let _chrome = start_chrome().await?;

    // Broadcast channel: screencast reader → all active WS connections.
    let (video_tx, _) = broadcast::channel::<Bytes>(128);
    // Last frame cache: screencast stores the latest frame for immediate delivery
    let last_frame: Arc<Mutex<Option<Bytes>>> = Arc::new(Mutex::new(None));

    // Persistent input channel: all input/navigate commands go through here
    // to avoid creating a new CDP WS connection per event.
    let (input_chan_tx, input_chan_rx) = mpsc::channel::<serde_json::Value>(256);
    INPUT_TX.set(input_chan_tx).expect("input_tx already set");
    tokio::spawn(input_loop(input_chan_rx));

    // Start CDP screencast task
    tokio::spawn(screencast_loop(video_tx.clone(), last_frame.clone()));

    connect_loop(config, identity, video_tx, last_frame).await
}

// ── Config / identity ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct AgentConfig {
    control_http_url: String,
    control_ws_url: String,
    registration_token: String,
    data_dir: PathBuf,
    agent_name: String,
    browser_type: String,
    browser_version: String,
}

impl AgentConfig {
    fn from_env() -> anyhow::Result<Self> {
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
            browser_version: env::var("BROWSER_VERSION")
                .unwrap_or_else(|_| "unknown".to_string()),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentIdentity {
    agent_id: String,
    browser_instance_id: String,
    agent_runtime_token: String,
}

async fn load_or_register(config: &AgentConfig) -> anyhow::Result<AgentIdentity> {
    let path = config.data_dir.join("identity.json");
    if let Ok(raw) = fs::read_to_string(&path).await {
        return serde_json::from_str(&raw).context("invalid identity file");
    }
    let client = reqwest::Client::new();
    let identity = client
        .post(format!(
            "{}/api/v1/agents/register",
            config.control_http_url
        ))
        .json(&json!({
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

async fn save_identity(config: &AgentConfig, identity: &AgentIdentity) -> anyhow::Result<()> {
    let path = config.data_dir.join("identity.json");
    fs::write(path, serde_json::to_vec_pretty(identity)?).await?;
    Ok(())
}

// ── Chrome process ───────────────────────────────────────────────────────────

async fn start_chrome() -> anyhow::Result<Child> {
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

    // Wait until the CDP HTTP endpoint responds (up to 15 s).
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

// ── CDP Screencast loop ──────────────────────────────────────────────────────

/// Connects to Chrome CDP, starts Page.startScreencast, and broadcasts
/// JPEG frames to all connected WS clients.
async fn screencast_loop(video_tx: broadcast::Sender<Bytes>, last_frame: Arc<Mutex<Option<Bytes>>>) {
    loop {
        if let Err(e) = run_screencast(&video_tx, &last_frame).await {
            warn!("screencast error: {e}");
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn run_screencast(video_tx: &broadcast::Sender<Bytes>, last_frame: &Arc<Mutex<Option<Bytes>>>) -> anyhow::Result<()> {
    let target = ensure_page_target().await?;
    let ws_url = target["webSocketDebuggerUrl"]
        .as_str()
        .context("target missing webSocketDebuggerUrl")?;

    let (mut cdp_ws, _) = connect_async(ws_url)
        .await
        .context("CDP WS connect failed")?;

    // Start screencast
    cdp_ws.send(Message::Text(
        json!({
            "id": 1,
            "method": "Page.startScreencast",
            "params": {
                "format": "jpeg",
                "quality": 80,
                "maxWidth": 1280,
                "maxHeight": 720,
                "everyNthFrame": 1
            }
        })
        .to_string(),
    ))
    .await?;

    info!("CDP screencast started");
    let mut sequence: u64 = 0;

    loop {
        match cdp_ws.next().await {
            Some(Ok(Message::Text(text))) => {
                let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };

                if value.get("method").and_then(|v| v.as_str()) == Some("Page.screencastFrame") {
                    let params = &value["params"];
                    let session_id = params["sessionId"].as_i64().unwrap_or(0);
                    let data_b64 = params["data"].as_str().unwrap_or("");

                    // Decode base64 JPEG
                    if let Ok(jpeg_bytes) = base64::engine::general_purpose::STANDARD.decode(data_b64) {
                        let ts = Utc::now().timestamp_millis() as u64;
                        let encoded = encode_video_frame(&VideoFrame {
                            frame_type: VideoFrameType::Jpeg,
                            stream_id: 0,
                            sequence,
                            timestamp_ms: ts,
                            payload: Bytes::from(jpeg_bytes),
                        });
                        sequence += 1;
                        // Always send — drops are fine when no subscribers
                        let _ = video_tx.send(encoded.clone());
                        // Cache for instant delivery on reconnect
                        *last_frame.lock().await = Some(encoded);
                    }

                    // Acknowledge the frame
                    let ack = json!({
                        "id": 100 + session_id,
                        "method": "Page.screencastFrameAck",
                        "params": { "sessionId": session_id }
                    });
                    if cdp_ws.send(Message::Text(ack.to_string())).await.is_err() {
                        break;
                    }
                }
            }
            Some(Ok(Message::Close(_))) | None => break,
            Some(Err(e)) => {
                error!("CDP WS error: {e}");
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

// ── Tab poll task ────────────────────────────────────────────────────────────

async fn tab_poll_task(tx: mpsc::Sender<Message>, _identity: AgentIdentity) {
    let mut ticker = interval(Duration::from_secs(2));
    let mut prev_fingerprints: Vec<String> = Vec::new();

    loop {
        ticker.tick().await;

        let targets = match cdp_get_targets().await {
            Ok(t) => t,
            Err(e) => {
                warn!("tab poll failed: {e}");
                continue;
            }
        };

        let tabs = cdp_to_browser_tabs(&targets);
        let fingerprints: Vec<String> = tabs
            .iter()
            .map(|t| format!("{}|{}|{}", t.id, t.url, t.title))
            .collect();

        if fingerprints != prev_fingerprints {
            prev_fingerprints = fingerprints;
            let msg = json!({ "type": "tab.list", "payload": { "tabs": tabs } });
            if tx.send(Message::Text(msg.to_string())).await.is_err() {
                break;
            }
        }
    }
}

// ── Video stream task ────────────────────────────────────────────────────────

async fn video_stream_task(
    mut video_rx: broadcast::Receiver<Bytes>,
    tx: mpsc::Sender<Message>,
) {
    loop {
        match video_rx.recv().await {
            Ok(data) => {
                if tx.send(Message::Binary(data.to_vec())).await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("video stream lagged by {n} frames");
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

// ── CDP helpers ──────────────────────────────────────────────────────────────

/// Persistent input loop: keeps a single CDP WebSocket open and forwards all
/// input / navigate commands without the overhead of a new handshake per event.
async fn input_loop(mut rx: mpsc::Receiver<serde_json::Value>) {
    loop {
        match run_input_session(&mut rx).await {
            Ok(()) => warn!("input CDP session closed, reconnecting in 500ms"),
            Err(e) => warn!("input CDP session error: {e}, reconnecting in 500ms"),
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn run_input_session(rx: &mut mpsc::Receiver<serde_json::Value>) -> anyhow::Result<()> {
    let target = ensure_page_target().await?;
    let ws_url = target["webSocketDebuggerUrl"]
        .as_str()
        .context("target has no webSocketDebuggerUrl")?;
    let (ws, _) = connect_async(ws_url)
        .await
        .context("input CDP WS connect failed")?;
    let (mut write, mut read) = ws.split();
    info!("input CDP session connected");

    let mut cmd_id: u64 = 1000;
    loop {
        tokio::select! {
            msg = rx.recv() => {
                let Some(payload) = msg else { return Ok(()); };
                let commands = build_input_cdp_commands(&mut cmd_id, &payload);
                if !commands.is_empty() {
                    info!("input CDP sending {} command(s) for type={}", commands.len(), payload["type"].as_str().unwrap_or("?"));
                }
                for cmd in commands {
                    write.send(Message::Text(cmd)).await?;
                }
            }
            item = read.next() => {
                match item {
                    None => return Ok(()),
                    Some(Err(e)) => return Err(e.into()),
                    Some(Ok(_)) => {} // ignore CDP responses
                }
            }
        }
    }
}

fn build_input_cdp_commands(cmd_id: &mut u64, payload: &serde_json::Value) -> Vec<String> {
    let event_type = payload["type"].as_str().unwrap_or("");
    let x = payload["x"].as_f64().unwrap_or(0.0);
    let y = payload["y"].as_f64().unwrap_or(0.0);
    let mut cmds = Vec::new();

    match event_type {
        "navigate.url" => {
            let url = payload["url"].as_str().unwrap_or("about:blank");
            cmds.push(json!({"id": *cmd_id, "method": "Page.navigate", "params": {"url": url}}).to_string());
            *cmd_id += 1;
        }
        "navigate.back" => {
            cmds.push(json!({"id": *cmd_id, "method": "Page.goBack", "params": {}}).to_string());
            *cmd_id += 1;
        }
        "navigate.forward" => {
            cmds.push(json!({"id": *cmd_id, "method": "Page.goForward", "params": {}}).to_string());
            *cmd_id += 1;
        }
        "navigate.reload" => {
            cmds.push(json!({"id": *cmd_id, "method": "Page.reload", "params": {}}).to_string());
            *cmd_id += 1;
        }
        "click" => {
            let button = payload["button"].as_str().unwrap_or("left");
            let modifiers = payload["modifiers"].as_i64().unwrap_or(0);
            cmds.push(json!({"id": *cmd_id, "method": "Input.dispatchMouseEvent",
                "params": {"type": "mousePressed", "x": x, "y": y, "button": button, "clickCount": 1, "modifiers": modifiers}}).to_string());
            *cmd_id += 1;
            cmds.push(json!({"id": *cmd_id, "method": "Input.dispatchMouseEvent",
                "params": {"type": "mouseReleased", "x": x, "y": y, "button": button, "clickCount": 1, "modifiers": modifiers}}).to_string());
            *cmd_id += 1;
        }
        "mousedown" => {
            let button = payload["button"].as_str().unwrap_or("left");
            let click_count = payload["clickCount"].as_i64().unwrap_or(1);
            let modifiers = payload["modifiers"].as_i64().unwrap_or(0);
            cmds.push(json!({"id": *cmd_id, "method": "Input.dispatchMouseEvent",
                "params": {"type": "mousePressed", "x": x, "y": y, "button": button, "clickCount": click_count, "modifiers": modifiers}}).to_string());
            *cmd_id += 1;
        }
        "mouseup" => {
            let button = payload["button"].as_str().unwrap_or("left");
            let click_count = payload["clickCount"].as_i64().unwrap_or(1);
            let modifiers = payload["modifiers"].as_i64().unwrap_or(0);
            cmds.push(json!({"id": *cmd_id, "method": "Input.dispatchMouseEvent",
                "params": {"type": "mouseReleased", "x": x, "y": y, "button": button, "clickCount": click_count, "modifiers": modifiers}}).to_string());
            *cmd_id += 1;
        }
        "mousemove" => {
            let modifiers = payload["modifiers"].as_i64().unwrap_or(0);
            cmds.push(json!({"id": *cmd_id, "method": "Input.dispatchMouseEvent",
                "params": {"type": "mouseMoved", "x": x, "y": y, "modifiers": modifiers}}).to_string());
            *cmd_id += 1;
        }
        "wheel" => {
            let dx = payload["deltaX"].as_f64().unwrap_or(0.0);
            let dy = payload["deltaY"].as_f64().unwrap_or(0.0);
            cmds.push(json!({"id": *cmd_id, "method": "Input.dispatchMouseEvent",
                "params": {"type": "mouseWheel", "x": x, "y": y, "deltaX": dx, "deltaY": dy}}).to_string());
            *cmd_id += 1;
        }
        "keydown" | "keyup" => {
            let key = payload["key"].as_str().unwrap_or("");
            let code = payload["code"].as_str().unwrap_or("");
            let text = payload["text"].as_str().unwrap_or("");
            let modifiers = payload["modifiers"].as_i64().unwrap_or(0);
            let key_code = payload["keyCode"].as_i64().unwrap_or(0);
            let cdp_type = if event_type == "keydown" { "keyDown" } else { "keyUp" };
            cmds.push(json!({"id": *cmd_id, "method": "Input.dispatchKeyEvent",
                "params": {"type": cdp_type, "key": key, "code": code, "text": text,
                           "modifiers": modifiers, "windowsVirtualKeyCode": key_code,
                           "nativeVirtualKeyCode": key_code}}).to_string());
            *cmd_id += 1;
        }
        other => warn!("unknown input event type: {other}"),
    }
    cmds
}



async fn cdp_get_targets() -> anyhow::Result<Vec<serde_json::Value>> {
    reqwest::Client::new()
        .get("http://localhost:9222/json/list")
        .send()
        .await?
        .error_for_status()?
        .json::<Vec<serde_json::Value>>()
        .await
        .context("failed to parse /json/list")
}

async fn ensure_page_target() -> anyhow::Result<serde_json::Value> {
    if let Some(target) = cdp_get_targets()
        .await?
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

fn cdp_to_browser_tabs(targets: &[serde_json::Value]) -> Vec<BrowserTab> {
    targets
        .iter()
        .filter(|t| t["type"].as_str() == Some("page"))
        .enumerate()
        .map(|(i, t)| BrowserTab {
            id: t["id"].as_str().unwrap_or("").to_string(),
            title: t["title"].as_str().unwrap_or("").to_string(),
            url: t["url"].as_str().unwrap_or("").to_string(),
            active: i == 0, // first tab is always treated as active (main screencast target)
        })
        .collect()
}

async fn handle_navigate(payload: &serde_json::Value) -> anyhow::Result<()> {
    let url = payload["url"].as_str().context("navigate.url: missing url")?;
    if let Some(tx) = input_tx() {
        let _ = tx.try_send(json!({"type": "navigate.url", "url": url}));
    }
    Ok(())
}

async fn handle_navigate_history(method: &str) -> anyhow::Result<()> {
    let event_type = match method {
        "Page.goBack" => "navigate.back",
        "Page.goForward" => "navigate.forward",
        "Page.reload" => "navigate.reload",
        _ => return Ok(()),
    };
    if let Some(tx) = input_tx() {
        let _ = tx.try_send(json!({"type": event_type}));
    }
    Ok(())
}

async fn handle_input_event(payload: &serde_json::Value) -> anyhow::Result<()> {
    if let Some(tx) = input_tx() {
        let _ = tx.try_send(payload.clone());
    }
    Ok(())
}

async fn handle_tab_command(payload: &serde_json::Value) -> anyhow::Result<()> {
    info!("handle_tab_command: {:?}", payload);
    let client = reqwest::Client::new();
    match payload["command"].as_str().unwrap_or("") {
        "new" | "open" => {
            let url = payload["url"].as_str().unwrap_or("about:blank");
            let resp_text = client
                .put(format!("http://localhost:9222/json/new?{url}"))
                .send()
                .await?
                .text()
                .await
                .unwrap_or_default();
            info!("json/new response: {:?}", &resp_text[..resp_text.len().min(200)]);
            if url != "about:blank" {
                let resp: serde_json::Value = serde_json::from_str(&resp_text)
                    .unwrap_or(serde_json::Value::Null);
                if let Some(ws_url) = resp["webSocketDebuggerUrl"].as_str() {
                    if let Ok((mut tab_ws, _)) = connect_async(ws_url).await {
                        let _ = tab_ws.send(Message::Text(
                            json!({"id":1,"method":"Page.navigate","params":{"url":url}})
                                .to_string(),
                        ))
                        .await;
                        tab_ws.close(None).await.ok();
                    }
                }
            }
        }
        "close" => {
            if let Some(tab_id) = payload["tabId"].as_str() {
                client
                    .post(format!("http://localhost:9222/json/close/{tab_id}"))
                    .send()
                    .await?
                    .error_for_status()?;
            }
        }
        "activate" => {
            if let Some(tab_id) = payload["tabId"].as_str() {
                client
                    .post(format!("http://localhost:9222/json/activate/{tab_id}"))
                    .send()
                    .await?
                    .error_for_status()?;
            }
        }
        other => warn!("unknown tab command: {other}"),
    }
    Ok(())
}

async fn handle_browser_reset(_payload: &serde_json::Value) -> anyhow::Result<()> {
    let client = reqwest::Client::new();

    let targets: Vec<serde_json::Value> = client
        .get("http://localhost:9222/json/list")
        .send()
        .await?
        .json()
        .await?;

    let pages: Vec<_> = targets
        .iter()
        .filter(|t| t["type"].as_str() == Some("page"))
        .collect();

    // Keep first tab, close the rest
    for (i, tab) in pages.iter().enumerate() {
        if i > 0 {
            if let Some(id) = tab["id"].as_str() {
                let _ = client
                    .post(format!("http://localhost:9222/json/close/{id}"))
                    .send()
                    .await;
            }
        }
    }

    // Navigate the remaining/first tab to about:blank and clear cookies/cache
    if let Some(tab) = pages.first() {
        if let Some(ws_url) = tab["webSocketDebuggerUrl"].as_str() {
            let (mut ws, _) = connect_async(ws_url).await?;
            ws.send(Message::Text(
                json!({"id":1,"method":"Page.navigate","params":{"url":"about:blank"}}).to_string(),
            ))
            .await?;
            ws.send(Message::Text(
                json!({"id":2,"method":"Network.clearBrowserCookies","params":{}}).to_string(),
            ))
            .await?;
            ws.send(Message::Text(
                json!({"id":3,"method":"Network.clearBrowserCache","params":{}}).to_string(),
            ))
            .await?;
            ws.close(None).await.ok();
        } else {
            client
                .get("http://localhost:9222/json/new")
                .send()
                .await
                .ok();
        }
    } else {
        client
            .get("http://localhost:9222/json/new")
            .send()
            .await
            .ok();
    }

    Ok(())
}

async fn dispatch_control_message(msg: ControlToAgentMessage) {
    if msg.kind == "input.event" {
        let evt = msg.payload["type"].as_str().unwrap_or("?");
        let x = msg.payload["x"].as_f64().unwrap_or(-1.0);
        let y = msg.payload["y"].as_f64().unwrap_or(-1.0);
        info!("input.event type={evt} x={x:.0} y={y:.0}");
    } else {
        info!("control message: {}", msg.kind);
    }
    let result = match msg.kind.as_str() {
        "navigate.url" => handle_navigate(&msg.payload).await,
        "navigate.back" => handle_navigate_history("Page.goBack").await,
        "navigate.forward" => handle_navigate_history("Page.goForward").await,
        "navigate.reload" => handle_navigate_history("Page.reload").await,
        "input.event" => handle_input_event(&msg.payload).await,
        "tab.command" => handle_tab_command(&msg.payload).await,
        "browser.reset" => handle_browser_reset(&msg.payload).await,
        other => {
            info!("unhandled control message kind: {other}");
            Ok(())
        }
    };
    if let Err(e) = result {
        warn!("control message '{}' error: {e}", msg.kind);
    }
}

// ── WebSocket connect loop ───────────────────────────────────────────────────

async fn connect_loop(
    config: AgentConfig,
    mut identity: AgentIdentity,
    video_tx: broadcast::Sender<Bytes>,
    last_frame: Arc<Mutex<Option<Bytes>>>,
) -> anyhow::Result<()> {
    loop {
        match connect_once(&config, &identity, video_tx.clone(), last_frame.clone()).await {
            Ok(()) => warn!("agent websocket disconnected"),
            Err(err) => {
                let msg = err.to_string();
                warn!(error = %msg, "agent websocket failed");
                if msg.contains("401") || msg.contains("Unauthorized") {
                    warn!("runtime token rejected — deleting identity and re-registering");
                    let id_path = config.data_dir.join("identity.json");
                    let _ = fs::remove_file(&id_path).await;
                    match load_or_register(&config).await {
                        Ok(new_id) => {
                            if let Err(e) = save_identity(&config, &new_id).await {
                                warn!("failed to save new identity: {e}");
                            }
                            identity = new_id;
                        }
                        Err(e) => warn!("re-registration failed: {e}"),
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn connect_once(
    config: &AgentConfig,
    identity: &AgentIdentity,
    video_tx: broadcast::Sender<Bytes>,
    _last_frame: Arc<Mutex<Option<Bytes>>>,
) -> anyhow::Result<()> {
    let request = http_request_with_bearer(
        &format!("{}/api/v1/agents/connect", config.control_ws_url),
        &identity.agent_runtime_token,
    )?;
    let (socket, _) = connect_async(request).await?;
    let (mut ws_write, mut ws_read) = socket.split();
    info!("agent websocket connected");

    let (tx, mut rx) = mpsc::channel::<Message>(256);

    let writer_handle = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_write.send(msg).await.is_err() {
                break;
            }
        }
    });

    let video_rx = video_tx.subscribe();
    let video_handle = tokio::spawn(video_stream_task(video_rx, tx.clone()));

    let tab_handle = tokio::spawn(tab_poll_task(tx.clone(), identity.clone()));

    let mut heartbeat = interval(Duration::from_secs(5));

    let result: anyhow::Result<()> = loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                let msg = json!({
                    "type": "heartbeat",
                    "payload": {
                        "agent_id": identity.agent_id,
                        "browser_instance_id": identity.browser_instance_id
                    }
                });
                if tx.send(Message::Text(msg.to_string())).await.is_err() {
                    break Ok(());
                }
            }
            maybe_msg = ws_read.next() => {
                match maybe_msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ControlToAgentMessage>(&text) {
                            Ok(msg) => { tokio::spawn(dispatch_control_message(msg)); }
                            Err(e) => warn!("failed to parse control message: {e}"),
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break Ok(()),
                    Some(Err(e)) => break Err(anyhow::Error::from(e)),
                    _ => {}
                }
            }
        }
    };

    writer_handle.abort();
    video_handle.abort();
    tab_handle.abort();
    result
}

// ── HTTP/WS helpers ──────────────────────────────────────────────────────────

fn http_request_with_bearer(url: &str, token: &str) -> anyhow::Result<http::Request<()>> {
    use tokio_tungstenite::tungstenite::handshake::client::generate_key;
    let parsed = url::Url::parse(url).context("invalid websocket url")?;
    let host = parsed
        .host_str()
        .context("url has no host")?
        .to_string();
    let host_header = match parsed.port() {
        Some(p) => format!("{host}:{p}"),
        None => host,
    };
    Ok(http::Request::builder()
        .uri(url)
        .header("Host", host_header)
        .header("Authorization", format!("Bearer {token}"))
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", generate_key())
        .body(())?)
}
