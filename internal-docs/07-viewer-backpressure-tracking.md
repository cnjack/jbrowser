# Viewer Backpressure — Implementation Tracking

Based on: [07-design-viewer-backpressure.md](./07-design-viewer-backpressure.md)

## Status: ✅ Complete

## Changes Made

### 1. Control Plane `ws/control.rs` — ✅ Done

- **Added `PreviewRecv` enum** and `recv_preview()` helper to handle `Frame`, `Lagged`, `Closed` states
- **Added `recv_event()` helper** that logs `Lagged` events with `tracing::warn!`
- **Replaced `.recv().await.ok()`** (which silently swallowed `Lagged` and `Closed` errors) with proper match arms:
  - `Frame(bytes)` → send binary to viewer
  - `Lagged(n)` → log warning with `browser_id` and `lagged_frames`, no action needed (broadcast auto-advances)
  - `Closed` → send `{"type": "preview.ended"}` text message to frontend, set `preview_rx = None` (prevents spin loop)
- **Removed buggy resubscribe logic** (the `is_empty()` check block after each frame that was skipping buffered frames)

### 2. Broadcast Buffer Reduction — ✅ Done

| Location | Before | After |
|---|---|---|
| `crates/agent/src/main.rs` `video_tx` | 128 | 16 |
| `crates/control-plane/src/server/ws/agent.rs` `bcast_tx` | 128 | 16 |

~1.5 seconds of buffer at 10fps — triggers `Lagged` recovery much faster.

### 3. Dead Code Removal: `state.preview_tx` — ✅ Done

- Removed `preview_tx: broadcast::Sender<Vec<u8>>` field from `AppState`
- Removed `broadcast::channel(64)` initialization in `AppState::new()`
- Removed `state.preview_tx.send(bytes.clone())` call in `ws/agent.rs`

### 4. Frontend `ws/control.ts` — ✅ Done

- Added `preview.ended` to `ControlEvent` union type
- Added `preview.ended` to message type dispatch filter

### 5. Frontend `BrowserDetail.tsx` — ✅ Done

- Added `previewEnded` state
- Handle `preview.ended` event → `setPreviewEnded(true)`
- Clear `previewEnded` on new frame arrival → `setPreviewEnded(false)`
- Added frame watchdog (3s timeout) as safety net against missed `preview.ended` messages
- Pass `previewEnded` prop to `InputOverlay`

### 6. Frontend `InputOverlay.tsx` — ✅ Done

- Added `previewEnded?: boolean` prop
- Render "Browser disconnected" overlay when `previewEnded` is true
- "Waiting for stream" overlay only shows when no segment AND not ended

### 7. CSS `styles.css` — ✅ Done

- Added `.preview-ended-overlay` style (dark overlay with centered white text)

## Files Modified

| File | Change Summary |
|---|---|
| `crates/control-plane/src/server/ws/control.rs` | `recv_preview`/`recv_event` helpers; Lagged logging; Closed → `preview.ended`; removed resubscribe |
| `crates/control-plane/src/server/ws/agent.rs` | `bcast_tx` buffer 128→16; removed `state.preview_tx.send()` |
| `crates/control-plane/src/server/state.rs` | Removed `preview_tx` field + initialization |
| `crates/agent/src/main.rs` | `video_tx` buffer 128→16 |
| `frontend/src/ws/control.ts` | Added `preview.ended` event type + dispatch |
| `frontend/src/pages/BrowserDetail.tsx` | `previewEnded` state; `preview.ended` handler; frame watchdog |
| `frontend/src/components/InputOverlay.tsx` | `previewEnded` prop; disconnected overlay |
| `frontend/src/styles.css` | `.preview-ended-overlay` styles |

## Verification

- ✅ `cargo check` — compiles cleanly (no warnings, no errors)
- ✅ `npx tsc --noEmit` — TypeScript compiles cleanly

## SOP Verification Steps

See design doc section 6 for full manual test procedure.
