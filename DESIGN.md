# Sumika Console — 設計書

## 目的
あつやさんがローカルLLM（Strata 上の Qwen）を手動で簡単に「起動・設定・プロンプト入力（チャット）・終了」できる Windows デスクトップアプリ。単一 exe で配布する。

## 技術
- Rust + eframe/egui（GUI）、ureq（HTTP、rustls）、serde / serde_json
- Windows 専用。単一の release exe（依存 DLL なし）

## 重要な前提
- Strata: `%LOCALAPPDATA%\Strata\terminal\current\bin\strata.cmd`（.cmd なので起動は `cmd /C` 経由）
- サーバー: `http://127.0.0.1:<port>`（既定 8080）、OpenAI 互換 API（GET /v1/models、POST /v1/chat/completions）
- **日本語フォント必須**: egui 標準フォントには CJK が無い。起動時に `C:\Windows\Fonts\YuGothM.ttc`（無ければ `meiryob.ttc`、`msgothic.ttc` の順に探す）を読み込み、FontDefinitions の Proportional/Monospace の末尾にフォールバックとして追加すること。

## 画面構成（1ウィンドウ、タブ切替）
### 共通ヘッダー
- サーバー状態表示: ● 起動中（モデル名を /v1/models から表示）/ ○ 停止中 / ◐ 起動処理中
- 「起動」「終了」ボタン（状態に応じて活性/非活性）
- 状態は数秒ごとにポーリングして自動更新

### 「チャット」タブ
- システムプロンプト欄（編集可、設定に保存）。既定値: 「あなたは「すみか」です。あつやさんのPCに住むローカルAIとして、丁寧で役立つ回答を日本語でしてください。」
- 会話履歴（スクロール表示、ユーザー/アシスタントを区別）
- 入力欄（複数行）＋「送信」ボタン。送信中は「生成中…」表示
- POST /v1/chat/completions（非ストリーミング、model は /v1/models の先頭 id、temperature 0.7）
- HTTP はワーカースレッド＋ mpsc チャネルで実行し、UI をブロックしない

### 「設定」タブ
- family: ドロップダウン（qwen / qwen36）
- model: テキスト（例: IQ3_XXS、UD-Q4_K_XL）
- context: テキスト（例: 128k）
- vision: ドロップダウン（off / on / gpu / cpu）
- parallel: 数値（1〜8）
- port: 数値（既定 8080）
- 追加引数: 複数行テキスト。**初回起動時**に `%LOCALAPPDATA%\Strata\launch\iq3_xxs-128k.json` の `user_args` があれば、その内容を初期値として読み込む（制御ベクトル等の引き継ぎ）
- 設定は exe と同じフォルダの `settings.json` に保存・読込（「保存」ボタン＋起動時に自動読込）

## 起動/終了の実装
- 起動: `cmd /C strata.cmd launch --family <f> --model <m> --context <c> --vision <v> --parallel <n> --port <p> --yes --no-tui <追加引数>` を DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP で spawn し、即座に制御を返す。成否はポーリング（/v1/models）で判定し、一定時間応答が無ければ「起動に時間がかかっています」と表示（エラー断定しない）
- 終了: `cmd /C strata.cmd launch --stop` を実行。ポーリングで停止を確認

## 自己テスト
- `--selftest` 引数で GUI を出さず: ① settings.json の保存→読込の往復一致、② サーバー起動中なら /v1/models の取得とモデル名表示、をコンソールに出力し、exit code（0=成功）で終了する

## ビルドと配置
- 開発は手元（VM）にソースを書き、PC の `C:\Users\Atsuya\source\sumika-console` へコピーして `cargo build --release`
- 成果物 `sumika-console.exe` を `C:\Users\Atsuya\tools\sumika-console\` へ配置
- 最終ソース一式を `~/workspace/projects/sumika-console/src/` に戻す（後で GitHub 公開に使う）
- README.md（日本語、使い方とビルド方法）を同梱する
