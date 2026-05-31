use axum::{extract::State, Json};
use serde_json::{json, Value};

use jbrowser_shared::models::BrowserStatus;

use crate::server::state::AppState;

pub async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

pub async fn ready(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "checks": {
            "database_url_configured": !state.config.database_url.is_empty(),
            "state": "mysql_hybrid"
        }
    }))
}

pub async fn metrics(State(state): State<AppState>) -> String {
    let browsers = state.browsers.read().await;
    let online = browsers
        .values()
        .filter(|browser| browser.status == BrowserStatus::Online)
        .count();
    format!(
        "# HELP browser_instances_online Online browser instances\n# TYPE browser_instances_online gauge\nbrowser_instances_online {}\n",
        online
    )
}
