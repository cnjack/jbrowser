use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AgentStatus, BrowserTab};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserStatus {
    Online,
    Offline,
    Unhealthy,
    Restarting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserInstance {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub agent_id: Uuid,
    pub name: String,
    pub status: BrowserStatus,
    pub browser_type: String,
    pub browser_version: String,
    pub active_tab_id: Option<String>,
    pub tabs: Vec<BrowserTab>,
    pub proxy_enabled: bool,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub viewer_count: usize,
    pub agent_name: String,
    pub agent_status: AgentStatus,
    pub last_heartbeat_at: Option<String>,
}
