# Design: Viewer Backpressure — 慢消费者处理

更新时间：2026-05-31（经 grill 审查修订）

## 1. 现状分析

### 当前架构

```
Agent → [binary frame] → Control Plane → broadcast::channel(128) → Viewer 1
                                                                  → Viewer 2
                                                                  → Viewer N
```

代码中实际存在三个 broadcast channel：

| Channel | 位置 | 容量 | 发送方 | 订阅方 |
|---|---|---|---|---|
| `video_tx` | `agent/main.rs` | 128 | `screencast.rs` | `connection.rs`（1个） |
| `bcast_tx` per-browser | `control-plane/ws/agent.rs` | 128 | agent WS handler | 每个 viewer |
| `state.preview_tx` | `control-plane/state.rs` | 64 | agent WS handler | **无订阅者（死代码）** |

### 已确认的 Bug

| Bug | 位置 | 影响 |
|------|------|------|
| `recv().await.ok()` 静默吞掉 `RecvError::Lagged(n)` | `control.rs` video_fut | 丢帧无日志，不可观察 |
| `RecvError::Closed` 同样被 `.ok()` 吞掉 | `control.rs` video_fut 和 event_fut | channel 关闭后每次 select 迭代立即返回 `None`，造成**紧密循环（spin loop）**直到客户端断开 |
| 每收到一帧后的错误重订阅逻辑 | `control.rs` video 分支末尾 | queue 不为空时创建新 receiver，**跳过已有帧** |
| `state.preview_tx` 无人订阅 | `control-plane/state.rs` | 每帧都 clone 并发送到死 channel，纯浪费 |
| broadcast buffer 128 帧 | agent 和 control-plane | viewer 需落后 ~12 秒（@10fps）才触发 Lagged，恢复太迟 |

### `tokio::broadcast` 行为

- 当 receiver 落后超过 buffer 容量时，`recv()` 返回 `Lagged(n)`
- Lagged 后 receiver 被自动快进到最新位置；下一次 `recv()` 正常返回最新帧
- 不会阻塞 sender
- JPEG 逐帧模式下跳帧无害；未来切 fMP4 时需要额外的关键帧恢复策略

## 2. 设计目标

- **不影响快 viewer**：慢 viewer 的问题不能拖慢其他人
- **优雅降级**：慢 viewer 应丢帧而非无限缓冲
- **可观察**：Lagged 事件通过结构化日志记录
- **无 spin loop**：channel 关闭后应干净退出，通知前端
- **未来兼容**：为 fMP4/MSE 的 segment 模型预留接口

## 3. 实现方案

### 3.1 Control Plane 变更（`ws/control.rs`）

提取辅助函数处理三种 recv 状态，保持 select 分支简洁：

```rust
use tokio::sync::broadcast;

enum PreviewRecv {
    Frame(Vec<u8>),
    Lagged(u64),
    Closed,
}

async fn recv_preview(rx: &mut broadcast::Receiver<Vec<u8>>) -> PreviewRecv {
    match rx.recv().await {
        Ok(bytes) => PreviewRecv::Frame(bytes),
        Err(broadcast::error::RecvError::Lagged(n)) => PreviewRecv::Lagged(n),
        Err(broadcast::error::RecvError::Closed) => PreviewRecv::Closed,
    }
}
```

主循环中的 video 分支改为：

```rust
let video_fut = async {
    match preview_rx.as_mut() {
        Some(rx) => Some(recv_preview(rx).await),
        None => std::future::pending().await,
    }
};

// ...

tokio::select! {
    // ...
    Some(result) = video_fut => {
        match result {
            PreviewRecv::Frame(bytes) => {
                if sender.send(Message::Binary(bytes)).await.is_err() {
                    return;
                }
            }
            PreviewRecv::Lagged(n) => {
                // broadcast 内部已自动快进到最新位置；下一帧正常到来即可恢复
                // JPEG 模式无需发送缓存帧
                tracing::warn!(browser_id = ?current_browser_id, lagged_frames = n, "viewer lagged");
            }
            PreviewRecv::Closed => {
                // agent 已断线，通知前端，清理 receiver 避免 spin loop
                let _ = sender.send(Message::Text(
                    serde_json::json!({"type": "preview.ended"}).to_string()
                )).await;
                preview_rx = None;
            }
        }
    }
    // ...
}
```

同样处理 `events_rx`（当前也用 `.ok()` 静默处理 Closed）：

```rust
async fn recv_event(rx: &mut broadcast::Receiver<String>) -> Option<String> {
    match rx.recv().await {
        Ok(s) => Some(s),
        Err(broadcast::error::RecvError::Lagged(n)) => {
            tracing::warn!(lagged_events = n, "event channel lagged");
            None  // select 忽略该次，下一帧正常恢复
        }
        Err(broadcast::error::RecvError::Closed) => {
            // events channel 关闭，清理
            None  // 调用方设 events_rx = None
        }
    }
}
```

**同时删除**每帧后的错误重订阅逻辑（video 分支末尾的 `is_empty()` 检查块）。

### 3.2 减小 broadcast buffer

```rust
// 控制面 per-browser channel（ws/agent.rs）
// 当前 128 → 改为 16（约 1.5 秒 @10fps，更快触发 Lagged 恢复）
let (bcast_tx, _) = broadcast::channel::<Vec<u8>>(16);

// agent 侧（main.rs）
// 只有 control plane 一个 subscriber，128 → 16
let (video_tx, _) = broadcast::channel::<Bytes>(16);
```

### 3.3 删除死代码 `state.preview_tx`

从 `AppState` 中删除 `preview_tx` 字段，从 `state.rs` 构造函数和 `ws/agent.rs` 中移除所有引用。

### 3.4 前端适配（`InputOverlay.tsx`）

响应 `preview.ended` 消息显示 "Disconnected" overlay：

```typescript
// 在 BrowserDetail 页面的 ws 消息处理中
if (message.type === 'preview.ended') {
  setPreviewEnded(true);
  return;
}
// 收到新帧时清除 overlay
setPreviewEnded(false);
```

`InputOverlay` 接收 `previewEnded` prop，在 canvas 上叠加提示层：

```tsx
{previewEnded && (
  <div className="preview-ended-overlay">
    <span>Browser disconnected</span>
  </div>
)}
```

帧间隔检测（>3s 无新帧）作为补充安全网，防止 `preview.ended` 消息丢失：

```typescript
let lastFrameTime = Date.now();
// 在帧渲染后更新 lastFrameTime
// 定时检查（每秒）：if (Date.now() - lastFrameTime > 3000) setPreviewEnded(true)
```

## 4. 可观察性

本次不引入 `metrics` crate（当前无 Prometheus 基础设施）。用结构化日志替代：

```
WARN jbrowser_control_plane::server::ws::control: viewer lagged browser_id=... lagged_frames=42
```

后续作为独立任务添加 `metrics` + `metrics-exporter-prometheus` 并暴露 `/metrics` 端点。

## 5. 未来演进：fMP4/MSE 模式的背压

当切换到 fMP4 segment 模式后，帧不再独立——丢帧可能导致解码器出错。策略变为：

1. **Init segment 必达**：不能丢 init segment
2. **丢帧时发送 init + latest keyframe**：确保解码器可恢复
3. **Per-viewer adaptive quality**：慢 viewer 降低帧率/质量
4. **Per-viewer mpsc + ring buffer**：替代直接处理 broadcast Lagged，实现 DropOldest 策略

这是 P2 优化，当前 JPEG 模式不需要。

## 6. SOP 验证步骤

```bash
# 1. 启动环境
make up

# 2. 打开 Web UI，连接到 browser detail 页面
# 确认可以看到实时画面

# 3. 模拟慢 viewer（使用 websocat 限速连接）
websocat -t --ping-interval 5 "ws://localhost:3000/ws/control" \
  | pv -L 1k > /dev/null  # 限制读取速度为 1KB/s

# 4. 观察 control plane 日志
# 应该看到结构化 WARN: "viewer lagged lagged_frames=N"

# 5. 验证正常 viewer 不受影响
# 在另一个浏览器标签页打开同一 browser detail
# 画面应保持流畅

# 6. 验证 Closed 恢复
# 停止 agent，观察：
# - control plane 日志无 spin loop（不应有大量连续日志）
# - 前端出现 "Browser disconnected" overlay

# 7. 验证前端恢复
# 重新启动 agent，browser.subscribe 后 overlay 消失，画面恢复

# 8. 压测：开 10 个 viewer 同时看同一 browser
# 确认没有 memory leak 或 panic
```

## 7. 影响范围

| 文件 | 变更 |
|---|---|
| `crates/control-plane/src/server/ws/control.rs` | 提取 `recv_preview` / `recv_event` 辅助函数；处理 Lagged（记日志）和 Closed（发 `preview.ended`，置 None）；删除错误重订阅逻辑 |
| `crates/control-plane/src/server/ws/agent.rs` | `bcast_tx` buffer 128→16；移除 `state.preview_tx.send()` 调用 |
| `crates/control-plane/src/server/state.rs` | 删除 `preview_tx` 字段及构造函数初始化 |
| `crates/agent/src/main.rs` | `video_tx` buffer 128→16 |
| `frontend/src/components/InputOverlay.tsx` | 接收 `previewEnded` prop；显示 "Browser disconnected" overlay |
| `frontend/src/pages/BrowserDetail.tsx` | 响应 `preview.ended` 消息；帧间隔超时安全网 |
