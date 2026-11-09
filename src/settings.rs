use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const DEFAULT_SYSTEM_PROMPT: &str =
    "あなたは「すみか」です。あつやさんのPCに住むローカルAIとして、丁寧で役立つ回答を日本語でしてください。";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Strata のモデルファミリー（qwen / qwen36）
    pub family: String,
    /// モデル（量子化）名。例: IQ3_XXS、UD-Q4_K_XL
    pub model: String,
    /// コンテキスト長。例: 128k
    pub context: String,
    /// vision: off / on / gpu / cpu
    pub vision: String,
    /// 並列リクエスト数 1..=8
    pub parallel: u32,
    /// サーバーのポート
    pub port: u16,
    /// 起動コマンド末尾へそのまま付与する追加引数（制御ベクトル等）
    pub extra_args: String,
    /// チャットのシステムプロンプト
    pub system_prompt: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            family: "qwen".to_string(),
            model: "IQ3_XXS".to_string(),
            context: "128k".to_string(),
            vision: "gpu".to_string(),
            parallel: 1,
            port: 8080,
            extra_args: String::new(),
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
        }
    }
}

/// settings.json の場所（exe と同じフォルダ）
pub fn settings_path() -> PathBuf {
    let exe = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("sumika-console.exe"));
    let dir = exe
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    dir.join("settings.json")
}

impl Settings {
    /// ファイルがあれば読み込む。無ければ既定値＋初回シード（追加引数の引き継ぎ）。
    pub fn load(path: &Path) -> Self {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(s) = serde_json::from_str::<Settings>(&text) {
                return s;
            }
        }
        let mut s = Settings::default();
        s.extra_args = seed_extra_args();
        s
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, text)
    }
}

/// 初回起動時、Strata の起動設定（iq3_xxs-128k.json）の user_args を
/// 追加引数の初期値として読み込む（制御ベクトル設定などの引き継ぎ用）。
/// user_args が文字列ならそのまま、配列なら空白区切りで連結する。
pub fn seed_extra_args() -> String {
    let Ok(local) = std::env::var("LOCALAPPDATA") else {
        return String::new();
    };
    let path = PathBuf::from(local)
        .join("Strata")
        .join("launch")
        .join("iq3_xxs-128k.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return String::new();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return String::new();
    };
    match v.get("user_args") {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|x| x.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}
