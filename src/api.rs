use serde_json::Value;
use std::time::Duration;

fn agent(connect_secs: u64, read_secs: u64) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(connect_secs))
        .timeout_read(Duration::from_secs(read_secs))
        .timeout_write(Duration::from_secs(15))
        .build()
}

/// GET /v1/models を取得し、モデル id の一覧を返す。（サーバー状態の監視用）
pub fn fetch_models(port: u16) -> Result<Vec<String>, String> {
    let url = format!("http://127.0.0.1:{port}/v1/models");
    let resp = agent(2, 3)
        .get(&url)
        .call()
        .map_err(|e| e.to_string())?;
    let v: Value = resp.into_json().map_err(|e| e.to_string())?;
    let mut ids = Vec::new();
    if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
        for item in arr {
            if let Some(id) = item.get("id").and_then(|i| i.as_str()) {
                ids.push(id.to_string());
            }
        }
    }
    Ok(ids)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatMsg {
    pub role: String,
    pub content: String,
}
