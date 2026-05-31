use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use base64::Engine;
use bytes::Bytes;
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use jbrowser_shared::protocol::{encode_video_frame, VideoFrame, VideoFrameType};
use serde_json::json;
use tokio::sync::{broadcast, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};

use crate::chrome::ensure_page_target;
use crate::globals::active_tab_rx;

pub(crate) async fn screencast_loop(
    video_tx: broadcast::Sender<Bytes>,
    last_frame: Arc<Mutex<Option<Bytes>>>,
) {
    loop {
        match run_screencast(&video_tx, &last_frame).await {
            Ok(()) => {
                info!("screencast session ended, reconnecting in 100ms");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => {
                warn!("screencast error: {e}, reconnecting in 2s");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

async fn run_screencast(
    video_tx: &broadcast::Sender<Bytes>,
    last_frame: &Arc<Mutex<Option<Bytes>>>,
) -> anyhow::Result<()> {
    let target = ensure_page_target().await?;
    let ws_url = target["webSocketDebuggerUrl"]
        .as_str()
        .context("target missing webSocketDebuggerUrl")?;

    let (cdp_ws, _) = connect_async(ws_url)
        .await
        .context("CDP WS connect failed")?;

    let (mut ws_write, mut ws_read) = cdp_ws.split();

    ws_write
        .send(Message::Text(
            json!({
                "id": 0,
                "method": "Emulation.setDeviceMetricsOverride",
                "params": {
                    "width": 1280,
                    "height": 720,
                    "deviceScaleFactor": 1,
                    "mobile": false
                }
            })
            .to_string(),
        ))
        .await?;

    ws_write
        .send(Message::Text(
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
    let mut tab_rx = active_tab_rx();

    loop {
        tokio::select! {
            msg_result = ws_read.next() => {
                match msg_result {
                    Some(Ok(Message::Text(text))) => {
                        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
                            continue;
                        };

                        if value.get("method").and_then(|v| v.as_str()) == Some("Page.screencastFrame") {
                            let params = &value["params"];
                            let session_id = params["sessionId"].as_i64().unwrap_or(0);
                            let data_b64 = params["data"].as_str().unwrap_or("");

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
                                let _ = video_tx.send(encoded.clone());
                                *last_frame.lock().await = Some(encoded);
                            }

                            let ack = json!({
                                "id": 100 + session_id,
                                "method": "Page.screencastFrameAck",
                                "params": { "sessionId": session_id }
                            });
                            if ws_write.send(Message::Text(ack.to_string())).await.is_err() {
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
            _ = tab_rx.changed() => {
                info!("active tab changed, reconnecting screencast");
                break;
            }
        }
    }

    Ok(())
}
