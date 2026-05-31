use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AgentStatus, BrowserTab};
use crate::constants::{
    DEFAULT_DEVICE_SCALE_FACTOR, DEFAULT_VIEWPORT_HEIGHT, DEFAULT_VIEWPORT_WIDTH,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserStatus {
    Online,
    Offline,
    Unhealthy,
    Restarting,
}

/// Stealth evasion level
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StealthLevel {
    /// No evasion — transparent automation
    #[default]
    None,
    /// Basic: hide navigator.webdriver + common headless detection points
    Basic,
}

/// Browser fingerprint configuration (per-instance)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintConfig {
    /// Custom User-Agent (None = Chrome default)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// Viewport width (default 1280)
    #[serde(default = "default_viewport_width")]
    pub viewport_width: u32,
    /// Viewport height (default 720)
    #[serde(default = "default_viewport_height")]
    pub viewport_height: u32,
    /// Device scale factor / DPR (default 1.0)
    #[serde(default = "default_device_scale_factor")]
    pub device_scale_factor: f64,
    /// IANA timezone (e.g. "America/New_York"), None = system default
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// BCP-47 locale (e.g. "en-US"), None = system default
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
}

fn default_viewport_width() -> u32 {
    DEFAULT_VIEWPORT_WIDTH
}
fn default_viewport_height() -> u32 {
    DEFAULT_VIEWPORT_HEIGHT
}
fn default_device_scale_factor() -> f64 {
    DEFAULT_DEVICE_SCALE_FACTOR
}

impl Default for FingerprintConfig {
    fn default() -> Self {
        Self {
            user_agent: None,
            viewport_width: DEFAULT_VIEWPORT_WIDTH,
            viewport_height: DEFAULT_VIEWPORT_HEIGHT,
            device_scale_factor: DEFAULT_DEVICE_SCALE_FACTOR,
            timezone: None,
            locale: None,
        }
    }
}

/// Top-level browser configuration (fingerprint + stealth)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrowserConfig {
    /// Fingerprint configuration
    #[serde(default)]
    pub fingerprint: FingerprintConfig,
    /// Stealth evasion level
    #[serde(default)]
    pub stealth: StealthLevel,
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
    /// Browser fingerprint + stealth configuration
    #[serde(default)]
    pub config: BrowserConfig,
}
