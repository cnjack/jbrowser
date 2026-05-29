use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Online,
    Offline,
    Unhealthy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummary {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub browser_instance_id: Uuid,
    pub name: String,
    pub status: AgentStatus,
    pub browser_type: String,
    pub browser_version: String,
    pub last_heartbeat_at: Option<String>,
}
