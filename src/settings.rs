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

/// 追加引数の初期値。以前は起動プロファイル（iq3_xxs-128k.json）の user_args を
/// 引き継いでいたが、あれはエンジン（llama-server層）への引数であり、
/// `strata launch` のオプションではないため、末尾に付けると起動が即失敗する
/// （2026-11-13 実測）。user_args はプロファイル側に保存されていて起動時に
/// 自動で適用されるので、こちらは空欄が正しい。
pub fn seed_extra_args() -> String {
    String::new()
}
