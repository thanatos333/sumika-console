//! チャットを彼女の本体（OpenCode）経由で送るためのモジュール。
//!
//! 以前は Strata の /v1/chat/completions へ直接 POST していたため、コンソールの
//! チャットでは彼女の憲章・記憶・道具が一切読み込まれず、「別人」になっていた。
//! ここでは `opencode run` を彼女の家（D:\strata-work\friend）を作業フォルダとして
//! 起動し、憲章と道具を持った本人と会話する。
//!
//! モデルはハーネス（PDCAループ）と同方式で明示指定する：
//! - `--model strata/<起動中モデル id>`
//! - OPENCODE_CONFIG に、このアプリが生成する設定ファイルを指定し、
//!   strata プロバイダとモデル定義を確実に与える。
//! セッションは `--session` で継続し、コンソール内の会話がそのまま続く。

use serde_json::{json, Value};
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// OpenCode 本体
pub fn opencode_exe() -> PathBuf {
    PathBuf::from(r"C:\Users\Atsuya\tools\opencode\opencode.exe")
}

/// 彼女の家（憲章 AGENTS.md と記憶・道具の置き場所）
pub fn friend_dir() -> PathBuf {
    PathBuf::from(r"D:\strata-work\friend")
}

/// このアプリ専用の OpenCode 設定ファイルを（exe の隣に）生成して、そのパスを返す。
/// 起動中モデルの id に合わせてモデル定義を作るので、ファミリーを切り替えても追随する。
fn write_config(model_id: &str, port: u16) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let path = dir.join("opencode-config.json");
    let model_ref = format!("strata/{model_id}");
    let cfg = json!({
        "$schema": "https://opencode.ai/config.json",
        "model": model_ref,
        "small_model": model_ref,
        "provider": {
            "strata": {
                "name": "Strata (local)",
                "npm": "@ai-sdk/openai-compatible",
                "options": {
                    "apiKey": "strata-local",
                    "baseURL": format!("http://127.0.0.1:{port}/v1")
                },
                "models": {
                    model_id: {
                        "name": "Local model on Strata",
                        "attachment": true,
                        "reasoning": true,
                        "tool_call": true,
                        "modalities": { "input": ["text", "image"], "output": ["text"] },
                        "limit": { "context": 131072, "input": 98304, "output": 32768 }
                    }
                }
            }
        }
    });
    let text = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("OpenCode設定の生成に失敗しました: {e}"))?;
    Ok(path)
}

/// `opencode run --format json` のイベント列から、最終回答テキストとセッション id を取り出す。
fn parse_events(stdout: &str) -> Result<(String, Option<String>), String> {
    let mut session: Option<String> = None;
    // part.id の初出順を保ちながら、同じ part の更新は最後の内容で上書きする
    let mut order: Vec<String> = Vec::new();
    let mut texts: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut error: Option<String> = None;

    for line in stdout.lines() {
        let line = line.trim();
        if !line.starts_with('{') {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if session.is_none() {
            if let Some(s) = v.get("sessionID").and_then(|s| s.as_str()) {
                session = Some(s.to_string());
            }
        }
        match v.get("type").and_then(|t| t.as_str()) {
            Some("text") => {
                if let Some(part) = v.get("part") {
                    let id = part
                        .get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("")
                        .to_string();
                    let text = part
                        .get("text")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();
                    if !texts.contains_key(&id) {
                        order.push(id.clone());
                    }
                    texts.insert(id, text);
                }
            }
            Some("error") => {
                let msg = v
                    .get("error")
                    .and_then(|e| e.get("data"))
                    .and_then(|d| d.get("message"))
                    .and_then(|m| m.as_str())
                    .or_else(|| {
                        v.get("error")
                            .and_then(|e| e.get("name"))
                            .and_then(|n| n.as_str())
                    })
                    .unwrap_or("不明なエラー")
                    .to_string();
                error = Some(msg);
            }
            _ => {}
        }
    }

    let text = order
        .iter()
        .filter_map(|id| texts.get(id))
        .cloned()
        .collect::<Vec<_>>()
        .join("");
    if !text.trim().is_empty() {
        return Ok((text, session));
    }
    if let Some(e) = error {
        return Err(format!("OpenCode がエラーを返しました: {e}"));
    }
    Err("応答が空でした".to_string())
}

/// 1往復分のチャットを実行する。生成と道具の実行を待つため、制限時間は長め（30分）。
/// 戻り値は (回答テキスト, セッション id)。
pub fn run_chat(
    model_id: &str,
    port: u16,
    session: &Option<String>,
    message: &str,
) -> Result<(String, Option<String>), String> {
    let exe = opencode_exe();
    if !exe.exists() {
        return Err(format!(
            "OpenCode が見つかりません: {}",
            exe.display()
        ));
    }
    let cfg = write_config(model_id, port)?;

    let mut cmd = Command::new(&exe);
    cmd.arg("run")
        .arg("--auto")
        .arg("--format")
        .arg("json")
        .arg("--model")
        .arg(format!("strata/{model_id}"));
    if let Some(s) = session {
        cmd.arg("--session").arg(s);
    }
    cmd.arg(message);
    cmd.current_dir(friend_dir());
    cmd.env("OPENCODE_CONFIG", &cfg);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW（コンソール窓を出さない）

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("OpenCode の起動に失敗しました: {e}"))?;

    // stdout / stderr は別スレッドで読み切る（パイプ詰まり防止）
    let mut out_pipe = child.stdout.take();
    let (out_tx, out_rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(p) = out_pipe.as_mut() {
            let _ = p.read_to_string(&mut s);
        }
        let _ = out_tx.send(s);
    });
    let mut err_pipe = child.stderr.take();
    let (err_tx, err_rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(p) = err_pipe.as_mut() {
            let _ = p.read_to_string(&mut s);
        }
        let _ = err_tx.send(s);
    });

    let deadline = Instant::now() + Duration::from_secs(1800);
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => {
                if Instant::now() > deadline {
                    let _ = child.kill();
                    return Err("応答が制限時間（30分）を超えたため中断しました".to_string());
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(format!("OpenCode の実行状態を確認できませんでした: {e}")),
        }
    };
    let stdout = out_rx.recv().unwrap_or_default();
    let stderr = err_rx.recv().unwrap_or_default();

    match parse_events(&stdout) {
        Ok(r) => Ok(r),
        Err(e) => {
            if !status.success() {
                let tail: String = stderr.chars().rev().take(400).collect::<String>();
                let tail: String = tail.chars().rev().collect();
                Err(format!("{e}（終了コード {:?}）: {tail}", status.code()))
            } else {
                Err(e)
            }
        }
    }
}
