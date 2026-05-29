use std::{env, path::PathBuf, sync::Arc, time::Duration};

use anyhow::Context;
use bytes::Bytes;
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use jbrowser_shared::{
    models::BrowserTab,
    protocol::{encode_video_frame, ControlToAgentMessage, VideoFrame, VideoFrameType},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{
    fs,
    io::AsyncReadExt,
    process::{Child, ChildStdout, Command},
    sync::{broadcast, mpsc, Mutex},
    time::interval,
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

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

    // Start Xvfb, Chrome, ffmpeg once; keep handles alive for the whole process.
    let (_handles, ffmpeg_stdout) = start_browser_processes().await?;

    // Shared init-segment cache: the first fMP4 chunk, re-sent to every new
    // WS client so it can decode subsequent media segments.
    let init_cache: Arc<Mutex<Option<Bytes>>> = Arc::new(Mutex::new(None));
    // Broadcast channel: ffmpeg reader → all active WS connections.
    let (video_tx, _) = broadcast::channel::<Bytes>(64);

    tokio::spawn(ffmpeg_reader_task(
        ffmpeg_stdout,
        init_cache.clone(),
        video_tx.clone(),
    ));

    connect_loop(config, identity, init_cache, video_tx).await
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

// ── Process management ───────────────────────────────────────────────────────

struct ProcessHandles {
    _xvfb: Child,
    _chrome: Child,
    _ffmpeg: Child,
}

async fn start_browser_processes() -> anyhow::Result<(ProcessHandles, ChildStdout)> {
    // Clean up any stale X lock / socket files from a previous run.
    for path in &[
        "/tmp/.X99-lock",
        "/tmp/.X11-unix/X99",
    ] {
        let _ = tokio::fs::remove_file(path).await;
    }

    // 1. Xvfb
    info!("Starting Xvfb on :99 …");
    let xvfb = Command::new("Xvfb")
        .args([":99", "-screen", "0", "1280x720x24"])
        .spawn()
        .context("failed to spawn Xvfb")?;
    // Give the display time to initialise before Chrome tries to use it.
    tokio::time::sleep(Duration::from_secs(1)).await;

    // 2. Chromium
    info!("Starting chromium …");
    let chrome = Command::new("chromium")
        .args([
            "--headless=false",
            "--no-sandbox",
            "--disable-dev-shm-usage",
            "--remote-debugging-port=9222",
            "--display=:99",
            "--window-size=1280,720",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-extensions",
        ])
        .env("DISPLAY", ":99")
        .spawn()
        .context("failed to spawn chromium")?;

    // Wait until the CDP HTTP endpoint responds (up to 10 s).
    let http = reqwest::Client::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(r) = http.get("http://localhost:9222/json/version").send().await {
            if r.status().is_success() {
                info!("Chrome CDP ready");
                break;
            }
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("Chrome CDP did not become available within 10 s");
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // 3. ffmpeg – capture :99, encode as fMP4, write to stdout
    info!("Starting ffmpeg screen capture …");
    let mut ffmpeg = Command::new("ffmpeg")
        .args([
            "-f",
            "x11grab",
            "-r",
            "15",
            "-s",
            "1280x720",
            "-i",
            ":99",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-tune",
            "zerolatency",
            "-profile:v",
            "baseline",
            "-level",
            "3.0",
            "-pix_fmt",
            "yuv420p",
            "-f",
            "mp4",
            "-movflags",
            "frag_keyframe+empty_moov+default_base_moof",
            "-frag_duration",
            "1000000",
            "-an",
            "-",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("failed to spawn ffmpeg")?;

    let stdout = ffmpeg.stdout.take().context("ffmpeg stdout unavailable")?;
    Ok((
        ProcessHandles {
            _xvfb: xvfb,
            _chrome: chrome,
            _ffmpeg: ffmpeg,
        },
        stdout,
    ))
}

// ── ffmpeg reader – runs for the lifetime of the process ────────────────────

/// Reads ffmpeg stdout in 16 KB chunks, wraps each in a `VideoFrame`, and
/// broadcasts encoded bytes to all active WS connections.
/// The very first chunk is the fMP4 init segment and is cached so that
/// late-joining clients can still decode the stream.
async fn ffmpeg_reader_task(
    mut stdout: ChildStdout,
    init_cache: Arc<Mutex<Option<Bytes>>>,
    video_tx: broadcast::Sender<Bytes>,
) {
    let mut buf = vec![0u8; 16 * 1024];
    let mut sequence: u64 = 0;
    // Accumulate pre-fragment MP4 boxes (ftyp + moov) before the first moof box.
    let mut init_buf: Vec<u8> = Vec::new();
    let mut init_done = false;

    loop {
        match stdout.read(&mut buf).await {
            Ok(0) => {
                warn!("ffmpeg stdout closed (EOF)");
                break;
            }
            Ok(n) => {
                let chunk = &buf[..n];

                if !init_done {
                    init_buf.extend_from_slice(chunk);
                    // Check if we've received a moof box — that signals end of init.
                    if contains_box(&init_buf, b"moof") {
                        // Split: everything before the first moof is init, the rest is the first media chunk.
                        let moof_offset = find_box_offset(&init_buf, b"moof").unwrap_or(init_buf.len());
                        let init_payload = Bytes::copy_from_slice(&init_buf[..moof_offset]);
                        let media_payload = Bytes::copy_from_slice(&init_buf[moof_offset..]);
                        init_done = true;

                        let init_encoded = encode_video_frame(&VideoFrame {
                            frame_type: VideoFrameType::Init,
                            stream_id: 0,
                            sequence: 0,
                            timestamp_ms: 0,
                            payload: init_payload,
                        });
                        *init_cache.lock().await = Some(init_encoded.clone());
                        if video_tx.receiver_count() > 0 {
                            let _ = video_tx.send(init_encoded);
                        }

                        if !media_payload.is_empty() {
                            let ts = Utc::now().timestamp_millis() as u64;
                            let media_encoded = encode_video_frame(&VideoFrame {
                                frame_type: VideoFrameType::Media,
                                stream_id: 0,
                                sequence,
                                timestamp_ms: ts,
                                payload: media_payload,
                            });
                            sequence += 1;
                            if video_tx.receiver_count() > 0 {
                                let _ = video_tx.send(media_encoded);
                            }
                        }
                    }
                    // else keep buffering until we see moof
                } else {
                    let ts = Utc::now().timestamp_millis() as u64;
                    let encoded = encode_video_frame(&VideoFrame {
                        frame_type: VideoFrameType::Media,
                        stream_id: 0,
                        sequence,
                        timestamp_ms: ts,
                        payload: Bytes::copy_from_slice(chunk),
                    });
                    sequence += 1;
                    if video_tx.receiver_count() > 0 {
                        let _ = video_tx.send(encoded);
                    }
                }
            }
            Err(e) => {
                error!("ffmpeg read error: {e}");
                break;
            }
        }
    }
}

/// Returns true if `data` contains an MP4 box with the given 4-byte type.
fn contains_box(data: &[u8], box_type: &[u8; 4]) -> bool {
    find_box_offset(data, box_type).is_some()
}

/// Returns the byte offset of the first MP4 box with the given type, or None.
fn find_box_offset(data: &[u8], box_type: &[u8; 4]) -> Option<usize> {
    let mut pos = 0usize;
    while pos + 8 <= data.len() {
        let size = u32::from_be_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
        let t = &data[pos+4..pos+8];
        if t == box_type {
            return Some(pos);
        }
        if size < 8 { break; } // guard against malformed data
        pos += size;
    }
    None
}

// ── Per-connection tasks ─────────────────────────────────────────────────────

/// Sends the cached init segment to the WS client (so the player can decode),
/// then forwards every subsequent broadcast frame.
async fn video_stream_task(
    init_cache: Arc<Mutex<Option<Bytes>>>,
    mut video_rx: broadcast::Receiver<Bytes>,
    tx: mpsc::Sender<Message>,
) {
    // Subscribe before reading the cache so no frame is missed.
    {
        let guard = init_cache.lock().await;
        if let Some(init) = guard.clone() {
            if tx.send(Message::Binary(init.to_vec())).await.is_err() {
                return;
            }
        }
    }

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

/// Polls Chrome's `/json/list` every 2 s and pushes a `tab.list` message
/// whenever the tab set changes.
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
        // Use a fingerprint (id+url+title) to detect changes.
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

// ── CDP helpers ──────────────────────────────────────────────────────────────

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

fn cdp_to_browser_tabs(targets: &[serde_json::Value]) -> Vec<BrowserTab> {
    targets
        .iter()
        .filter(|t| t["type"].as_str() == Some("page"))
        .map(|t| BrowserTab {
            id: t["id"].as_str().unwrap_or("").to_string(),
            title: t["title"].as_str().unwrap_or("").to_string(),
            url: t["url"].as_str().unwrap_or("").to_string(),
            active: false,
        })
        .collect()
}

/// Navigate the first `page` target to `url`.
async fn handle_navigate(payload: &serde_json::Value) -> anyhow::Result<()> {
    let url = payload["url"].as_str().context("navigate.url: missing url")?;
    let targets = cdp_get_targets().await?;
    let target = targets
        .iter()
        .find(|t| t["type"].as_str() == Some("page"))
        .context("no page target available")?;
    let ws_url = target["webSocketDebuggerUrl"]
        .as_str()
        .context("target has no webSocketDebuggerUrl")?;

    let (mut ws, _) = connect_async(ws_url)
        .await
        .context("CDP WS connect failed")?;
    ws.send(Message::Text(
        json!({"id":1,"method":"Page.navigate","params":{"url":url}}).to_string(),
    ))
    .await?;
    ws.close(None).await.ok();
    Ok(())
}

/// Dispatch a mouse event to the first `page` target.
async fn handle_input_event(payload: &serde_json::Value) -> anyhow::Result<()> {
    let targets = cdp_get_targets().await?;
    let target = targets
        .iter()
        .find(|t| t["type"].as_str() == Some("page"))
        .context("no page target available")?;
    let ws_url = target["webSocketDebuggerUrl"]
        .as_str()
        .context("target has no webSocketDebuggerUrl")?;

    let (mut ws, _) = connect_async(ws_url)
        .await
        .context("CDP WS connect failed")?;

    let event_type = payload["type"].as_str().unwrap_or("");
    let x = payload["x"].as_f64().unwrap_or(0.0);
    let y = payload["y"].as_f64().unwrap_or(0.0);

    match event_type {
        "click" => {
            let button = payload["button"].as_str().unwrap_or("left");
            let modifiers = payload["modifiers"].as_i64().unwrap_or(0) as i64;
            ws.send(Message::Text(
                json!({"id":1,"method":"Input.dispatchMouseEvent",
                       "params":{"type":"mousePressed","x":x,"y":y,
                                 "button":button,"clickCount":1,
                                 "modifiers":modifiers}})
                .to_string(),
            ))
            .await?;
            ws.send(Message::Text(
                json!({"id":2,"method":"Input.dispatchMouseEvent",
                       "params":{"type":"mouseReleased","x":x,"y":y,
                                 "button":button,"clickCount":1,
                                 "modifiers":modifiers}})
                .to_string(),
            ))
            .await?;
        }
        "wheel" => {
            let dx = payload["deltaX"].as_f64().unwrap_or(0.0);
            let dy = payload["deltaY"].as_f64().unwrap_or(0.0);
            ws.send(Message::Text(
                json!({"id":1,"method":"Input.dispatchMouseEvent",
                       "params":{"type":"mouseWheel","x":x,"y":y,
                                 "deltaX":dx,"deltaY":dy}})
                .to_string(),
            ))
            .await?;
        }
        "keydown" | "keyup" => {
            let key = payload["key"].as_str().unwrap_or("");
            let code = payload["code"].as_str().unwrap_or("");
            let text = payload["text"].as_str().unwrap_or("");
            let modifiers = payload["modifiers"].as_i64().unwrap_or(0) as i64;
            let key_code = payload["keyCode"].as_i64().unwrap_or(0) as i64;
            let cdp_event_type = if event_type == "keydown" { "keyDown" } else { "keyUp" };
            ws.send(Message::Text(
                json!({"id":1,"method":"Input.dispatchKeyEvent",
                       "params":{"type":cdp_event_type,"key":key,"code":code,
                                 "text":text,"modifiers":modifiers,
                                 "windowsVirtualKeyCode":key_code,
                                 "nativeVirtualKeyCode":key_code}})
                .to_string(),
            ))
            .await?;
        }
        "mousedown" => {
            let button = payload["button"].as_str().unwrap_or("left");
            let click_count = payload["clickCount"].as_i64().unwrap_or(1) as i64;
            let modifiers = payload["modifiers"].as_i64().unwrap_or(0) as i64;
            ws.send(Message::Text(
                json!({"id":1,"method":"Input.dispatchMouseEvent",
                       "params":{"type":"mousePressed","x":x,"y":y,
                                 "button":button,"clickCount":click_count,
                                 "modifiers":modifiers}})
                .to_string(),
            ))
            .await?;
        }
        "mouseup" => {
            let button = payload["button"].as_str().unwrap_or("left");
            let click_count = payload["clickCount"].as_i64().unwrap_or(1) as i64;
            let modifiers = payload["modifiers"].as_i64().unwrap_or(0) as i64;
            ws.send(Message::Text(
                json!({"id":1,"method":"Input.dispatchMouseEvent",
                       "params":{"type":"mouseReleased","x":x,"y":y,
                                 "button":button,"clickCount":click_count,
                                 "modifiers":modifiers}})
                .to_string(),
            ))
            .await?;
        }
        "mousemove" => {
            let modifiers = payload["modifiers"].as_i64().unwrap_or(0) as i64;
            ws.send(Message::Text(
                json!({"id":1,"method":"Input.dispatchMouseEvent",
                       "params":{"type":"mouseMoved","x":x,"y":y,
                                 "modifiers":modifiers}})
                .to_string(),
            ))
            .await?;
        }
        other => warn!("unknown input event type: {other}"),
    }

    ws.close(None).await.ok();
    Ok(())
}

/// Open / close / activate a Chrome tab.
async fn handle_tab_command(payload: &serde_json::Value) -> anyhow::Result<()> {
    info!("handle_tab_command: {:?}", payload);
    let client = reqwest::Client::new();
    match payload["command"].as_str().unwrap_or("") {
        "new" | "open" => {
            let url = payload["url"].as_str().unwrap_or("about:blank");
            // Chrome's /json/new endpoint — use PUT (more compatible across Chrome versions)
            let resp_text = client
                .put(format!("http://localhost:9222/json/new?{url}"))
                .send()
                .await?
                .text()
                .await
                .unwrap_or_default();
            info!("json/new response: {:?}", &resp_text[..resp_text.len().min(200)]);
            // Navigate to URL if one was provided
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
    let result = match msg.kind.as_str() {
        "navigate.url" => handle_navigate(&msg.payload).await,
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
    init_cache: Arc<Mutex<Option<Bytes>>>,
    video_tx: broadcast::Sender<Bytes>,
) -> anyhow::Result<()> {
    loop {
        match connect_once(&config, &identity, init_cache.clone(), video_tx.clone()).await {
            Ok(()) => warn!("agent websocket disconnected"),
            Err(err) => {
                let msg = err.to_string();
                warn!(error = %msg, "agent websocket failed");
                // Re-register if our runtime token was rejected (e.g. control plane restarted).
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
    init_cache: Arc<Mutex<Option<Bytes>>>,
    video_tx: broadcast::Sender<Bytes>,
) -> anyhow::Result<()> {
    let request = http_request_with_bearer(
        &format!("{}/api/v1/agents/connect", config.control_ws_url),
        &identity.agent_runtime_token,
    )?;
    let (socket, _) = connect_async(request).await?;
    let (mut ws_write, mut ws_read) = socket.split();
    info!("agent websocket connected");

    // mpsc channel: all background tasks → single WS writer task.
    let (tx, mut rx) = mpsc::channel::<Message>(256);

    // Task: drain the mpsc channel and write to the WebSocket.
    let writer_handle = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_write.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Task: stream video frames to this WS client.
    let video_rx = video_tx.subscribe();
    let video_handle = tokio::spawn(video_stream_task(init_cache, video_rx, tx.clone()));

    // Task: poll Chrome tabs and push changes.
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
