use serde_json::Value;
use std::time::Duration;

fn agent(connect_secs: u64, read_secs: u64) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(connect_secs))
        .timeout_read(Duration::from_secs(read_secs))
        .timeout_write(Duration::from_secs(15))
        .build()
}

/// GET /v1/models を取得し、モデル id の一覧を返す。
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

/// POST /v1/chat/completions（非ストリーミング）で応答本文を返す。
/// 生成には時間がかかることがあるため読み取りタイムアウトは長め。
pub fn chat_completion(port: u16, model: &str, messages: &[ChatMsg]) -> Result<String, String> {
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
    let msgs: Vec<Value> = messages
        .iter()
        .map(|m| serde_json::json!({ "role": m.role, "content": m.content }))
        .collect();
    let body = serde_json::json!({
        "model": model,
        "messages": msgs,
        "temperature": 0.7,
        "stream": false,
    });
    let resp = agent(5, 1800).post(&url).send_json(body).map_err(|e| match e {
        ureq::Error::Status(code, r) => {
            let text = r.into_string().unwrap_or_default();
            format!("HTTP {code}: {text}")
        }
        other => other.to_string(),
    })?;
    let v: Value = resp.into_json().map_err(|e| e.to_string())?;
    v.get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "応答の形式が想定と違いました".to_string())
}
