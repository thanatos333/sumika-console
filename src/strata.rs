use crate::settings::Settings;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Strata 本体（strata.cmd）のパス
pub fn strata_cmd_path() -> PathBuf {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(local)
            .join("Strata")
            .join("terminal")
            .join("current")
            .join("bin")
            .join("strata.cmd");
    }
    PathBuf::from("strata.cmd")
}

/// サーバーを起動する。strata.cmd はバッチファイルなので cmd /C 経由で実行し、
/// DETACHED_PROCESS で切り離して即座に制御を返す（成否は /v1/models のポーリングで判定）。
pub fn start_server(s: &Settings) -> Result<(), String> {
    let path = strata_cmd_path();
    if !path.exists() {
        return Err(format!(
            "strata.cmd が見つかりません: {}",
            path.display()
        ));
    }
    let mut cmd = Command::new("cmd");
    cmd.arg("/C")
        .arg(&path)
        .arg("launch")
        .arg("--family")
        .arg(&s.family)
        .arg("--model")
        .arg(&s.model)
        .arg("--context")
        .arg(&s.context)
        .arg("--vision")
        .arg(&s.vision)
        .arg("--parallel")
        .arg(s.parallel.to_string())
        .arg("--port")
        .arg(s.port.to_string())
        .arg("--yes")
        .arg("--no-tui");
    for a in s.extra_args.split_whitespace() {
        cmd.arg(a);
    }
    #[cfg(windows)]
    cmd.creation_flags(0x0000_0008 | 0x0000_0200); // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("起動コマンドの実行に失敗しました: {e}"))
}

/// サーバーを終了する（strata launch --stop）。
pub fn stop_server() -> Result<(), String> {
    let path = strata_cmd_path();
    if !path.exists() {
        return Err(format!(
            "strata.cmd が見つかりません: {}",
            path.display()
        ));
    }
    let out = Command::new("cmd")
        .arg("/C")
        .arg(&path)
        .arg("launch")
        .arg("--stop")
        .output()
        .map_err(|e| format!("終了コマンドの実行に失敗しました: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if msg.is_empty() {
            Err(format!("strata が終了コード {} を返しました", out.status))
        } else {
            Err(msg)
        }
    }
}
