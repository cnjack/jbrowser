use std::{collections::HashMap, sync::OnceLock};

use std::sync::Arc;
use tokio::sync::{mpsc, watch, Mutex, RwLock};

// ── Global persistent input channel ────────────────────────────────────────
pub(crate) static INPUT_TX: OnceLock<mpsc::Sender<serde_json::Value>> = OnceLock::new();

pub(crate) fn input_tx() -> Option<&'static mpsc::Sender<serde_json::Value>> {
    INPUT_TX.get()
}

// ── Global CDP tunnel sessions ─────────────────────────────────────────────
pub(crate) type CdpTunnelMap = Arc<RwLock<HashMap<String, mpsc::Sender<String>>>>;
pub(crate) static CDP_TUNNELS: OnceLock<CdpTunnelMap> = OnceLock::new();

pub(crate) fn cdp_tunnels() -> &'static CdpTunnelMap {
    CDP_TUNNELS.get().expect("CDP_TUNNELS not set")
}

// ── Global control-plane sender ────────────────────────────────────────────
pub(crate) static CONTROL_TX: OnceLock<
    Mutex<Option<mpsc::Sender<tokio_tungstenite::tungstenite::Message>>>,
> = OnceLock::new();

pub(crate) async fn send_to_control(msg: tokio_tungstenite::tungstenite::Message) -> bool {
    if let Some(lock) = CONTROL_TX.get() {
        let guard = lock.lock().await;
        if let Some(tx) = guard.as_ref() {
            return tx.try_send(msg).is_ok();
        }
    }
    false
}

// ── Global active-tab tracker ──────────────────────────────────────────────
pub(crate) static ACTIVE_TAB_TX: OnceLock<watch::Sender<Option<String>>> = OnceLock::new();

pub(crate) fn active_tab_rx() -> watch::Receiver<Option<String>> {
    ACTIVE_TAB_TX
        .get()
        .expect("ACTIVE_TAB_TX not set")
        .subscribe()
}

pub(crate) fn set_active_tab(tab_id: &str) {
    if let Some(tx) = ACTIVE_TAB_TX.get() {
        tx.send_replace(Some(tab_id.to_string()));
    }
}
