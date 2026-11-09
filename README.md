# Sumika Console

ローカルLLM（Strata 上の Qwen）を手動で簡単に「起動・設定・チャット・終了」できる Windows デスクトップアプリです。単一の exe で動作します。

## 機能

- **状態表示**: サーバー（`http://127.0.0.1:<port>`）の状態を数秒ごとに確認し、起動中はモデル名を表示
- **起動 / 終了**: Strata（`%LOCALAPPDATA%\Strata\terminal\current\bin\strata.cmd`）の launch / launch --stop を GUI から実行
- **チャット**: OpenAI 互換 API（`POST /v1/chat/completions`、非ストリーミング）で対話。システムプロンプトは編集・保存可能
- **設定**: family / model / context / vision / parallel / port / 追加引数を編集し、exe と同じフォルダの `settings.json` に保存
  - 初回起動時は、Strata の起動設定 `%LOCALAPPDATA%\Strata\launch\iq3_xxs-128k.json` の `user_args`（制御ベクトル等）を「追加引数」の初期値として引き継ぎます

## 使い方

1. `sumika-console.exe` をダブルクリックで起動
2. ヘッダーの「起動」でサーバーを起動（モデルの読み込みに数分かかることがあります）
3. 「チャット」タブで対話。「設定」タブで起動パラメータを変更（次回起動から反映）
4. 「終了」でサーバーを停止

## 自己テスト

```
sumika-console.exe --selftest
```

GUI を出さずに、① settings.json の保存→読込の往復チェック、② サーバー起動中なら `/v1/models` の取得、を行い、結果をコンソールと exe と同じフォルダの `selftest-result.txt` に出力します。終了コード 0 = 成功（サーバー未応答の場合は ② をスキップして正常終了）。

## ビルド方法

Rust（cargo）が必要です。

```
cargo build --release
```

成果物は `target\release\sumika-console.exe` です。適当なフォルダ（例: `C:\Users\<ユーザー>\tools\sumika-console\`）に exe を置いて使ってください。`settings.json` は exe と同じフォルダに作られます。

## 技術

- Rust + eframe/egui（GUI、glow レンダラー）
- ureq（HTTP クライアント）
- serde / serde_json
- 日本語表示のため、起動時に Windows のフォント（游ゴシック等）を読み込みます
