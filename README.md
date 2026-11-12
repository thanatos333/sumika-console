# Sumika Console

ローカルLLM（Strata 上の Qwen）を手動で簡単に「起動・設定・チャット・終了」できる Windows デスクトップアプリです。単一の exe で動作します。

## 機能

- **状態表示**: サーバー（`http://127.0.0.1:<port>`）の状態を数秒ごとに確認し、起動中はモデル名を表示
- **起動 / 終了**: Strata（`%LOCALAPPDATA%\Strata\terminal\current\bin\strata.cmd`）の launch / launch --stop を GUI から実行
- **チャット**: 彼女の本体（OpenCode）経由で対話します（v0.2.0〜）。作業フォルダ `D:\strata-work\friend` で `opencode run` を実行するため、憲章・記憶・道具（ファイル操作や画像生成キューなど）がそのまま使えます。「新しい会話」ボタンで文脈をリセットできます。初回の返信は本体側の文脈読み込みのため数分かかることがあります
- **設定**: family / model / context / vision / parallel / port / 追加引数を編集し、exe と同じフォルダの `settings.json` に保存
  - 追加引数は通常は空欄にしてください。起動プロファイル側の `user_args`（制御ベクトル等）はエンジン層の引数で、起動時に自動で適用されます。ここへ転記すると起動に失敗します

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
